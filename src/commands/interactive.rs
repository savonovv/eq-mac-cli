use crate::commands;
use crate::eq::editor::EditorState;
use crate::eq::parser;
use crate::storage::{active, autostart, config, daemon_state, index, presets};
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn run() -> Result<()> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = App::new();

    loop {
        app.refresh()?;
        render(&mut terminal.stdout, &app)?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if handle_key(&mut terminal, &mut app, key.code)? {
                    break;
                }
            }
        }
    }

    Ok(())
}

struct TerminalSession {
    stdout: io::Stdout,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self { stdout })
    }

    fn suspend(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.stdout, LeaveAlternateScreen)?;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        execute!(self.stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.stdout, LeaveAlternateScreen);
    }
}

#[derive(Debug, Clone)]
enum Screen {
    Main,
    Presets,
    AddPreset,
    Outputs,
    ConfirmDelete,
    EditPreset,
}

struct App {
    screen: Screen,
    selected: usize,
    message: String,
    overview: Overview,
    presets: Vec<index::IndexEntry>,
    outputs: Vec<String>,
    delete_target: Option<index::IndexEntry>,
    editor: Option<EditorState>,
    editor_target: Option<u32>,
    editor_live_preview: bool,
    editor_created_here: bool,
    editor_previous_active: Option<u32>,
    frequency_input: String,
    frequency_input_started_at: Option<Instant>,
}

#[derive(Default)]
struct Overview {
    active_preset: String,
    daemon_running: bool,
    output_device: String,
    autostart_enabled: bool,
    routing_warning: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Main,
            selected: 0,
            message: "Ready".to_string(),
            overview: Overview::default(),
            presets: Vec::new(),
            outputs: Vec::new(),
            delete_target: None,
            editor: None,
            editor_target: None,
            editor_live_preview: false,
            editor_created_here: false,
            editor_previous_active: None,
            frequency_input: String::new(),
            frequency_input_started_at: None,
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.presets = index::read_entries()?;
        self.outputs = commands::audio::available_output_devices()?;

        self.overview.active_preset = match active::read_active_id()? {
            Some(id) => {
                let entry = index::resolve_id(id)?;
                format!("{} ({})", entry.id, entry.name)
            }
            None => "None".to_string(),
        };
        self.overview.daemon_running = daemon_state::is_running()?;
        let cfg = config::read_config()?;
        self.overview.output_device = cfg.output_device.unwrap_or_else(|| "automatic".to_string());
        self.overview.autostart_enabled = autostart::is_enabled()?;
        self.overview.routing_warning = if !commands::audio::has_system_audio_input()? {
            Some(
                "warning: no system-audio loopback input is available; EQ will not affect playback"
                    .to_string(),
            )
        } else {
            None
        };

        let max = self.current_items_len().saturating_sub(1);
        if self.selected > max {
            self.selected = max;
        }

        Ok(())
    }

    fn current_items_len(&self) -> usize {
        match self.screen {
            Screen::Main => 5,
            Screen::Presets => 2 + self.presets.len(),
            Screen::AddPreset => 4,
            Screen::Outputs => 1 + self.outputs.len(),
            Screen::ConfirmDelete => 2,
            Screen::EditPreset => self
                .editor
                .as_ref()
                .map(|editor| editor.filters.len())
                .unwrap_or(0),
        }
    }

    fn move_up(&mut self) {
        if self.selected == 0 {
            self.selected = self.current_items_len().saturating_sub(1);
        } else {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.current_items_len() == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1) % self.current_items_len();
        }
    }
}

fn keybind_agenda(screen: &Screen) -> &'static str {
    match screen {
        Screen::Main => "Keys: j/k or arrows move, Enter/l select, q quit",
        Screen::Presets => {
            "Keys: j/k or arrows move, Enter/l select, e edit, r rename, d delete, s show, h/Esc back, q quit"
        }
        Screen::AddPreset => "Keys: j/k or arrows move, Enter/l select, h/Esc back, q quit",
        Screen::Outputs => "Keys: j/k or arrows move, Enter/l select, h/Esc back, q quit",
        Screen::ConfirmDelete => "Keys: j/k or arrows move, Enter/l confirm, h/Esc back, q quit",
        Screen::EditPreset => {
            "Keys: digits set freq, n/p bands, a add, d delete, space toggle, m mode, j/k +/-1Hz, +/- gain, Backspace gain=0, h/l q, s save+restart, e exit editor, q quit app"
        }
    }
}

