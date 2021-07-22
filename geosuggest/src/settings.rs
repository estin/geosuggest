use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::Path;

const CONFIG_PREFIX: &str = "GEOSUGGEST";
const CONFIG_FILE_PATH: &str = "./defaults.toml";
const CONFIG_FILE_ENV_PATH_KEY: &str = "GEOSUGGEST_CONFIG_FILE";

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub host: String,
    pub port: usize,
    pub index_file: String,
    pub static_dir: Option<String>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        log::info!("Try read config from: {}", CONFIG_FILE_PATH);
        if Path::new(CONFIG_FILE_PATH).exists() {
            if let Err(e) = s.merge(File::with_name(CONFIG_FILE_PATH).required(false)) {
                log::info!("{}", e);
            };
        }

        log::info!(
            "Try read and merge in config from file by environment variable: {}",
            CONFIG_FILE_ENV_PATH_KEY
        );
        if let Ok(config_path) = std::env::var(CONFIG_FILE_ENV_PATH_KEY) {
            s.merge(File::with_name(&config_path))?;
        };

        log::info!(
            "Try read and merge in config from environment variables with prefix {}",
            CONFIG_PREFIX
        );
        s.merge(Environment::with_prefix(CONFIG_PREFIX).separator("__"))?;

        s.try_into()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            host: "localhost".to_owned(),
            port: 8080,
            index_file: "".to_string(),
            static_dir: None,
        }
    }
}
