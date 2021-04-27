//! Collection of [`RtcIceServer`][1]s.
//!
//! [1]: https://w3.org/TR/webrtc/#rtciceserver-dictionary

use medea_client_api_proto::IceServer;

/// Collection of [`RtcIceServer`]s (see [RTCIceServer][1]).
///
/// [1]: https://w3.org/TR/webrtc/#rtciceserver-dictionary
#[derive(Debug)]
pub struct RtcIceServers;

impl<I> From<I> for RtcIceServers
where
    I: IntoIterator<Item = IceServer>,
{
    fn from(servers: I) -> Self {
        unimplemented!()
    }
}