fn render(stdout: &mut io::Stdout, app: &App) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    let (_, terminal_height) = size()?;
    let agenda_row = terminal_height.saturating_sub(2);
    let status_row = terminal_height.saturating_sub(1);
    let mut row = 0u16;

    draw_line(stdout, row, "eqcli")?;
    row += 2;
    draw_line(stdout, row, "Overview")?;
    row += 1;
    draw_line(
        stdout,
        row,
        &format!("  Active preset : {}", app.overview.active_preset),
    )?;
    row += 1;
    draw_line(
        stdout,
        row,
        &format!(
            "  Daemon        : {}",
            if app.overview.daemon_running {
                "running"
            } else {
                "stopped"
            }
        ),
    )?;
    row += 1;
    draw_line(
        stdout,
        row,
        &format!("  Output device : {}", app.overview.output_device),
    )?;
    row += 1;
    draw_line(
        stdout,
        row,
        &format!(
            "  Autostart     : {}",
            if app.overview.autostart_enabled {
                "enabled"
            } else {
                "disabled"
            }
        ),
    )?;
    row += 1;
    if let Some(warning) = &app.overview.routing_warning {
        draw_line(stdout, row, &format!("  {}", warning))?;
        row += 1;
    }
    row += 1;

    match app.screen {
        Screen::Main => {
            draw_line(stdout, row, "Main")?;
            row += 1;
            row = render_items(
                stdout,
                row,
                app,
                &[
                    "Presets",
                    "Output device",
                    "Install BlackHole driver",
                    "Toggle autostart",
                    "Quit",
                ],
                agenda_row.saturating_sub(1),
            )?;
        }
        Screen::Presets => {
            draw_line(stdout, row, "Presets")?;
            row += 1;
            let active_id = active::read_active_id()?;
            let mut items = vec![
                if active_id.is_none() {
                    "None (disable EQ) [active]".to_string()
                } else {
                    "None (disable EQ)".to_string()
                },
                "Add new preset".to_string(),
            ];
            for preset in &app.presets {
                let marker = if Some(preset.id) == active_id {
                    " [active]"
                } else {
                    ""
                };
                items.push(format!("{}: {}{}", preset.id, preset.name, marker));
            }
            let refs: Vec<&str> = items.iter().map(String::as_str).collect();
            row = render_items(stdout, row, app, &refs, agenda_row.saturating_sub(1))?;
        }
        Screen::AddPreset => {
            draw_line(stdout, row, "Add preset")?;
            row += 1;
            row = render_items(
                stdout,
                row,
                app,
                &["From file", "Paste text", "Create my own", "Back"],
                agenda_row.saturating_sub(1),
            )?;
        }
        Screen::Outputs => {
            draw_line(stdout, row, "Output devices")?;
            row += 1;
            let mut items = vec![if app.overview.output_device == "automatic" {
                "Automatic [selected]".to_string()
            } else {
                "Automatic".to_string()
            }];
            for device in &app.outputs {
                let marker = if app.overview.output_device == *device {
                    " [selected]"
                } else {
                    ""
                };
                items.push(format!("{}{}", device, marker));
            }
            let refs: Vec<&str> = items.iter().map(String::as_str).collect();
            row = render_items(stdout, row, app, &refs, agenda_row.saturating_sub(1))?;
        }
        Screen::ConfirmDelete => {
            let title = if let Some(target) = &app.delete_target {
                format!("Delete preset {} ({})?", target.id, target.name)
            } else {
                "Delete preset?".to_string()
            };
            draw_line(stdout, row, &title)?;
            row += 2;
            row = render_items(
                stdout,
                row,
                app,
                &["Yes, delete", "No, keep it"],
                agenda_row.saturating_sub(1),
            )?;
        }
        Screen::EditPreset => {
            if let Some(editor) = &app.editor {
                row = render_editor(stdout, row, editor, agenda_row.saturating_sub(1))?;
            }
        }
    }

    execute!(stdout, MoveTo(0, agenda_row), Clear(ClearType::CurrentLine))?;
    draw_line(stdout, agenda_row, keybind_agenda(&app.screen))?;
    execute!(stdout, MoveTo(0, status_row), Clear(ClearType::CurrentLine))?;
    draw_line(stdout, status_row, &format!("Status: {}", app.message))?;
    stdout.flush()?;
    Ok(())
}

