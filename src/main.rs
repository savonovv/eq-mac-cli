use anyhow::Result;
use clap::Parser;
use eq_mac_cli::cli::{Cli, Commands};
use eq_mac_cli::commands;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => commands::interactive::run(),
        Some(Commands::Interactive) => commands::interactive::run(),
        Some(Commands::I) => commands::interactive::run(),
        Some(Commands::Template) => commands::template::run(),
        Some(Commands::Add { file, text, name }) => commands::add::run(file, text, name),
        Some(Commands::List) => commands::list::run(),
        Some(Commands::Ls) => commands::list::run(),
        Some(Commands::L) => commands::list::run(),
        Some(Commands::Show { selector }) => commands::show::run(selector),
        Some(Commands::Enable { selector }) => commands::enable::run(selector),
        Some(Commands::Disable) => commands::disable::run(),
        Some(Commands::Delete { selector }) => commands::delete::run(selector),
        Some(Commands::Rename { selector, name }) => commands::rename::run(selector, name),
        Some(Commands::Status) => commands::status::run(),
        Some(Commands::Doctor) => commands::doctor::run(),
        Some(Commands::InstallDriver) => commands::install_driver::run(),
        Some(Commands::Audio { command }) => commands::audio::run(command),
        Some(Commands::Autostart { command }) => commands::autostart::run(command),
        Some(Commands::Daemon { command }) => commands::daemon::run(command),
    }
}
