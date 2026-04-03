use anyhow::{anyhow, bail, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, SizedSample, Stream, StreamConfig};
use eq_mac_cli::eq::dsp::{DspChain, RuntimePreset};
use eq_mac_cli::eq::parser;
use eq_mac_cli::storage;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::fs;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

fn main() {
    if let Err(err) = run() {
        eprintln!("eqmacd error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    storage::paths::ensure_layout()?;
    storage::daemon_state::write_pid(std::process::id())?;

    let _guard = RuntimeGuard;

    eprintln!("eqmacd: starting");

    let socket_path = storage::daemon_state::socket_path()?;
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    listener.set_nonblocking(true)?;
    eprintln!("eqmacd: control socket ready at {}", socket_path.display());

    let host = cpal::default_host();
    eprintln!("eqmacd: host acquired");
    let device_config = storage::config::read_config()?;
    eprintln!("eqmacd: config loaded");
    let input_device = select_input_device(&host, device_config.input_device.as_deref())?;
    let input_name = input_device
        .name()
        .unwrap_or_else(|_| "unknown input".to_string());
    eprintln!("eqmacd: input device selected={input_name}");
    let input_config = input_device
        .default_input_config()
        .map_err(|err| anyhow!("failed to get default input config for {input_name}: {err}"))?;

    let output_device = select_output_device(&host, device_config.output_device.as_deref())?;
    let output_name = output_device
        .name()
        .unwrap_or_else(|_| "unknown output".to_string());
    eprintln!("eqmacd: output device selected={output_name}");
    let output_config = select_output_config(&output_device, input_config.sample_rate())
        .map_err(|err| anyhow!("failed to choose output config for {output_name}: {err}"))?;

    eprintln!(
        "eqmacd: input={} {}ch {}Hz {:?}",
        input_name,
        input_config.channels(),
        input_config.sample_rate().0,
        input_config.sample_format()
    );
    eprintln!(
        "eqmacd: output={} {}ch {}Hz {:?}",
        output_name,
        output_config.channels(),
        output_config.sample_rate().0,
        output_config.sample_format()
    );

    let sample_rate = output_config.sample_rate().0;
    let channels = output_config.channels() as usize;
    let capacity = (sample_rate as usize * channels * 5).max(8192);
    let ring = HeapRb::<f32>::new(capacity);
    let (producer, consumer) = ring.split();

    let running = Arc::new(AtomicBool::new(true));
    let state = Arc::new(SharedState::new(channels, sample_rate));
    load_active_preset(&state, sample_rate, channels)?;

    let control_thread = spawn_control_thread(
        listener,
        Arc::clone(&running),
        Arc::clone(&state),
        sample_rate,
        channels,
    );

    let input_stream = build_input_stream(
        &input_device,
        input_config.clone().into(),
        input_config.sample_format(),
        channels,
        producer,
    )
    .map_err(|err| anyhow!("failed to build input stream for {input_name}: {err}"))?;
    let output_stream = build_output_stream(
        &output_device,
        output_config.clone().into(),
        output_config.sample_format(),
        consumer,
        Arc::clone(&state),
    )
    .map_err(|err| anyhow!("failed to build output stream for {output_name}: {err}"))?;

    input_stream.play()?;
    output_stream.play()?;
    eprintln!("eqmacd: streams started");

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));
    }

    drop(input_stream);
    drop(output_stream);

    let _ = control_thread.join();

    if socket_path.exists() {
        fs::remove_file(socket_path)?;
    }
    storage::daemon_state::clear_pid()?;
    Ok(())
}

struct RuntimeGuard;

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        if let Ok(socket_path) = storage::daemon_state::socket_path() {
            if socket_path.exists() {
                let _ = fs::remove_file(socket_path);
            }
        }

        let _ = storage::daemon_state::clear_pid();
    }
}

struct SharedState {
    revision: AtomicU64,
    chain: RwLock<DspChain>,
    mode: RwLock<String>,
    channels: usize,
    sample_rate: u32,
}

