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
    pub url_path_prefix: String,
    #[cfg(feature = "geoip2")]
    pub geoip2_file: Option<String>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::builder();

        #[cfg(feature = "tracing")]
        tracing::info!("Try read config from: {}", CONFIG_FILE_PATH);
        if Path::new(CONFIG_FILE_PATH).exists() {
            s = s.add_source(File::with_name(CONFIG_FILE_PATH).required(false))
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Try read and merge in config from file by environment variable: {}",
            CONFIG_FILE_ENV_PATH_KEY
        );
        if let Ok(config_path) = std::env::var(CONFIG_FILE_ENV_PATH_KEY) {
            s = s.add_source(File::with_name(&config_path));
        };

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Try read and merge in config from environment variables with prefix {}",
            CONFIG_PREFIX
        );
        s = s.add_source(Environment::with_prefix(CONFIG_PREFIX).separator("__"));

        s.build()?.try_deserialize()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            host: "localhost".to_owned(),
            port: 8080,
            index_file: "".to_string(),
            static_dir: None,
            url_path_prefix: "/".to_string(),
            #[cfg(feature = "geoip2")]
            geoip2_file: None,
        }
    }
}
