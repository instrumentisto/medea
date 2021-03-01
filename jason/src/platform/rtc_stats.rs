//! Platform-agnostic functionality of [`platform::RtcStats`].

use std::rc::Rc;

use derive_more::{Display, From};

use crate::{platform, utils::JsCaused};

/// Errors which can occur during deserialization of the [`RtcStatsType`].
///
/// [`RtcStatsType`]: medea_client_api_proto::stats::RtcStatsType
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum RtcStatsError {
    /// [RTCStats.id][1] is undefined.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcstats-id
    #[display(fmt = "RTCStats.id is undefined")]
    UndefinedId,

    /// [RTCStats.stats] is undefined.
    ///
    /// [1]: https://w3.org/TR/webrtc-stats/#dfn-stats-object
    #[display(fmt = "RTCStats.stats is undefined")]
    UndefinedStats,

    /// Some platform error occurred.
    #[display(fmt = "Unexpected platform error: {}", _0)]
    Platform(platform::Error),

    /// `RTCStats.entries` is undefined.
    #[display(fmt = "RTCStats.entries is undefined")]
    UndefinedEntries,

    /// [`platform::RtcStats`] deserialization error.
    #[display(fmt = "Failed to deserialize into RtcStats: {}", _0)]
    ParseError(Rc<serde_json::Error>),
}
