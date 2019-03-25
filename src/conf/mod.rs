/// Provides application configuration options.
///
/// Configuration options can be parsed from config files in TOML format.
use config::{
    Config, ConfigError, Environment, File, FileFormat, Source, Value,
};
use failure::Error;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

mod duration;
pub mod server;

static APP_CONF_PATH_CMD_ARG_NAME: &str = "--conf";
static APP_CONF_PATH_ENV_VAR_NAME: &str = "MEDEA_CONF";

/// Settings represents all configuration setting of application.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Conf {
    /// Represents [`Server`] configuration section.
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
    pub fn new() -> Result<Self, Error> {
        use std::env;

        let mut cfg = Config::new();

        cfg.merge(Self::default())?;

        if let Some(path) = get_conf_file_name(
            env::var(APP_CONF_PATH_ENV_VAR_NAME),
            env::args(),
        ) {
            cfg.merge(File::with_name(&path))?;
        }

        cfg.merge(Environment::with_prefix("MEDEA").separator("__"))?;

        let s: Self = cfg.try_into()?;
        Ok(s)
    }
}

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
    use super::get_conf_file_name;
    use crate::conf::{
        Conf, APP_CONF_PATH_CMD_ARG_NAME, APP_CONF_PATH_ENV_VAR_NAME,
    };
    use std::time::Duration;

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
    fn ensure_file_overrides_defaults() {
        let defaults = Conf::new().unwrap();
        let test_config_file_path =
            "ensure_file_overrides_defaults_test_config.toml";

        let data = format!("[server]\nclient_idle_timeout = \"55s\"");
        std::fs::write(test_config_file_path, data).unwrap();
        std::env::set_var(APP_CONF_PATH_ENV_VAR_NAME, test_config_file_path);

        let new_config = Conf::new().unwrap();

        std::env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        std::fs::remove_file(test_config_file_path).unwrap();

        assert_eq!(
            new_config.server.client_idle_timeout,
            Duration::from_secs(55)
        );
        assert_ne!(
            new_config.server.client_idle_timeout,
            defaults.server.client_idle_timeout
        );
    }
}
