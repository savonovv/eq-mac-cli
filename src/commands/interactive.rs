use crate::commands;
use crate::storage::{active, autostart, config, daemon_state, index, presets};
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

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
}

struct App {
    screen: Screen,
    selected: usize,
    message: String,
    overview: Overview,
    presets: Vec<index::IndexEntry>,
    outputs: Vec<String>,
    delete_target: Option<index::IndexEntry>,
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
            message: "j/k or arrows to move, Enter/l to select, h/Esc to go back, d to delete preset, s to show preset, q to quit"
                .to_string(),
            overview: Overview::default(),
            presets: Vec::new(),
            outputs: Vec::new(),
            delete_target: None,
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
            Screen::AddPreset => 3,
            Screen::Outputs => 1 + self.outputs.len(),
            Screen::ConfirmDelete => 2,
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

fn render(stdout: &mut io::Stdout, app: &App) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
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
            row = render_items(stdout, row, app, &refs)?;
        }
        Screen::AddPreset => {
            draw_line(stdout, row, "Add preset")?;
            row += 1;
            row = render_items(stdout, row, app, &["From file", "Paste text", "Back"])?;
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
            row = render_items(stdout, row, app, &refs)?;
        }
        Screen::ConfirmDelete => {
            let title = if let Some(target) = &app.delete_target {
                format!("Delete preset {} ({})?", target.id, target.name)
            } else {
                "Delete preset?".to_string()
            };
            draw_line(stdout, row, &title)?;
            row += 2;
            row = render_items(stdout, row, app, &["Yes, delete", "No, keep it"])?;
        }
    }

    row += 1;
    draw_line(stdout, row, &app.message)?;
    stdout.flush()?;
    Ok(())
}

fn render_items(stdout: &mut io::Stdout, start_row: u16, app: &App, items: &[&str]) -> Result<u16> {
    let mut row = start_row;
    for (index, item) in items.iter().enumerate() {
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
    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('s') | KeyCode::Char('S') => {
            let selected_preset_id = selected_preset_id(app)?;
            run_modal(terminal, app, || show_preset_flow(selected_preset_id))?;
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