fn render_items(
    stdout: &mut io::Stdout,
    start_row: u16,
    app: &App,
    items: &[&str],
    max_row: u16,
) -> Result<u16> {
    let mut row = start_row;
    for (index, item) in items.iter().enumerate() {
        if row > max_row {
            break;
        }
        let marker = if app.selected == index { ">" } else { " " };
        draw_item(
            stdout,
            row,
            &format!("{} {}", marker, item),
            app.selected == index,
        )?;
        row += 1;
    }
    Ok(row)
}

fn render_editor(
    stdout: &mut io::Stdout,
    start_row: u16,
    editor: &EditorState,
    max_row: u16,
) -> Result<u16> {
    let mut row = start_row;
    if row <= max_row {
        draw_line(stdout, row, &format!("Edit preset: {}", editor.preset_name))?;
        row += 1;
    }
    if row <= max_row {
        let selected = editor.selected_filter();
        let gain = if selected.kind.uses_gain() {
            format!("{} dB", format_value(selected.gain_db))
        } else {
            "n/a".to_string()
        };
        draw_line(
            stdout,
            row,
            &format!(
                "Selected: {}  Freq: {} Hz  Gain: {}  Q: {}  {}",
                selected.kind.token(),
                format_value(selected.frequency_hz),
                gain,
                format_value(selected.q),
                if selected.enabled { "ON" } else { "OFF" }
            ),
        )?;
        row += 2;
    }

    for (index, filter) in editor.filters.iter().enumerate() {
        if row > max_row {
            break;
        }
        let marker = if index == editor.selected_index {
            ">"
        } else {
            " "
        };
        let gain = if filter.kind.uses_gain() {
            format!(" {:>6} dB", format_value(filter.gain_db))
        } else {
            "         ".to_string()
        };
        let line = format!(
            "{} #{} {:<2} {:>7} Hz{} Q {:>5} {}",
            marker,
            index + 1,
            filter.kind.token(),
            format_value(filter.frequency_hz),
            gain,
            format_value(filter.q),
            if filter.enabled { "ON" } else { "OFF" }
        );
        draw_item(stdout, row, &line, index == editor.selected_index)?;
        row += 1;
    }

    Ok(row)
}

