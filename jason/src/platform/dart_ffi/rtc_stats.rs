//! Deserialization of [`RtcStats`] from [`SysRtcStats`].
//!
//! [`SysRtcStats`]: web_sys::RtcStats

use medea_client_api_proto::stats::RtcStat;

/// All available [`RtcStatsType`]s of a [`platform::RtcPeerConnection`].
#[derive(Clone, Debug)]
pub struct RtcStats(pub Vec<RtcStat>);
