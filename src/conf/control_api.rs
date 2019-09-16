//! [Control API] settings.
//!
//! [Control API]: http://tiny.cc/380uaz

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [Control API] settings.
///
/// [Control API]: http://tiny.cc/380uaz
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApi {
    /// Path to directory with static [Сontrol API] specs.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[default(String::from("specs/"))]
    pub static_specs_dir: String,
}

#[cfg(test)]
mod control_conf_specs {
    use serial_test_derive::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_CONTROL_API__STATIC_SPECS_DIR" => "test/"
        );

        assert_ne!(
            default_conf.control_api.static_specs_dir,
            env_conf.control_api.static_specs_dir
        );

        assert_eq!(env_conf.control_api.static_specs_dir, "test/");
    }
}
