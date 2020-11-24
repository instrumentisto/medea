//! Implementation of [Client API].
//!
//! [Client API]: https://tinyurl.com/yx9thsnr

mod session;

pub mod rpc_connection;
pub mod server;

pub use self::session::RpcServerRepository;
