//! Wrapper around [`IPTables`] for opening/closing ports.

use std::sync::Arc;

use iptables::{error::IPTResult, IPTables};

/// Actually newtype for [`IPTables`].
#[derive(Clone)]
pub struct Firewall(Arc<IPTables>);

impl Firewall {
    /// Create new instance of [`Firewall`].
    pub fn new() -> IPTResult<Self> {
        iptables::new(false).map(|ipt| Self(Arc::new(ipt)))
    }

    /// `DROP` all connections to the provided port.
    pub fn close_port(&self, port: u16) -> IPTResult<bool> {
        self.0.append_unique(
            "filter",
            "INPUT",
            &format!("-p tcp --dport {} -j DROP", port),
        )
    }

    /// Remove all rules which `DROP`s provided port.
    pub fn open_port(&self, port: u16) -> IPTResult<bool> {
        self.0.delete_all(
            "filter",
            "INPUT",
            &format!("-p tcp --dport {} -j DROP", port),
        )
    }
}
