/// Provides application configuration options.
///
/// Configuration options can be parsed from config files in TOML format.
use config::{Config, Environment, File, FileFormat};
use failure::Error;
use serde_derive::{Deserialize, Serialize};
use toml::to_string;

mod duration;
pub mod server;

/// Settings represents all configuration setting of application.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    /// Represents [`Server`] configuration section.
    pub server: server::Server,
}

impl Settings {
    pub fn new() -> Result<Self, Error> {
        let mut cfg = Config::new();
        let defaults = to_string(&Settings::default())?;
        cfg.merge(File::from_str(defaults.as_str(), FileFormat::Toml))?;
        cfg.merge(File::with_name("config"))?;
        cfg.merge(Environment::with_prefix("conf").separator("__"))?;

        let s: Settings = cfg.try_into()?;
        Ok(s)
    }
}
