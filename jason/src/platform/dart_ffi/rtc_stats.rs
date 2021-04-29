//! Deserialization of [`RtcStats`].

use medea_client_api_proto::stats::RtcStat;

/// All available [`RtcStatsType`]s of a [`RtcPeerConnection`].
///
/// [`RtcStatsType`]: medea_client_api_proto::stats::RtcStatsType
/// [`RtcPeerConnection`]: crate::platform::RtcPeerConnection
#[derive(Clone, Debug)]
pub struct RtcStats(pub Vec<RtcStat>);
