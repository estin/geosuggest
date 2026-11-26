const CONFIG_PREFIX: &str = "GEOSUGGEST";
const CONFIG_FILE_PATH: &str = "./defaults.toml";

#[bonfig::settings]
#[derive(Debug, Clone)]
pub struct Settings {
    #[builder(default = "localhost")]
    pub host: String,
    #[builder(default = 8080)]
    pub port: usize,
    #[builder(default)]
    pub index_file: String,
    pub static_dir: Option<String>,
    #[builder(default = "/")]
    pub url_path_prefix: String,
    /// Proxy networks allowed to report the client IP, all are trusted by default
    #[builder(default = vec!["0.0.0.0/0".into(), "::/0".into()])]
    pub trusted_proxies: Vec<String>,
    /// Public prefix for the API docs when a proxy strips url_path_prefix, defaults to url_path_prefix
    pub public_url_prefix: Option<String>,
    #[cfg(feature = "geoip2")]
    pub geoip2_file: Option<String>,
}

impl Settings {
    pub fn new() -> bonfig::Result<Self> {
        bonfig::Loader::builder()
            .env_prefix(CONFIG_PREFIX)
            .file(CONFIG_FILE_PATH)
            .build()
            .load()
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use bonfig::FromSource as _;

    #[test]
    fn toml_overrides_layer_over_defaults() {
        let settings = Settings::from_toml("port = 9090\nhost = \"example.com\"").unwrap();
        assert_eq!(settings.port, 9090);
        assert_eq!(settings.host, "example.com");
        assert_eq!(settings.url_path_prefix, "/");
        assert_eq!(settings.index_file, "");
        assert_eq!(settings.static_dir, None);
        assert_eq!(settings.trusted_proxies, ["0.0.0.0/0", "::/0"]);
        assert_eq!(settings.public_url_prefix, None);

        let settings = Settings::from_toml("trusted_proxies = [\"10.42.0.0/16\"]").unwrap();
        assert_eq!(settings.trusted_proxies, ["10.42.0.0/16"]);
    }
}
