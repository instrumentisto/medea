//! Implementation of [Client API].
//!
//! [Client API]: https://tinyurl.com/yx9thsnr

mod session;

pub mod rpc_connection;
pub mod server;

pub use self::session::RpcServerRepository;

/// Maximum size of a WebSocket message in bytes.
///
/// This limit also is used for a fragmented message.
///
/// `Room` state of 5 `Member`s with a screen sharing, camera and audio will be
/// around 100 Kb, so this value is multiplied by 5 and rounded just in case.
const MAX_WS_MSG_SIZE: usize = 512 * 1024; // 512 Kb
