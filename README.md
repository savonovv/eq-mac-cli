# eq-mac-cli

[![Platform](https://img.shields.io/badge/platform-macOS-black)](https://www.apple.com/macos/)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/savonovv/eq-mac-cli)](./LICENSE)
[![GitHub repo](https://img.shields.io/badge/github-eq--mac--cli-181717?logo=github)](https://github.com/savonovv/eq-mac-cli)

`eq-mac-cli` is a free, highly customizable EQ workflow for macOS built for people who want more control than the typical paid GUI app gives them.

It gives you a scriptable, editable, plain-text EQ system on macOS without hiding your filters behind a closed UI or charging you for basic control. Your presets are real text files, use familiar APO-style filter lines, can be edited in the terminal, and can be reused, versioned, and tuned as aggressively as you want.

If you have looked for a macOS EQ tool that is both deeply customizable and free, that is exactly the gap this project is trying to fill.

It is built around a simple model:

1. macOS playback is routed to a loopback device such as `BlackHole 2ch`
2. `eqmacd` reads that playback stream
3. `eqmacd` applies preamp + EQ filters in real time
4. `eqmacd` sends processed audio to your real output device such as headphones or speakers

This project is focused on being:

1. terminal-first
2. preset-based
3. macOS-native enough to autostart with `launchd`
4. simple to control from an interactive TUI
5. open and editable because presets live as plain text instead of a hidden database

## What It Does

1. Stores presets as plain text under `~/.local/share/eq-mac-cli`
2. Assigns simple numeric preset IDs
3. Enables one preset at a time or disables EQ entirely
4. Runs a background daemon for live audio processing
5. Supports login autostart
6. Provides an interactive selector UI with keyboard navigation

## What It Does Not Do

1. It does not work without a loopback/system-audio input device
2. It does not directly modify your hardware headphone EQ or DSP
3. It does not replace macOS sound routing by itself; you still need to route system output to `BlackHole 2ch`

## Quick Start

### 1. Install

Homebrew tap:

```bash
brew tap savonovv/eq-mac-cli
brew install savonovv/eq-mac-cli/eq-mac-cli
```

Or build/install locally:

```bash
cargo install --path . --force
```

This installs:

```bash
~/.cargo/bin/eqcli
~/.cargo/bin/eqmacd
```

### 2. Install BlackHole

```bash
eqcli install-driver
```

or directly:

```bash
brew install --cask blackhole-2ch
```

Important:

1. after installing `BlackHole 2ch`, log out and back in or reboot if macOS has not refreshed audio devices yet

### 3. Verify device visibility

```bash
eqcli doctor
eqcli audio list
```

You want to see `BlackHole 2ch` available as an input device.

### 4. Add a preset

From a file:

```bash
eqcli add --file examples/sample-preset.txt --name "My EQ"
```

From inline text:

```bash
eqcli add --name samsung --text 'Preamp: -5 dB
Filter 1: ON LS Fc 28 Hz Gain 3.26 dB Q 0.92
Filter 2: ON PK Fc 223 Hz Gain -2.99 dB Q 0.41'
```

From the built-in creator:

```bash
eqcli
```

Then:

1. open `Presets`
2. choose `Add new preset`
3. choose `Create my own`
4. type a preset name
5. tune filters live while listening

### 5. Choose the real output device

```bash
eqcli audio use-output "External Headphones"
```

### 6. Route macOS playback to BlackHole

In macOS Sound settings:

1. set system output device to `BlackHole 2ch`

Then in `eqcli`:

1. leave output device as your real headphones or speakers

The expected chain is:

```text
Music/App -> BlackHole 2ch -> eqmacd -> External Headphones
```

### 7. Enable your preset

```bash
eqcli enable 1
```

### 8. Compare EQ on/off

```bash
eqcli disable
eqcli enable 1
```

If routing is correct, you should hear the difference immediately.

## Interactive Mode

Run the app with no arguments:

```bash
eqcli
```

or explicitly:

```bash
eqcli interactive
eqcli i
```

Keyboard controls:

1. `j` / `k` or arrow keys: move
2. `Enter` or `l`: select
3. `h` or `Esc`: go back
4. `d` or `Delete`: delete the selected preset with confirmation
5. `e`: edit the selected saved preset
6. `r`: rename the selected saved preset
7. `s`: show the selected preset config, or the active preset if nothing is selected
8. `q`: quit

Interactive flow:

1. `Presets`
2. `None (disable EQ)` to bypass processing
3. `Add new preset`
4. choose `From file`, `Paste text`, or `Create my own`
5. pick a saved preset to enable it
6. `Output device` to choose where processed audio goes

The interactive menu intentionally hides low-level process details. You do not need to think about the daemon to use the app normally.

Editor workflow:

1. `Create my own` opens a live editor for a brand new preset
2. `e` edits an existing preset from the presets list
3. filter changes can be auditioned live while creating your own preset
4. `s` saves and restarts the daemon

## CLI Commands

### Presets

```bash
eqcli list
eqcli ls
eqcli l
eqcli show 1
eqcli add --file preset.txt --name "Example"
eqcli add --text 'Preamp: -2.5 dB ...' --name "Example"
eqcli rename 1 --name "New Name"
eqcli delete 1
eqcli enable 1
eqcli disable
```

Interactive-only actions:

1. create your own preset from scratch
2. edit an existing preset with `e`
3. tune filters directly in the TUI

### Audio output

```bash
eqcli audio list
eqcli audio use-output "External Headphones"
eqcli audio reset
```

`audio reset` returns device selection to automatic mode.

### Daemon and diagnostics

```bash
eqcli status
eqcli doctor
eqcli daemon start
eqcli daemon stop
eqcli daemon restart
```

### Autostart

```bash
eqcli autostart enable
eqcli autostart disable
```

When autostart is enabled:

1. the daemon starts on login
2. the last active preset is restored automatically
3. if EQ was disabled, the daemon starts in bypass mode

## Preset Format

Supported import format:

```text
Preamp: -2.5 dB
Filter 1: ON LS Fc 28 Hz Gain 2.2 dB Q 0.917
Filter 2: ON PK Fc 223 Hz Gain -6.6 dB Q 0.412
Filter 3: ON PK Fc 791 Hz Gain 2.4 dB Q 1.277
Filter 4: ON HS Fc 8000 Hz Gain 4 dB Q 0.707
Filter 5: ON HP Fc 80 Hz Q 0.707
Filter 6: ON LP Fc 16000 Hz Q 0.707
```

Supported filter types:

1. `LS`
2. `LSC`
3. `HS`
4. `HSC`
5. `HP`
6. `LP`
7. `PK`

Supported Equalizer APO-style lines currently expect `Fc`, `Q`, and:

1. `Gain` for shelf and peak filters
2. no `Gain` for `HP` and `LP`

This is broader Equalizer APO filter support, but not the full Equalizer APO grammar yet. The parser is still intentionally focused on common AutoEQ / Equalizer APO text lines rather than every EAPO feature.

## Storage

All app state lives under:

```bash
~/.local/share/eq-mac-cli
```

Important files:

1. `presets/<preset-name>.txt`
2. `index.txt`
3. `active.txt`
4. `config.txt`
5. `runtime/daemon.log`
6. `runtime/daemon.sock`
7. `runtime/daemon.pid`

## BlackHole Setup Notes

### Why BlackHole is needed

`eqmacd` must receive system playback audio from a loopback device. On macOS, `BlackHole 2ch` is the intended default for that.

Without a loopback device:

1. the app cannot process music/app playback
2. it should fail instead of silently monitoring your microphone

### Correct routing

Correct:

```text
macOS output device = BlackHole 2ch
eqcli output device = External Headphones
```

Wrong:

```text
macOS output device = External Headphones
```

In the wrong setup, your apps bypass the EQ daemon entirely.

### If BlackHole does not appear

1. confirm it is installed:

```bash
brew list --cask blackhole-2ch
```

2. log out/in or reboot
3. run:

```bash
eqcli audio list
eqcli doctor
```

## Troubleshooting

### I still hear my microphone

That means your playback chain is not routed through a system loopback device. The app should now avoid mic fallback in automatic mode, but if routing is wrong you may still be testing the wrong path.

Check:

1. macOS output device is `BlackHole 2ch`
2. `eqcli doctor` sees a loopback input
3. `eqcli audio use-output` points to your real headphones or speakers

### I hear no difference between `enable` and `disable`

Usually this means:

1. macOS output is not actually set to `BlackHole 2ch`
2. the daemon is not running
3. the active preset is not the one you expect

Check:

```bash
eqcli status
tail -40 ~/.local/share/eq-mac-cli/runtime/daemon.log
```

### The routed audio is quieter

That can happen because:

1. your preset has negative preamp
2. the real output device keeps its own volume state
3. the loopback bridge and hardware output are separate gain stages

Try:

1. raising headphone hardware volume
2. testing with `eqcli disable`
3. testing with a preset that uses `Preamp: 0 dB`

## Development

Build:

```bash
cargo build
```

Run from source:

```bash
cargo run --bin eqcli --
```

Install local build:

```bash
cargo install --path . --force
```

## Roadmap

1. Better routing diagnostics and setup checks
2. More robust inter-device clock handling
3. Output/master gain control
4. Better in-TUI text input flows
5. GitHub release automation
6. Homebrew tap and package
