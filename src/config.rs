use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppTheme {
    #[default]
    #[serde(alias = "Auto")]
    System,
    #[serde(alias = "Modern")]
    Dark,
    #[serde(alias = "Classic")]
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub theme: AppTheme,
    #[serde(default = "default_true")]
    pub calculate_hashes: bool,
    #[serde(default)]
    pub auto_copy_csv_on_inspect: bool,
    #[serde(default)]
    pub export_dir: String,
    #[serde(default = "default_max_recent")]
    pub max_recent_files: usize,
}

fn default_true() -> bool {
    true
}

fn default_max_recent() -> usize {
    20
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_export_dir = dirs::download_dir()
            .or_else(|| dirs::document_dir())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .to_string_lossy()
            .to_string();

        Self {
            theme: AppTheme::Dark,
            calculate_hashes: true,
            auto_copy_csv_on_inspect: false,
            export_dir: default_export_dir,
            max_recent_files: 20,
        }
    }
}

impl AppConfig {
    pub fn config_file_path() -> PathBuf {
        config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                return cfg;
            }
        }
        let default = Self::default();
        let _ = default.save();
        default
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(roaming) = dirs::data_dir() {
            return roaming.join("mta");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join("Library/Application Support/mta");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(data) = dirs::data_dir() {
            return data.join("mta");
        }
    }

    PathBuf::from(".mta")
}
