//! Implementation of [Client API].
//!
//! [Client API]: https://tinyurl.com/yx9thsnr

mod session;

pub mod rpc_connection;
pub mod server;

pub use self::session::RpcServerRepository;

/// Max size of WebSocket message in bytes.
///
/// This limit also will be used for the fragmented message.
///
/// `Room` state of 5 `Member`s with screen sharing, camera and audio will be
/// ~300Kb, this value is multiplied by 3 just in case.
const MAX_WS_MSG_SIZE: usize = 1000 * 1024;
