//! [Control API] settings.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [Control API] settings.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApi {
    /// Path to directory with static [Сontrol API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[default = "specs/"]
    pub static_specs_dir: String,
}

#[cfg(test)]
mod spec {
    use serial_test::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_CONTROL__STATIC_SPECS_DIR" => "test/",
        );

        assert_ne!(
            default_conf.control.static_specs_dir,
            env_conf.control.static_specs_dir
        );

        assert_eq!(env_conf.control.static_specs_dir, "test/");
    }
}
