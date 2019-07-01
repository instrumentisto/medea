//! More system settings

use serde::{Deserialize, Serialize};
use smart_default::*;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct SystemConfiguration {
    #[default(5000)]
    pub shutdown_timeout: u64,
}