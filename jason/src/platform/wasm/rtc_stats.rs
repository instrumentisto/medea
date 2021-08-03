//! Deserialization of [`RtcStats`] from [`SysRtcStats`].
//!
//! [`SysRtcStats`]: web_sys::RtcStats

use std::{convert::TryFrom, rc::Rc};

use js_sys::{
    Array as JsArray, Function as JsFunction, Iterator as JsIterator, JsString,
};
use medea_client_api_proto::stats::{RtcStat, RtcStatsType};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};

use crate::platform::{self, wasm::get_property_by_name, RtcStatsError};

/// All available [`RtcStatsType`]s of a [`platform::RtcPeerConnection`].
#[derive(Clone, Debug)]
pub struct RtcStats(pub Vec<RtcStat>);

impl TryFrom<&JsValue> for RtcStats {
    type Error = Traced<RtcStatsError>;

    fn try_from(stats: &JsValue) -> Result<Self, Self::Error> {
        use RtcStatsError::{Platform, UndefinedEntries};

        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<JsFunction>())
            })
            .ok_or_else(|| tracerr::new!(UndefinedEntries))?;

        let iterator = entries_fn
            .call0(stats.as_ref())
            .map_err(|e| tracerr::new!(Platform(platform::error::from(e))))?
            .unchecked_into::<JsIterator>();

        let mut stats = Vec::new();

        for stat in iterator {
            let stat = stat.map_err(|e| {
                tracerr::new!(Platform(platform::error::from(e)))
            })?;
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
/// Entry of a JS RTC stats dictionary.
struct RtcStatsReportEntry(JsString, JsValue);

impl TryFrom<JsArray> for RtcStatsReportEntry {
    type Error = Traced<RtcStatsError>;

    fn try_from(value: JsArray) -> Result<Self, Self::Error> {
        use RtcStatsError::{Platform, UndefinedId, UndefinedStats};

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
            .map_err(|e| tracerr::new!(Platform(platform::error::from(e))))?;
        let stats = stats
            .dyn_into::<JsValue>()
            .map_err(|e| tracerr::new!(Platform(platform::error::from(e))))?;

        Ok(RtcStatsReportEntry(id, stats))
    }
}
