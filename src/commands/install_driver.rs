use anyhow::{Result, bail};
use std::io::{self, Write};
use std::process::Command;

pub fn run() -> Result<()> {
    print!("install blackhole-2ch with Homebrew? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_lowercase();

    if answer != "y" && answer != "yes" {
        println!("aborted");
        return Ok(());
    }

    let status = Command::new("brew")
        .args(["install", "--cask", "blackhole-2ch"])
        .status()?;

    if !status.success() {
        bail!("brew install --cask blackhole-2ch failed");
    }

    println!("installed blackhole-2ch; logout/login or reboot may be required");
    Ok(())
}
