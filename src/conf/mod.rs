//! Provides application configuration options.

pub mod control;
pub mod log;
pub mod media;
pub mod rpc;
pub mod server;
pub mod shutdown;
pub mod turn;

use std::env;

use config::{Config, Environment, File};
use failure::Error;
use serde::{Deserialize, Serialize};

#[doc(inline)]
pub use self::{
    control::ControlApi, log::Log, media::Media, rpc::Rpc, server::Server,
    shutdown::Shutdown, turn::Turn,
};

/// CLI argument that is responsible for holding application configuration
/// file path.
static APP_CONF_PATH_CMD_ARG_NAME: &str = "--conf";
/// Environment variable that is responsible for holding application
/// configuration file path.
static APP_CONF_PATH_ENV_VAR_NAME: &str = "MEDEA_CONF";

/// Holds application config.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Conf {
    /// RPC connection settings.
    pub rpc: Rpc,

    /// Servers settings.
    pub server: Server,

    /// TURN server settings.
    pub turn: Turn,

    /// Logging settings.
    pub log: Log,

    /// Application shutdown settings.
    pub shutdown: Shutdown,

    /// [Control API] settings.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub control: ControlApi,

    /// [`Peer`] media traffic watcher configuration.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub media: Media,
}

impl Conf {
    /// Creates new [`Conf`] and applies values from the following sources
    /// (in the following order):
    /// - default values;
    /// - configuration file, the name of which is given as a command line
    ///   parameter or environment variable;
    /// - environment variables.
    ///
    /// # Errors
    ///
    /// Errors if parsing fails.
    pub fn parse() -> Result<Self, Error> {
        let mut cfg = Config::new();

        if let Some(path) = get_conf_file_name(env::args()) {
            cfg.merge(File::with_name(&path))?;
        }

        cfg.merge(Environment::with_prefix("MEDEA").separator("__"))?;

        Ok(cfg.try_into()?)
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
pub(crate) mod spec {
    use serial_test::serial;

    use super::*;

    /// Macro which overrides environment variables with provided values,
    /// parses [`Conf`] and finally removes all the overrided variables.
    ///
    /// # Usage
    ///
    /// ```rust
    /// # use crate::conf::Conf;
    /// #
    /// let default_conf = Conf::default();
    /// let env_conf = overrided_by_env_conf!(
    ///        "MEDEA_TURN__HOST" => "example.com",
    ///        "MEDEA_TURN__PORT" => "1234",
    ///        "MEDEA_TURN__USER" => "ferris",
    ///        "MEDEA_TURN__PASS" => "qwerty"
    /// );
    ///
    /// assert_ne!(default_conf.turn.host, env_conf.turn.host);
    /// assert_ne!(default_conf.turn.port, env_conf.turn.port);
    /// // ...
    /// ```
    #[macro_export]
    macro_rules! overrided_by_env_conf {
        ($($env:expr => $value:expr),+ $(,)?) => {{
            $(::std::env::set_var($env, $value);)+
            let conf = crate::conf::Conf::parse().unwrap();
            $(::std::env::remove_var($env);)+
            conf
        }};
    }

    #[test]
    #[serial]
    fn get_conf_file_name_spec_none_if_nothing_is_set() {
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        assert_eq!(get_conf_file_name(Vec::new()), None);
    }

    #[test]
    #[serial]
    fn get_conf_file_name_spec_none_if_empty() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(
            get_conf_file_name(vec![
                APP_CONF_PATH_CMD_ARG_NAME.to_owned(),
                "".to_owned(),
            ]),
            None,
        );
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
    }

    #[test]
    #[serial]
    fn get_conf_file_name_spec_env_if_set() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(get_conf_file_name(Vec::new()), Some("env_path".to_owned()));
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
    }

    #[test]
    #[serial]
    fn get_conf_file_name_spec_arg_if_set() {
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
    #[serial]
    fn get_conf_file_name_spec_arg_is_prioritized() {
        env::set_var(APP_CONF_PATH_ENV_VAR_NAME, "env_path");
        assert_eq!(
            get_conf_file_name(vec![
                APP_CONF_PATH_CMD_ARG_NAME.to_owned(),
                "arg_path".to_owned(),
            ]),
            Some("arg_path".to_owned()),
        );
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
    }
}
