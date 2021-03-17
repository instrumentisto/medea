//! Deserialization of the [`RtcStats`] from the [`SysRtcStats`].
//!
//! [`SysRtcStats`]: web_sys::RtcStats

use std::{convert::TryFrom, rc::Rc};

use derive_more::{Display, From};
use js_sys::{
    Array as JsArray, Function as JsFunction, Iterator as JsIterator, JsString,
};
use medea_client_api_proto::stats::{RtcStat, RtcStatsType};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};

use crate::utils::{get_property_by_name, JsCaused, JsError};

/// Entry of the JS RTC stats dictionary.
struct RtcStatsReportEntry(JsString, JsValue);

impl TryFrom<JsArray> for RtcStatsReportEntry {
    type Error = Traced<RtcStatsError>;

    fn try_from(value: JsArray) -> Result<Self, Self::Error> {
        use RtcStatsError::{Js, UndefinedId, UndefinedStats};

        let id = value.get(0);
        let stats = value.get(1);

        if id.is_undefined() {
            return Err(tracerr::new!(UndefinedId));
        }

        if stats.is_undefined() {
            return Err(tracerr::new!(UndefinedStats));
        }

        let id = id
            .dyn_into::<JsString>()
            .map_err(|e| tracerr::new!(Js(JsError::from(e))))?;
        let stats = stats
            .dyn_into::<JsValue>()
            .map_err(|e| tracerr::new!(Js(JsError::from(e))))?;

        Ok(RtcStatsReportEntry(id, stats))
    }
}

/// All available [`RtcStatsType`] of [`PeerConnection`].
///
/// [`PeerConnection`]: crate::peer::PeerConnection
#[derive(Clone, Debug)]
pub struct RtcStats(pub Vec<RtcStat>);

impl TryFrom<&JsValue> for RtcStats {
    type Error = Traced<RtcStatsError>;

    fn try_from(stats: &JsValue) -> Result<Self, Self::Error> {
        use RtcStatsError::{Js, UndefinedEntries};

        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<JsFunction>())
            })
            .ok_or_else(|| tracerr::new!(UndefinedEntries))?;

        let iterator = entries_fn
            .call0(stats.as_ref())
            .map_err(|e| tracerr::new!(Js(JsError::from(e))))?
            .unchecked_into::<JsIterator>();

        let mut stats = Vec::new();

        for stat in iterator {
            let stat = stat.map_err(|e| tracerr::new!(Js(JsError::from(e))))?;
            let stat = stat.unchecked_into::<JsArray>();
            let stat = RtcStatsReportEntry::try_from(stat)
                .map_err(tracerr::map_from_and_wrap!())?;
            let stat: RtcStat = JsValue::from(&stat.1)
                .into_serde()
                .map_err(Rc::new)
                .map_err(tracerr::from_and_wrap!())?;

            if let RtcStatsType::Other = &stat.stats {
                continue;
            }

            stats.push(stat);
        }

        Ok(RtcStats(stats))
    }
}

/// Errors which can occur during deserialization of the [`RtcStatsType`].
#[derive(Clone, Debug, Display, From, JsCaused)]
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
    Js(JsError),

    /// `RTCStats.entries` is undefined.
    #[display(fmt = "RTCStats.entries is undefined")]
    UndefinedEntries,

    /// Error of [`RtcStats`] deserialization.
    #[display(fmt = "Failed to deserialize into RtcStats: {}", _0)]
    ParseError(Rc<serde_json::Error>),
}