impl SharedState {
    fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            revision: AtomicU64::new(0),
            chain: RwLock::new(DspChain::bypass(channels)),
            mode: RwLock::new("bypass".to_string()),
            channels,
            sample_rate,
        }
    }

    fn update_chain(&self, runtime_preset: Option<RuntimePreset>) {
        let (mode, chain) = match runtime_preset {
            Some(preset) => {
                let mode = format!("preset {}", preset.name);
                let chain = DspChain::from_runtime_preset(&preset, self.sample_rate, self.channels);
                (mode, chain)
            }
            None => ("bypass".to_string(), DspChain::bypass(self.channels)),
        };

        if let Ok(mut lock) = self.chain.write() {
            *lock = chain;
        }
        if let Ok(mut lock) = self.mode.write() {
            *lock = mode;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

fn load_active_preset(state: &SharedState, sample_rate: u32, channels: usize) -> Result<()> {
    let preset = current_runtime_preset()?;
    let _ = (sample_rate, channels);
    state.update_chain(preset);
    Ok(())
}

fn current_runtime_preset() -> Result<Option<RuntimePreset>> {
    let Some(id) = storage::active::read_active_id()? else {
        return Ok(None);
    };

    let name = storage::index::resolve_id(id)?.name;
    let raw = storage::presets::read_raw_preset(id)?;
    let preset = parser::parse_preset(&raw, Some(name))?;
    Ok(Some(RuntimePreset::from_preset(&preset)))
}

fn spawn_control_thread(
    listener: UnixListener,
    running: Arc<AtomicBool>,
    state: Arc<SharedState>,
    _sample_rate: u32,
    _channels: usize,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = String::new();
                    if stream.read_to_string(&mut buf).is_ok() {
                        match buf.trim() {
                            "reload" => match current_runtime_preset() {
                                Ok(preset) => state.update_chain(preset),
                                Err(err) => eprintln!("eqmacd: failed to reload preset: {err}"),
                            },
                            "stop" => running.store(false, Ordering::SeqCst),
                            _ => {}
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(err) => {
                    eprintln!("eqmacd: control socket error: {err}");
                    running.store(false, Ordering::SeqCst);
                }
            }
        }
    })
}

fn select_input_device(host: &cpal::Host, configured_name: Option<&str>) -> Result<Device> {
    if let Some(configured_name) = configured_name {
        if let Some(device) = find_device_by_name(host.input_devices()?, configured_name) {
            return Ok(device);
        }
        eprintln!("eqmacd: configured input device not found, falling back: {configured_name}");
    }

    for device in host.input_devices()? {
        let is_loopback = device
            .name()
            .map(|name| eq_mac_cli::commands::audio::is_likely_system_audio_input(&name))
            .unwrap_or(false);
        if is_loopback {
            return Ok(device);
        }
    }

    bail!(
        "no loopback/system-audio input device detected in automatic mode; install and route audio through BlackHole 2ch or set an explicit input device"
    )
}

fn select_output_device(host: &cpal::Host, configured_name: Option<&str>) -> Result<Device> {
    if let Some(configured_name) = configured_name {
        if let Some(device) = find_device_by_name(host.output_devices()?, configured_name) {
            return Ok(device);
        }
        eprintln!("eqmacd: configured output device not found, falling back: {configured_name}");
    }

    let mut outputs = host.output_devices()?;
    outputs
        .next()
        .or_else(|| host.default_output_device())
        .ok_or_else(|| anyhow!("no output device available"))
}

fn find_device_by_name<I>(devices: I, target: &str) -> Option<Device>
where
    I: Iterator<Item = Device>,
{
    devices
        .into_iter()
        .find(|device| device.name().map(|name| name == target).unwrap_or(false))
}

fn select_output_config(
    device: &Device,
    sample_rate: SampleRate,
) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_output_config()?;
    if default.sample_rate() == sample_rate {
        return Ok(default);
    }

    let mut preferred = None;
    let mut fallback = None;
    for config in device.supported_output_configs()? {
        if config.min_sample_rate() <= sample_rate && config.max_sample_rate() >= sample_rate {
            let selected = config.with_sample_rate(sample_rate);
            if selected.channels() == 2 {
                preferred = Some(selected);
                break;
            }
            if fallback.is_none() {
                fallback = Some(selected);
            }
        }
    }

    preferred
        .or(fallback)
        .ok_or_else(|| anyhow!("could not find output config matching {}Hz", sample_rate.0))
}

