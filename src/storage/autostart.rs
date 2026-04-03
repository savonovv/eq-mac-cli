use anyhow::Result;
use std::path::{Path, PathBuf};

const LABEL: &str = "com.savonovv.eq-mac-cli.eqmacd";

pub fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory not found"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

pub fn is_enabled() -> Result<bool> {
    Ok(plist_path()?.exists())
}

pub fn render_plist(daemon_path: &Path, data_dir: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>WorkingDirectory</key>
  <string>{}</string>
</dict>
</plist>
"#,
        daemon_path.display(),
        data_dir.display()
    )
}