fn format_value(value: f32) -> String {
    if value.fract().abs() < 0.001 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

fn draw_line(stdout: &mut io::Stdout, row: u16, text: &str) -> Result<()> {
    execute!(stdout, MoveTo(0, row))?;
    write!(stdout, "{}", text)?;
    Ok(())
}

fn draw_item(stdout: &mut io::Stdout, row: u16, text: &str, selected: bool) -> Result<()> {
    execute!(stdout, MoveTo(0, row))?;
    if selected {
        execute!(
            stdout,
            SetForegroundColor(Color::Black),
            SetBackgroundColor(Color::Cyan),
            Print(text),
            ResetColor
        )?;
    } else {
        write!(stdout, "{}", text)?;
    }
    Ok(())
}

fn handle_key(terminal: &mut TerminalSession, app: &mut App, code: KeyCode) -> Result<bool> {
    if matches!(app.screen, Screen::EditPreset) {
        return handle_editor_key(app, code);
    }

    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('s') | KeyCode::Char('S') => {
            let selected_preset_id = selected_preset_id(app)?;
            run_modal(terminal, app, || show_preset_flow(selected_preset_id))?;
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            if matches!(app.screen, Screen::Presets) {
                begin_edit_preset(app)?;
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if matches!(app.screen, Screen::Presets) {
                let selected_preset_id = selected_saved_preset_id(app);
                run_modal(terminal, app, || rename_preset_flow(selected_preset_id))?;
            }
        }
        KeyCode::Delete | KeyCode::Char('d') => {
            if matches!(app.screen, Screen::Presets) {
                begin_delete_confirmation(app);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            if activate_current(terminal, app)? {
                return Ok(true);
            }
        }
        KeyCode::Left | KeyCode::Esc | KeyCode::Char('h') => {
            if !matches!(app.screen, Screen::Main) {
                app.screen = match app.screen {
                    Screen::AddPreset => Screen::Presets,
                    Screen::ConfirmDelete => Screen::Presets,
                    _ => Screen::Main,
                };
                app.selected = 0;
                app.message = "Back to main menu".to_string();
                if matches!(app.screen, Screen::Presets) {
                    app.delete_target = None;
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_editor_key(app: &mut App, code: KeyCode) -> Result<bool> {
    if let KeyCode::Char(ch) = code {
        if ch.is_ascii_digit() {
            apply_frequency_digit(app, ch)?;
            return Ok(false);
        }
    }

    let Some(editor) = app.editor.as_mut() else {
        app.screen = Screen::Presets;
        return Ok(false);
    };

    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('e') | KeyCode::Char('E') => {
            app.editor = None;
            if app.editor_created_here && !app.editor_live_preview {
                app.editor_target = None;
            }
            app.screen = Screen::Presets;
            app.selected = 0;
            app.message = "Exited editor".to_string();
            app.frequency_input.clear();
            app.frequency_input_started_at = None;
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            let text = editor.to_preset_text();
            let id = if let Some(id) = app.editor_target {
                presets::write_preset(id, &text)?;
                id
            } else {
                let id = index::next_id()?;
                presets::write_preset(id, &text)?;
                index::append_entry(id, &editor.preset_name)?;
                app.editor_target = Some(id);
                id
            };

            editor.dirty = false;
            match commands::daemon::run(crate::cli::DaemonCommand::Restart) {
                Ok(()) => app.message = format!("Saved preset {} and restarted daemon", id),
                Err(err) => {
                    app.message = format!("Saved preset {} but restart failed: {}", id, err)
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Down => editor.next_filter(),
        KeyCode::Char('p') | KeyCode::Up => editor.previous_filter(),
        KeyCode::Char('a') => editor.add_filter(),
        KeyCode::Char('d') | KeyCode::Delete => editor.delete_selected_filter(),
        KeyCode::Char(' ') => editor.toggle_selected_filter(),
        KeyCode::Char('m') => editor.cycle_mode(),
        KeyCode::Char('j') => editor.adjust_frequency(-1.0),
        KeyCode::Char('k') => editor.adjust_frequency(1.0),
        KeyCode::Char('-') => editor.adjust_gain(-0.1),
        KeyCode::Char('+') | KeyCode::Char('=') => editor.adjust_gain(0.1),
        KeyCode::Backspace => editor.reset_gain(),
        KeyCode::Char('h') | KeyCode::Left => editor.adjust_q(-0.05),
        KeyCode::Char('l') | KeyCode::Right => editor.adjust_q(0.05),
        _ => {}
    }

    maybe_live_apply_editor(app)?;

    if app.editor.as_ref().is_some_and(|state| state.dirty) {
        let dirty = app.editor.as_ref().unwrap().dirty;
        if dirty {
            let filter = app.editor.as_ref().unwrap().selected_filter();
            app.message = format!(
                "Editing {} {} Hz {}",
                filter.kind.token(),
                format_value(filter.frequency_hz),
                if filter.enabled { "ON" } else { "OFF" }
            );
        }
    }
    Ok(false)
}

fn begin_edit_preset(app: &mut App) -> Result<()> {
    let Some(id) = selected_saved_preset_id(app) else {
        app.message = "Select a saved preset to edit".to_string();
        return Ok(());
    };

    let entry = index::resolve_id(id)?;
    let raw = presets::read_raw_preset(id)?;
    let preset = parser::parse_preset(&raw, Some(entry.name.clone()))?;
    let editor = EditorState::from_preset(&preset)?;
    app.editor = Some(editor);
    app.editor_target = Some(id);
    app.editor_live_preview = false;
    app.editor_created_here = false;
    app.editor_previous_active = None;
    app.frequency_input.clear();
    app.frequency_input_started_at = None;
    app.screen = Screen::EditPreset;
    app.message = format!("Editing preset {}", entry.name);
    Ok(())
}

fn begin_create_preset(terminal: &mut TerminalSession, app: &mut App) -> Result<()> {
    let Some(name) = prompt_without_pause(terminal, "preset name")? else {
        app.message = "Preset creation cancelled".to_string();
        return Ok(());
    };

    let id = index::next_id()?;
    let editor = EditorState::new(name.clone());
    presets::write_preset(id, &editor.to_preset_text())?;
    index::append_entry(id, &name)?;
    let previous_active = active::read_active_id()?;
    let _ = commands::enable::run(id.to_string());

    app.editor = Some(editor);
    app.editor_target = Some(id);
    app.editor_live_preview = true;
    app.editor_created_here = true;
    app.editor_previous_active = previous_active;
    app.frequency_input.clear();
    app.frequency_input_started_at = None;
    app.screen = Screen::EditPreset;
    app.message = format!("Creating preset {}", name);
    Ok(())
}

fn apply_frequency_digit(app: &mut App, ch: char) -> Result<()> {
    let Some(editor) = app.editor.as_mut() else {
        return Ok(());
    };

    let now = Instant::now();
    let should_reset = app
        .frequency_input_started_at
        .map(|started| now.duration_since(started) > Duration::from_secs(2))
        .unwrap_or(true);
    if should_reset {
        app.frequency_input.clear();
    }

    app.frequency_input.push(ch);
    app.frequency_input_started_at = Some(now);

    if let Ok(value) = app.frequency_input.parse::<f32>() {
        let new_frequency = value.clamp(10.0, 22_000.0);
        let filter = editor.selected_filter_mut();
        filter.frequency_hz = new_frequency;
        let shown_frequency = filter.frequency_hz;
        editor.dirty = true;
        app.message = format!("Frequency input: {} Hz", format_value(shown_frequency));
        maybe_live_apply_editor(app)?;
    }

    Ok(())
}

fn maybe_live_apply_editor(app: &mut App) -> Result<()> {
    if !app.editor_live_preview {
        return Ok(());
    }

    let Some(id) = app.editor_target else {
        return Ok(());
    };
    let Some(editor) = app.editor.as_ref() else {
        return Ok(());
    };

    let text = editor.to_preset_text();
    presets::write_preset(id, &text)?;
    commands::daemon::ensure_started_with_state()?;
    let _ = commands::daemon::notify_reload();
    Ok(())
}

fn activate_current(terminal: &mut TerminalSession, app: &mut App) -> Result<bool> {
    match app.screen {
        Screen::Main => activate_main(terminal, app),
        Screen::Presets => {
            activate_presets(terminal, app)?;
            Ok(false)
        }
        Screen::AddPreset => {
            activate_add_preset(terminal, app)?;
            Ok(false)
        }
        Screen::Outputs => {
            activate_outputs(app)?;
            Ok(false)
        }
        Screen::ConfirmDelete => {
            activate_delete_confirmation(app)?;
            Ok(false)
        }
        Screen::EditPreset => Ok(false),
    }
}

fn activate_main(terminal: &mut TerminalSession, app: &mut App) -> Result<bool> {
    match app.selected {
        0 => {
            app.screen = Screen::Presets;
            app.selected = 0;
            app.message = "Select a preset, None, or Add new".to_string();
        }
        1 => {
            app.screen = Screen::Outputs;
            app.selected = 0;
            app.message = "Select an output device".to_string();
        }
        2 => run_modal(terminal, app, || {
            commands::install_driver::run()?;
            Ok("Driver installation flow finished".to_string())
        })?,
        3 => {
            if app.overview.autostart_enabled {
                run_action(
                    app,
                    commands::autostart::run(crate::cli::AutostartCommand::Disable),
                    "Autostart disabled",
                )?;
            } else {
                run_action(
                    app,
                    commands::autostart::run(crate::cli::AutostartCommand::Enable),
                    "Autostart enabled",
                )?;
            }
        }
        4 => return Ok(true),
        _ => {}
    }
    Ok(false)
}

fn activate_presets(_terminal: &mut TerminalSession, app: &mut App) -> Result<()> {
    if app.selected == 0 {
        run_action(app, commands::disable::run(), "EQ disabled")?;
        return Ok(());
    }

    if app.selected == 1 {
        app.screen = Screen::AddPreset;
        app.selected = 0;
        app.message = "Choose how to add the preset".to_string();
        return Ok(());
    }

    let preset_index = app.selected - 2;
    if let Some(preset) = app.presets.get(preset_index) {
        let selector = preset.id.to_string();
        let message = format!("Preset {} enabled", preset.name);
        run_action(app, commands::enable::run(selector), &message)?;
    }
    Ok(())
}

fn activate_add_preset(terminal: &mut TerminalSession, app: &mut App) -> Result<()> {
    match app.selected {
        0 => {
            run_modal(terminal, app, add_preset_from_file_flow)?;
            app.screen = Screen::Presets;
            app.selected = 0;
        }
        1 => {
            run_modal(terminal, app, add_preset_from_text_flow)?;
            app.screen = Screen::Presets;
            app.selected = 0;
        }
        2 => {
            begin_create_preset(terminal, app)?;
        }
        3 => {
            app.screen = Screen::Presets;
            app.selected = 0;
            app.message = "Back to presets".to_string();
        }
        _ => {}
    }
    Ok(())
}

fn begin_delete_confirmation(app: &mut App) {
    if app.selected < 2 {
        app.message = "Only saved presets can be deleted".to_string();
        return;
    }

    let preset_index = app.selected - 2;
    if let Some(preset) = app.presets.get(preset_index) {
        app.delete_target = Some(preset.clone());
        app.screen = Screen::ConfirmDelete;
        app.selected = 1;
        app.message = "Confirm deletion".to_string();
    }
}

fn activate_delete_confirmation(app: &mut App) -> Result<()> {
    match app.selected {
        0 => {
            if let Some(target) = &app.delete_target {
                let message = format!("Deleted preset {} ({})", target.id, target.name);
                run_action(app, commands::delete::run(target.id.to_string()), &message)?;
            }
            app.delete_target = None;
            app.screen = Screen::Presets;
            app.selected = 0;
        }
        1 => {
            app.delete_target = None;
            app.screen = Screen::Presets;
            app.selected = 0;
            app.message = "Deletion cancelled".to_string();
        }
        _ => {}
    }
    Ok(())
}

fn activate_outputs(app: &mut App) -> Result<()> {
    if app.selected == 0 {
        run_action(
            app,
            commands::audio::set_output(None),
            "Output device set to automatic",
        )?;
        return Ok(());
    }

    if let Some(name) = app.outputs.get(app.selected - 1) {
        let message = format!("Output device set to {name}");
        run_action(
            app,
            commands::audio::set_output(Some(name.clone())),
            &message,
        )?;
    }
    Ok(())
}

fn run_action(app: &mut App, result: Result<()>, success_message: &str) -> Result<()> {
    match result {
        Ok(()) => app.message = success_message.to_string(),
        Err(err) => app.message = format!("error: {err}"),
    }
    Ok(())
}

fn run_modal<F>(terminal: &mut TerminalSession, app: &mut App, action: F) -> Result<()>
where
    F: FnOnce() -> Result<String>,
{
    terminal.suspend()?;
    let result = action();
    println!();
    println!("Press Enter to return to eqcli...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    terminal.resume()?;

    match result {
        Ok(message) => app.message = message,
        Err(err) => app.message = format!("error: {err}"),
    }

    Ok(())
}

fn add_preset_from_file_flow() -> Result<String> {
    let name = prompt("preset name")?;
    let file = prompt("path to preset file")?;
    commands::add::run(Some(PathBuf::from(file)), None, name.clone())?;
    Ok(format!("Preset {name} added"))
}

fn add_preset_from_text_flow() -> Result<String> {
    let name = prompt("preset name")?;
    let text = prompt_multiline()?;
    commands::add::run(None, Some(text), name.clone())?;
    Ok(format!("Preset {name} added"))
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_string())
}

fn prompt_without_pause(terminal: &mut TerminalSession, label: &str) -> Result<Option<String>> {
    terminal.suspend()?;
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    terminal.resume()?;

    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed))
}

fn prompt_multiline() -> Result<String> {
    println!("paste preset text, then enter a single '.' line to finish:");
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "." {
            break;
        }
        lines.push(trimmed.to_string());
    }
    Ok(lines.join("\n"))
}

fn selected_preset_id(app: &App) -> Result<Option<u32>> {
    if matches!(app.screen, Screen::Presets) && app.selected >= 2 {
        let preset_index = app.selected - 2;
        if let Some(preset) = app.presets.get(preset_index) {
            return Ok(Some(preset.id));
        }
    }

    Ok(active::read_active_id()?)
}

fn selected_saved_preset_id(app: &App) -> Option<u32> {
    if matches!(app.screen, Screen::Presets) && app.selected >= 2 {
        let preset_index = app.selected - 2;
        return app.presets.get(preset_index).map(|preset| preset.id);
    }

    None
}

fn show_preset_flow(selected_preset_id: Option<u32>) -> Result<String> {
    let Some(id) = selected_preset_id else {
        println!("No preset selected or active.");
        return Ok("No preset to show".to_string());
    };

    let entry = index::resolve_id(id)?;
    let raw = presets::read_raw_preset(id)?;

    println!("Preset {} ({})", entry.id, entry.name);
    println!();
    println!("{}", raw);

    Ok(format!("Displayed preset {} ({})", entry.id, entry.name))
}

fn rename_preset_flow(selected_preset_id: Option<u32>) -> Result<String> {
    let Some(id) = selected_preset_id else {
        println!("No preset selected or active.");
        return Ok("No preset to rename".to_string());
    };

    let entry = index::resolve_id(id)?;
    println!("Renaming preset {} ({})", entry.id, entry.name);
    let name = prompt("new preset name")?;
    commands::rename::run(id.to_string(), name.clone())?;
    Ok(format!("Renamed preset {} to {}", id, name))
}