fn build_input_stream<P>(
    device: &Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    target_channels: usize,
    producer: P,
) -> Result<Stream>
where
    P: Producer<Item = f32> + Send + 'static,
{
    let err_fn = |err| eprintln!("eqmacd input stream error: {err}");
    match sample_format {
        SampleFormat::F32 => {
            build_input_stream_inner::<f32, P>(device, &config, target_channels, producer, err_fn)
        }
        SampleFormat::I16 => {
            build_input_stream_inner::<i16, P>(device, &config, target_channels, producer, err_fn)
        }
        SampleFormat::U16 => {
            build_input_stream_inner::<u16, P>(device, &config, target_channels, producer, err_fn)
        }
        other => bail!("unsupported input sample format: {other:?}"),
    }
}

fn build_input_stream_inner<T, P>(
    device: &Device,
    config: &StreamConfig,
    target_channels: usize,
    mut producer: P,
    err_fn: fn(cpal::StreamError),
) -> Result<Stream>
where
    T: SizedSample,
    P: Producer<Item = f32> + Send + 'static,
{
    let input_channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            for frame in data.chunks(input_channels) {
                for target_channel in 0..target_channels {
                    let source_index = if input_channels == 1 {
                        0
                    } else {
                        target_channel.min(input_channels - 1)
                    };
                    let value = sample_to_f32(&frame[source_index]);
                    let _ = producer.try_push(value);
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn build_output_stream<C>(
    device: &Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    consumer: C,
    state: Arc<SharedState>,
) -> Result<Stream>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let err_fn = |err| eprintln!("eqmacd output stream error: {err}");
    match sample_format {
        SampleFormat::F32 => {
            build_output_stream_inner::<f32, C>(device, &config, consumer, state, err_fn)
        }
        SampleFormat::I16 => {
            build_output_stream_inner::<i16, C>(device, &config, consumer, state, err_fn)
        }
        SampleFormat::U16 => {
            build_output_stream_inner::<u16, C>(device, &config, consumer, state, err_fn)
        }
        other => bail!("unsupported output sample format: {other:?}"),
    }
}

fn build_output_stream_inner<T, C>(
    device: &Device,
    config: &StreamConfig,
    mut consumer: C,
    state: Arc<SharedState>,
    err_fn: fn(cpal::StreamError),
) -> Result<Stream>
where
    T: SizedSample,
    C: Consumer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;
    let mut last_revision = u64::MAX;
    let mut local_chain = DspChain::bypass(channels);
    let mut last_frame = vec![0.0_f32; channels];

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let revision = state.revision.load(Ordering::SeqCst);
            if revision != last_revision {
                if let Ok(chain) = state.chain.read() {
                    local_chain = chain.clone();
                }
                last_revision = revision;
            }

            for frame in data.chunks_mut(channels) {
                let mut samples = vec![0.0_f32; channels];
                for (index, sample) in samples.iter_mut().enumerate() {
                    if let Some(value) = consumer.try_pop() {
                        *sample = value;
                        last_frame[index] = value;
                    } else {
                        *sample = last_frame[index];
                    }
                }

                local_chain.process_frame(&mut samples);

                for (slot, sample) in frame.iter_mut().zip(samples.iter()) {
                    *slot = f32_to_sample::<T>(*sample);
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn sample_to_f32<T>(sample: &T) -> f32
where
    T: SizedSample,
{
    match T::FORMAT {
        SampleFormat::F32 => unsafe { *(sample as *const T as *const f32) },
        SampleFormat::I16 => {
            let value = unsafe { *(sample as *const T as *const i16) };
            value as f32 / i16::MAX as f32
        }
        SampleFormat::U16 => {
            let value = unsafe { *(sample as *const T as *const u16) };
            (value as f32 / u16::MAX as f32) * 2.0 - 1.0
        }
        _ => 0.0,
    }
}

fn f32_to_sample<T>(sample: f32) -> T
where
    T: SizedSample,
{
    let clamped = sample.clamp(-1.0, 1.0);
    match T::FORMAT {
        SampleFormat::F32 => unsafe { std::mem::transmute_copy(&clamped) },
        SampleFormat::I16 => {
            let value = (clamped * i16::MAX as f32).round() as i16;
            unsafe { std::mem::transmute_copy(&value) }
        }
        SampleFormat::U16 => {
            let value = (((clamped + 1.0) * 0.5) * u16::MAX as f32).round() as u16;
            unsafe { std::mem::transmute_copy(&value) }
        }
        _ => unreachable!(),
    }
}
