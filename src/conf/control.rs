use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [Control API] settings.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct Control {
    /// Path to directory with static [Сontrol API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[default(String::from("specs/"))]
    pub static_specs_dir: String,
}
