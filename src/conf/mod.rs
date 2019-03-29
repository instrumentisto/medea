/// Provides application configuration options.
///
/// Configuration options can be parsed from config files in TOML format.
pub mod rpc;
pub mod server;

use config::{
    Config, ConfigError, Environment, File, FileFormat, Source, Value,
};
use failure::Error;
use serde::{Deserialize, Serialize};
use smart_default::*;

pub use self::server::Server;
pub use self::rpc::Rpc;

use std::collections::HashMap;

static APP_CONF_PATH_CMD_ARG_NAME: &str = "--conf";
static APP_CONF_PATH_ENV_VAR_NAME: &str = "MEDEA_CONF";

/// Settings represents all configuration setting of application.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Conf {
    /// Represents [`Server`] configuration section.
    pub rpc: rpc::Rpc,
    pub server: server::Server,
}

impl Source for Conf {
    fn clone_into_box(&self) -> Box<Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        let serialized = toml::to_string(self).unwrap();
        File::from_str(serialized.as_str(), FileFormat::Toml).collect()
    }
}

impl Conf {
    /// Creates new [`Conf`] and applies values from such sources
    /// and in that order:
    /// - default values;
    /// - configuration file, the name of which is given as a command line
    /// parameter or environment variable;
    /// - environment variables;
    pub fn parse() -> Result<Self, Error> {
        use std::env;

        let mut cfg = Config::new();

        cfg.merge(Self::default())?;

        if let Some(path) = get_conf_file_name(
            env::var(APP_CONF_PATH_ENV_VAR_NAME),
            env::args(),
        ) {
            cfg.merge(File::with_name(&path))?;
        }

        cfg.merge(Environment::with_prefix("MEDEA").separator("."))?;

        let s: Self = cfg.try_into()?;
        Ok(s)
    }
}

/// Returns the name of the configuration file, if defined.
fn get_conf_file_name<T>(
    env_var: Result<String, std::env::VarError>,
    cmd_args: T,
) -> Option<String>
where
    T: Iterator<Item = String> + std::fmt::Debug,
{
    if let Ok(path) = env_var {
        Some(path)
    } else {
        let mut args = cmd_args.skip_while(|x| x != APP_CONF_PATH_CMD_ARG_NAME);
        if args.next().is_some() {
            args.next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use serial_test_derive::serial;

    use std::time::Duration;

    use crate::conf::{
        Conf, APP_CONF_PATH_CMD_ARG_NAME, APP_CONF_PATH_ENV_VAR_NAME,
    };

    use super::get_conf_file_name;
    #[test]
    fn get_conf_file_name_none() {
        let file = get_conf_file_name(
            Err(std::env::VarError::NotPresent),
            Vec::new().into_iter(),
        );
        assert_eq!(file, None);
    }

    #[test]
    fn get_conf_file_name_env() {
        let file = get_conf_file_name(
            Ok("env_path".to_owned()),
            Vec::new().into_iter(),
        );
        assert_eq!(file, Some("env_path".to_owned()));
    }

    #[test]
    fn get_conf_file_name_arg() {
        let file = get_conf_file_name(
            Err(std::env::VarError::NotPresent),
            vec![APP_CONF_PATH_CMD_ARG_NAME.to_owned(), "arg_path".to_owned()]
                .into_iter(),
        );
        assert_eq!(file, Some("arg_path".to_owned()));
    }

    #[test]
    fn get_conf_file_name_both_env_overrides() {
        let file = get_conf_file_name(
            Ok("env_path".to_owned()),
            vec![APP_CONF_PATH_CMD_ARG_NAME.to_owned(), "arg_path".to_owned()]
                .into_iter(),
        );
        assert_eq!(file, Some("env_path".to_owned()));
    }

    #[test]
    #[serial]
    fn file_overrides_defaults() {
        let defaults = Conf::default();
        let test_config_file_path = "test_config.toml";

        let data = format!("[rpc]\nidle_timeout = \"45s\"");
        std::fs::write(test_config_file_path, data).unwrap();
        std::env::set_var(APP_CONF_PATH_ENV_VAR_NAME, test_config_file_path);

        let new_config = Conf::parse().unwrap();

        std::env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        std::fs::remove_file(test_config_file_path).unwrap();

        assert_eq!(new_config.rpc.idle_timeout, Duration::from_secs(45));
        assert_ne!(new_config.rpc.idle_timeout, defaults.rpc.idle_timeout);
    }

    #[test]
    #[serial]
    fn env_overrides_defaults() {
        let defaults = Conf::default();

        std::env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "46s");
        let new_config = Conf::parse().unwrap();
        std::env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");

        assert_eq!(new_config.rpc.idle_timeout, Duration::from_secs(46));
        assert_ne!(new_config.rpc.idle_timeout, defaults.rpc.idle_timeout);
    }

    #[test]
    #[serial]
    fn env_overrides_file() {
        let test_config_file_path = "test_config.toml";

        let data = format!("[rpc]\nidle_timeout = \"47s\"");
        std::fs::write(test_config_file_path, data).unwrap();
        std::env::set_var(APP_CONF_PATH_ENV_VAR_NAME, test_config_file_path);

        let file_config = Conf::parse().unwrap();

        std::env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "48s");
        let file_env_config = Conf::parse().unwrap();

        std::env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        std::fs::remove_file(test_config_file_path).unwrap();
        std::env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");

        assert_eq!(file_config.rpc.idle_timeout, Duration::from_secs(47));

        assert_eq!(file_env_config.rpc.idle_timeout, Duration::from_secs(48));
    }
}
