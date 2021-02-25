use std::rc::Rc;

use derive_more::{Display, From};

use crate::{platform, utils::JsCaused};

/// Errors which can occur during deserialization of the [`RtcStatsType`].
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

    /// Some JS error occurred.
    #[display(fmt = "Unexpected JS side error: {}", _0)]
    Js(platform::Error),

    /// `RTCStats.entries` is undefined.
    #[display(fmt = "RTCStats.entries is undefined")]
    UndefinedEntries,

    /// Error of [`RtcStats`] deserialization.
    #[display(fmt = "Failed to deserialize into RtcStats: {}", _0)]
    ParseError(Rc<serde_json::Error>),
}
