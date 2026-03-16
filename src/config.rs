use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const APP_DIR_NAME: &str = "isclaude2x-tray";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub sound: bool,
    /// Minutes before 2x ends to send a notification.
    pub before_end_minutes: Vec<u32>,
    /// Minutes before 2x starts to send a notification.
    pub before_start_minutes: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub preset: ThemePreset,
    /// Optional hex color override for the accent, e.g. "#D28F69".
    pub accent_hex: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreset {
    ClaudeDark,
    ClaudeLight,
    Midnight,
    Sunset,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            preset: ThemePreset::ClaudeDark,
            accent_hex: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            notifications: NotificationConfig {
                enabled: true,
                sound: true,
                before_end_minutes: vec![5, 15],
                before_start_minutes: vec![5, 15],
            },
            theme: ThemeConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config from {}", path.display()))?;
            let config: AppConfig = toml::from_str(&contents)
                .with_context(|| format!("failed to parse config from {}", path.display()))?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {}", parent.display()))?;
        }
        let contents =
            toml::to_string_pretty(self).context("failed to serialize config to TOML")?;
        fs::write(&path, contents)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine config directory")?;
    Ok(base.join(APP_DIR_NAME).join(CONFIG_FILE_NAME))
}
