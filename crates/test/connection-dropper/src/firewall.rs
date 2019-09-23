//! Wrapper around [`IPTables`] for opening/closing ports.

use std::sync::Arc;

use iptables::{
    error::{IPTError, IPTResult},
    IPTables,
};

/// Actually newtype for [`IPTables`].
#[derive(Clone)]
pub struct Firewall(Arc<IPTables>);

impl Firewall {
    /// Create new instance of [`Firewall`].
    pub fn new() -> IPTResult<Self> {
        iptables::new(false).map(|ipt| Self(Arc::new(ipt)))
    }

    /// `DROP` all connections to the provided port.
    ///
    /// This function ignores "the rule exists in the table/chain" error.
    pub fn close_port(&self, port: u16) -> IPTResult<bool> {
        match self.0.append_unique(
            "filter",
            "INPUT",
            &format!("-p tcp --dport {} -j DROP", port),
        ) {
            Ok(b) => Ok(b),
            Err(e) => match e {
                IPTError::Other(error_text) => {
                    if error_text == "the rule exists in the table/chain" {
                        Ok(false)
                    } else {
                        Err(e)
                    }
                }
                _ => Err(e),
            },
        }
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
