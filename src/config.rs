use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub remember_clippings: usize,
    pub display_clippings: usize,
    pub start_at_startup: bool,
    pub record_history: bool,
    #[serde(default)]
    pub clear_history_flag: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            remember_clippings: 80,
            display_clippings: 80,
            start_at_startup: true,
            record_history: true,
            clear_history_flag: false,
        }
    }
}

impl Config {
    fn get_config_path() -> Option<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "openclip", "OpenClip") {
            let config_dir = proj_dirs.config_dir();
            if !config_dir.exists() {
                let _ = fs::create_dir_all(config_dir);
            }
            Some(config_dir.join("config.json"))
        } else {
            None
        }
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_config_path() {
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(config) = serde_json::from_str(&data) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::get_config_path() {
            if let Ok(data) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, data);
            }
        }
    }
}
