use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "eqcli")]
#[command(about = "Manage EQ presets and daemon state for macOS audio routing")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Interactive,
    I,
    Template,
    Add {
        #[arg(long, conflicts_with = "text")]
        file: Option<PathBuf>,
        #[arg(long, conflicts_with = "file")]
        text: Option<String>,
        #[arg(long)]
        name: String,
    },
    List,
    Ls,
    L,
    Show {
        selector: String,
    },
    Enable {
        selector: String,
    },
    Disable,
    Delete {
        selector: String,
    },
    Rename {
        selector: String,
        #[arg(long)]
        name: String,
    },
    Status,
    Doctor,
    InstallDriver,
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    Autostart {
        #[command(subcommand)]
        command: AutostartCommand,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand, Clone, Copy)]
pub enum AutostartCommand {
    Enable,
    Disable,
}

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommand {
    Start,
    Stop,
    Restart,
}

#[derive(Subcommand, Clone)]
pub enum AudioCommand {
    List,
    UseInput { name: String },
    UseOutput { name: String },
    Reset,
}
