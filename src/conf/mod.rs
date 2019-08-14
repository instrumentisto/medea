//! Provides application configuration options.

pub mod grpc;
pub mod http_server;
pub mod log;
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
    grpc::Grpc,
    http_server::HttpServer,
    log::Log,
    rpc::Rpc,
    server::Server,
    shutdown::Shutdown,
    turn::{Redis, Turn},
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
    /// HTTP server settings.
    pub rpc: Rpc,

    /// Servers related settings.
    pub server: Server,

    /// TURN server settings.
    pub turn: Turn,

    /// Logging settings.
    pub log: Log,

    /// Application shutdown settings.
    pub shutdown: Shutdown,
}

impl Conf {
    /// Creates new [`Conf`] and applies values from the following sources
    /// (in the following order):
    /// - default values;
    /// - configuration file, the name of which is given as a command line
    ///   parameter or environment variable;
    /// - environment variables.
    pub fn parse() -> Result<Self, Error> {
        let mut cfg = Config::new();

        if let Some(path) = get_conf_file_name(env::args()) {
            cfg.merge(File::with_name(&path))?;
        }

        cfg.merge(Environment::with_prefix("MEDEA").separator("."))?;

        Ok(cfg.try_into()?)
    }

    // TODO: any reason why this func is here and not in impl Server?
    //       dont hardcode scheme, just store it in 'host' field
    //       and dont forget to update helm configs
    pub fn get_base_rpc_url(&self) -> String {
        format!("wss://{}", self.server.http.host)
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
mod tests {
    use std::{fs, net::Ipv4Addr, time::Duration};

    use serial_test_derive::serial;

    use super::*;

    #[test]
    #[serial]
    fn get_conf_file_name_spec_none_if_nothing_is_set() {
        env::remove_var(APP_CONF_PATH_ENV_VAR_NAME);
        assert_eq!(get_conf_file_name(vec![]), None);
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
        assert_eq!(get_conf_file_name(vec![]), Some("env_path".to_owned()));
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

    #[test]
    #[serial]
    fn conf_parse_spec_file_overrides_defaults() {
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
    fn conf_parse_spec_env_overrides_defaults() {
        let defaults = Conf::default();

        env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "46s");
        let new_config = Conf::parse().unwrap();
        env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");

        assert_eq!(new_config.rpc.idle_timeout, Duration::from_secs(46));
        assert_ne!(new_config.rpc.idle_timeout, defaults.rpc.idle_timeout);
    }

    #[test]
    #[serial]
    fn conf_parse_spec_env_overrides_file() {
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

    #[test]
    #[serial]
    fn redis_conf() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_TURN.DB.REDIS.IP", "5.5.5.5");
        env::set_var("MEDEA_TURN.DB.REDIS.PORT", "1234");
        env::set_var("MEDEA_TURN.DB.REDIS.CONNECTION_TIMEOUT", "10s");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.turn.db.redis.ip, env_conf.turn.db.redis.ip);
        assert_ne!(
            default_conf.turn.db.redis.connection_timeout,
            env_conf.turn.db.redis.connection_timeout
        );
        assert_ne!(
            default_conf.turn.db.redis.connection_timeout,
            env_conf.turn.db.redis.connection_timeout
        );

        assert_eq!(env_conf.turn.db.redis.ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.turn.db.redis.port, 1234);
        assert_eq!(
            env_conf.turn.db.redis.connection_timeout,
            Duration::from_secs(10)
        )
    }

    #[test]
    #[serial]
    fn turn_conf() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_TURN.HOST", "example.com");
        env::set_var("MEDEA_TURN.PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.turn.host, env_conf.turn.host);
        assert_ne!(default_conf.turn.port, env_conf.turn.port);

        assert_eq!(env_conf.turn.host, "example.com");
        assert_eq!(env_conf.turn.port, 1234);
        assert_eq!(env_conf.turn.addr(), "example.com:1234");
    }

    #[test]
    #[serial]
    fn log_conf() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_LOG.LEVEL", "WARN");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.log.level(), env_conf.log.level());

        assert_eq!(env_conf.log.level(), Some(slog::Level::Warning));

        env::set_var("MEDEA_LOG.LEVEL", "OFF");

        assert_eq!(Conf::parse().unwrap().log.level(), None);
    }

    #[test]
    #[serial]
    fn shutdown_conf_test() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_SHUTDOWN.TIMEOUT", "700ms");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.shutdown.timeout, env_conf.shutdown.timeout);
        assert_eq!(env_conf.shutdown.timeout, Duration::from_millis(700));
    }
}
