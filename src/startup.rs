use anyhow::Result;

const APP_NAME: &str = "isclaude2x-tray";

/// Check whether the app is registered to launch at login.
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

/// Enable or disable launch-at-login.
pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled {
        platform::enable()
    } else {
        platform::disable()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::APP_NAME;
    use anyhow::{Context, Result};
    use std::process::Command;

    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn is_enabled() -> bool {
        Command::new("reg")
            .args(["query", RUN_KEY, "/v", APP_NAME])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("cannot resolve exe path")?;
        let exe_str = exe.display().to_string();
        let status = Command::new("reg")
            .args([
                "add", RUN_KEY, "/v", APP_NAME, "/t", "REG_SZ", "/d", &exe_str, "/f",
            ])
            .output()
            .context("failed to run reg add")?;
        if !status.status.success() {
            anyhow::bail!("reg add failed: {}", String::from_utf8_lossy(&status.stderr));
        }
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let status = Command::new("reg")
            .args(["delete", RUN_KEY, "/v", APP_NAME, "/f"])
            .output()
            .context("failed to run reg delete")?;
        if !status.status.success() {
            // If the key doesn't exist, that's fine.
            let stderr = String::from_utf8_lossy(&status.stderr);
            if !stderr.contains("unable to find") && !stderr.contains("The system was unable") {
                anyhow::bail!("reg delete failed: {stderr}");
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::APP_NAME;
    use anyhow::{Context, Result};
    use std::fs;
    use std::path::PathBuf;

    fn plist_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("cannot determine home directory")?;
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("com.{APP_NAME}.plist")))
    }

    pub fn is_enabled() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("cannot resolve exe path")?;
        let path = plist_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create LaunchAgents dir")?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.{APP_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            exe.display()
        );
        fs::write(&path, plist).context("failed to write LaunchAgent plist")?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let path = plist_path()?;
        if path.exists() {
            fs::remove_file(&path).context("failed to remove LaunchAgent plist")?;
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    use anyhow::Result;

    pub fn is_enabled() -> bool {
        false
    }

    pub fn enable() -> Result<()> {
        anyhow::bail!("launch-at-startup is not supported on this platform")
    }

    pub fn disable() -> Result<()> {
        Ok(())
    }
}
