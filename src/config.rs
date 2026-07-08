use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Auto,
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Auto
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub font_name: String,
    pub max_rows: usize,
    #[serde(default = "default_max_history")]
    pub max_history: usize,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_double_tap_ms")]
    pub double_tap_ms: u32,
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    #[serde(default = "default_theme_mode")]
    pub theme_mode: ThemeMode,
    #[serde(default = "default_snippet_key")]
    pub snippet_key: String,
    #[serde(default = "default_history_key")]
    pub history_key: String,
    #[serde(default = "default_exclude_apps")]
    pub exclude_apps: Vec<String>,
}

fn default_max_history() -> usize {
    1000
}

fn default_width() -> f32 {
    380.0
}

fn default_double_tap_ms() -> u32 {
    300
}

fn default_save_history() -> bool {
    true
}

fn default_theme_mode() -> ThemeMode {
    ThemeMode::Auto
}

fn default_snippet_key() -> String {
    "left_shift".to_string()
}

fn default_history_key() -> String {
    "left_ctrl".to_string()
}

fn default_exclude_apps() -> Vec<String> {
    vec![
        "1Password.exe".to_string(),
        "Bitwarden.exe".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Config {
            font_name: "Meiryo UI".to_string(),
            max_rows: 15,
            max_history: 1000,
            width: 380.0,
            double_tap_ms: 300,
            save_history: true,
            theme_mode: ThemeMode::Auto,
            snippet_key: "left_shift".to_string(),
            history_key: "left_ctrl".to_string(),
            exclude_apps: default_exclude_apps(),
        }
    }
}

impl Config {
    pub fn get_path() -> PathBuf {
        let mut path = if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data)
        } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
            PathBuf::from(user_profile)
        } else {
            PathBuf::from(".")
        };
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
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(config) => config,
                Err(_) => {
                    let mut backup_path = path.clone();
                    backup_path.set_extension("toml.bak");
                    let _ = fs::rename(&path, &backup_path);

                    let default_config = Config::default();
                    default_config.save();
                    default_config
                }
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
            font_name = "Segoe UI"
            max_rows = 10
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_name, "Segoe UI");
        assert_eq!(config.max_rows, 10);
        assert_eq!(config.max_history, 1000);
        assert_eq!(config.width, 380.0);
        assert_eq!(config.double_tap_ms, 300);
        assert_eq!(config.save_history, true);
        assert_eq!(config.theme_mode, ThemeMode::Auto);
        assert_eq!(config.snippet_key, "left_shift");
        assert_eq!(config.history_key, "left_ctrl");
    }

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
            font_name = "Segoe UI"
            max_rows = 20
            max_history = 500
            width = 450.0
            double_tap_ms = 300
            save_history = false
            theme_mode = "dark"
            snippet_key = "shift"
            history_key = "ctrl"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_name, "Segoe UI");
        assert_eq!(config.max_rows, 20);
        assert_eq!(config.max_history, 500);
        assert_eq!(config.width, 450.0);
        assert_eq!(config.double_tap_ms, 300);
        assert_eq!(config.save_history, false);
        assert_eq!(config.theme_mode, ThemeMode::Dark);
        assert_eq!(config.snippet_key, "shift");
        assert_eq!(config.history_key, "ctrl");
    }

    #[test]
    fn test_parse_exclude_apps() {
        let minimal_toml = r#"
            font_name = "Segoe UI"
            max_rows = 10
        "#;
        let config: Config = toml::from_str(minimal_toml).unwrap();
        assert!(config.exclude_apps.contains(&"1Password.exe".to_string()));
        assert!(config.exclude_apps.contains(&"Bitwarden.exe".to_string()));

        let custom_toml = r#"
            font_name = "Segoe UI"
            max_rows = 10
            exclude_apps = ["custom.exe"]
        "#;
        let config_custom: Config = toml::from_str(custom_toml).unwrap();
        assert_eq!(config_custom.exclude_apps, vec!["custom.exe".to_string()]);
    }
}
