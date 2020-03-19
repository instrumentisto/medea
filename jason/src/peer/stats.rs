//! Deserialization of the [`RtcStats`] from the [`SysRtcStats`].

use std::convert::TryFrom;

use derive_more::{Display, From};
use medea_client_api_proto::stats::{RtcStat, RtcStatsType};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};

use crate::utils::{get_property_by_name, JsCaused, JsError};

/// Entry of the [`SysRtcStats`] dictionary.
struct RtcStatsReportEntry(js_sys::JsString, JsValue);

/// Errors which can occur while deserialization of the [`RtcStatsType`].
#[derive(Debug, Display, From, JsCaused)]
pub enum RtcStatsError {
    /// `RTCStats.id` is undefined.
    #[display(fmt = "'RTCStats.id' is undefined.")]
    UndefinedId,

    /// `RTCStats.stats` is undefined.
    #[display(fmt = "'RTCStats.stats' is undefined.")]
    UndefinedStats,

    /// Some JS error occured.
    #[display(fmt = "Unexpected JS-side error: {}", _0)]
    Js(JsError),

    /// `RTCStats.entries` is undefined.
    #[display(fmt = "'RTCStats.entries' is undefined.")]
    EntriesNotFound,

    /// Error while [`RtcStats`] deserialization.
    #[display(fmt = "Error while 'RtcStats' deserialization: {:?}.", _0)]
    ParseError(serde_json::Error),
}

impl TryFrom<js_sys::Array> for RtcStatsReportEntry {
    type Error = Traced<RtcStatsError>;

    fn try_from(value: js_sys::Array) -> Result<Self, Self::Error> {
        let id = value.get(0);
        let stats = value.get(1);

        if id.is_undefined() {
            return Err(tracerr::new!(RtcStatsError::UndefinedId));
        }

        if stats.is_undefined() {
            return Err(tracerr::new!(RtcStatsError::UndefinedStats));
        }

        let id = id
            .dyn_into::<js_sys::JsString>()
            .map_err(|e| tracerr::new!(RtcStatsError::Js(JsError::from(e))))?;
        let stats = stats
            .dyn_into::<JsValue>()
            .map_err(|e| tracerr::new!(RtcStatsError::Js(JsError::from(e))))?;

        Ok(RtcStatsReportEntry(id, stats))
    }
}

/// All available [`RtcStatsType`] of `PeerConnection`.
#[derive(Debug)]
pub struct RtcStats(pub Vec<RtcStat>);

impl TryFrom<&JsValue> for RtcStats {
    type Error = Traced<RtcStatsError>;

    fn try_from(stats: &JsValue) -> Result<Self, Self::Error> {
        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<js_sys::Function>())
            })
            .ok_or_else(|| tracerr::new!(RtcStatsError::EntriesNotFound))?;

        let iterator = entries_fn
            .call0(stats.as_ref())
            .map_err(|e| tracerr::new!(RtcStatsError::Js(JsError::from(e))))?
            .unchecked_into::<js_sys::Iterator>();

        let mut stats = Vec::new();

        for stat in iterator {
            let stat = stat.map_err(|e| {
                tracerr::new!(RtcStatsError::Js(JsError::from(e)))
            })?;
            let stat = stat.unchecked_into::<js_sys::Array>();
            let stat = RtcStatsReportEntry::try_from(stat)
                .map_err(tracerr::map_from_and_wrap!())?;
            let stat: RtcStat = JsValue::from(&stat.1)
                .into_serde()
                .map_err(tracerr::from_and_wrap!())?;

            if let RtcStatsType::Other = &stat.stats {
                continue;
            }

            stats.push(stat);
        }

        Ok(RtcStats(stats))
    }
}
