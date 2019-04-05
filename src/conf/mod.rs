//! Provides application configuration options.

pub mod rpc;
pub mod server;

use std::env;

use config::{
    Config, ConfigError, Environment, File, FileFormat, Source, Value,
};
use failure::Error;
use serde::{Deserialize, Serialize};

pub use self::rpc::Rpc;
pub use self::server::Server;

use std::collections::HashMap;

/// CLI argument that is responsible for holding application configuration
/// file path.
static APP_CONF_PATH_CMD_ARG_NAME: &str = "--conf";
/// Environment variable that is responsible for holding application
/// configuration file path.
static APP_CONF_PATH_ENV_VAR_NAME: &str = "MEDEA_CONF";

/// Holds application config.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Conf {
    /// HTTP server settings.
    pub rpc: rpc::Rpc,
    /// RPC connection settings.
    pub server: server::Server,
}

/// Allows merging [`Conf`] into [`config::Config`].
// TODO: Remove after the following issue is resolved:
//       https://github.com/mehcode/config-rs/issues/60#issuecomment-443241600
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
    /// Creates new [`Conf`] and applies values from the following sources
    /// (in the following order):
    /// - default values;
    /// - configuration file, the name of which is given as a command line
    ///   parameter or environment variable;
    /// - environment variables.
    pub fn parse() -> Result<Self, Error> {
        // TODO: use Config::try_from(&Self::default()) when the issue is fixed:
        //       https://github.com/mehcode/config-rs/issues/60
        let mut cfg = Config::new();
        cfg.merge(Self::default())?;

        if let Some(path) = get_conf_file_name(env::args()) {
            cfg.merge(File::with_name(&path))?;
        }

        cfg.merge(Environment::with_prefix("MEDEA").separator("."))?;

        let s: Self = cfg.try_into()?;
        Ok(s)
    }
}

/// Returns the path to the configuration file, if it's set via CLI `args`
/// or environment variables.
fn get_conf_file_name<T>(args: T) -> Option<String>
where
    T: IntoIterator<Item = String>,
{
    // First, check CLI arguments as they have the highest priority.
    let mut args = args
        .into_iter()
        .skip_while(|x| x != APP_CONF_PATH_CMD_ARG_NAME);
    if args.next().is_some() {
        return args.next().filter(|v| !v.is_empty());
    }

    // Then check env var.
    env::var(APP_CONF_PATH_ENV_VAR_NAME)
        .ok()
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod get_conf_file_name_spec {
    use super::*;

    #[test]
    fn none_if_nothing_is_set() {
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        assert_eq!(get_conf_file_name(vec![]), None);
    }

    #[test]
    fn none_if_empty() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(
            get_conf_file_name(vec![
                APP_CONF_PATH_CMD_ARG_NAME.to_owned(),
                "".to_owned(),
            ]),
            None,
        );
    }

    #[test]
    fn env_if_set() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(get_conf_file_name(vec![]), Some("env_path".to_owned()));
    }

    #[test]
    fn arg_if_set() {
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        assert_eq!(
            get_conf_file_name(vec![
                APP_CONF_PATH_CMD_ARG_NAME.to_owned(),
                "arg_path".to_owned(),
            ]),
            Some("arg_path".to_owned()),
        );
    }

    #[test]
    fn arg_is_prioritized() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(
            get_conf_file_name(vec![
                APP_CONF_PATH_CMD_ARG_NAME.to_owned(),
                "arg_path".to_owned(),
            ]),
            Some("arg_path".to_owned()),
        );
    }
}

#[cfg(test)]
mod conf_parse_spec {
    use std::{fs, time::Duration};

    use serial_test_derive::serial;

    use super::*;

    #[test]
    #[serial]
    fn file_overrides_defaults() {
        let defaults = Conf::default();
        let test_config_file_path = "test_config.toml";

        let data = format!("[rpc]\nidle_timeout = \"45s\"");
        fs::write(test_config_file_path, data).unwrap();
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, test_config_file_path);

        let new_config = Conf::parse().unwrap();

        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        fs::remove_file(test_config_file_path).unwrap();

        assert_eq!(new_config.rpc.idle_timeout, Duration::from_secs(45));
        assert_ne!(new_config.rpc.idle_timeout, defaults.rpc.idle_timeout);
    }

    #[test]
    #[serial]
    fn env_overrides_defaults() {
        let defaults = Conf::default();

        env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "46s");
        let new_config = Conf::parse().unwrap();
        env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");

        assert_eq!(new_config.rpc.idle_timeout, Duration::from_secs(46));
        assert_ne!(new_config.rpc.idle_timeout, defaults.rpc.idle_timeout);
    }

    #[test]
    #[serial]
    fn env_overrides_file() {
        let test_config_file_path = "test_config.toml";

        let data = format!("[rpc]\nidle_timeout = \"47s\"");
        fs::write(test_config_file_path, data).unwrap();
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, test_config_file_path);

        let file_config = Conf::parse().unwrap();

        env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "48s");
        let file_env_config = Conf::parse().unwrap();

        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        fs::remove_file(test_config_file_path).unwrap();
        env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");

        assert_eq!(file_config.rpc.idle_timeout, Duration::from_secs(47));

        assert_eq!(file_env_config.rpc.idle_timeout, Duration::from_secs(48));
    }
}
