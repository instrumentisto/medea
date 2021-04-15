use std::{future::Future, time::Duration};

pub mod constraints;
pub mod error;
pub mod ice_server;
pub mod input_device_info;
pub mod media_devices;
pub mod media_track;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver;
pub mod transport;
pub mod utils;

pub use extern_executor::spawn;

pub async fn delay_for(delay: Duration) {
    todo!()
}
