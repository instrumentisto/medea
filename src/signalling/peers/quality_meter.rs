//! [`ConnectionQualityScore`] score calculator implementation.

use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use medea_client_api_proto::{stats::StatId, ConnectionQualityScore};
