use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub font_name: String,
    pub max_rows: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            font_name: "Meiryo UI".to_string(),
            max_rows: 15,
        }
    }
}

impl Config {
    pub fn get_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        });
        path.push("Clipper");
        path.push("config.toml");
        path
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if !path.exists() {
            let default_config = Config::default();
            default_config.save();
            return default_config;
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                match toml::from_str::<Config>(&content) {
                    Ok(config) => config,
                    Err(_) => {
                        let mut backup_path = path.clone();
                        backup_path.set_extension("toml.bak");
                        let _ = fs::rename(&path, &backup_path);
                        
                        let default_config = Config::default();
                        default_config.save();
                        default_config
                    }
                }
            }
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::get_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = toml::to_string(self) {
            let _ = fs::write(&path, content);
        }
    }
}
