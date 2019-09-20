use iptables::{error::IPTResult, IPTables};
use std::sync::Arc;

#[derive(Clone)]
pub struct Firewall(Arc<IPTables>);

impl Firewall {
    pub fn new() -> IPTResult<Self> {
        iptables::new(false).map(|ipt| Self(Arc::new(ipt)))
    }

    pub fn close_port(&self, port: u16) -> IPTResult<bool> {
        self.0.append_unique(
            "filter",
            "INPUT",
            &format!("-p tcp --dport {} -j DROP", port),
        )
    }

    pub fn open_port(&self, port: u16) -> IPTResult<bool> {
        self.0.delete_all(
            "filter",
            "INPUT",
            &format!("-p tcp --dport {} -j DROP", port),
        )
    }
}
