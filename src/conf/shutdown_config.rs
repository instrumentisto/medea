//! More system settings

use serde::{Deserialize, Serialize};
use smart_default::*;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ShutdownConfiguration {
    #[default(5000)]
    pub timeout: u64,
}
