use dart_sys::{Dart_Handle, Dart_HandleFromPersistent};
use derive_more::From;
use libc::c_char;

use crate::platform::dart::utils::option::{DartOption, DartStringOption, DartIntOption};
use crate::utils::dart::into_dart_string;
use crate::platform::dart::utils::nullable::{NullableChar, NullableInt};
use crate::{
    platform::dart::utils::handle::DartHandle, utils::dart::from_dart_string,
};

#[derive(From)]
pub struct IceCandidate(DartHandle);

impl From<Dart_Handle> for IceCandidate {
    fn from(handle: Dart_Handle) -> Self {
        Self(DartHandle::new(handle))
    }
}

type CandidateFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut CANDIDATE_FUNCTION: Option<CandidateFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__candidate(
    f: CandidateFunction,
) {
    CANDIDATE_FUNCTION = Some(f);
}

type SdpMLineIndexFunction = extern "C" fn(Dart_Handle) -> DartIntOption;
static mut SDP_M_LINE_INDEX_FUNCTION: Option<SdpMLineIndexFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_m_line_index(
    f: SdpMLineIndexFunction,
) {
    SDP_M_LINE_INDEX_FUNCTION = Some(f);
}

type SdpMidFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut SDP_MID_FUNCTION: Option<SdpMidFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_mid(f: SdpMidFunction) {
    SDP_MID_FUNCTION = Some(f);
}

type NewFunction = extern "C" fn(NullableChar, NullableChar, NullableInt) -> Dart_Handle;
static mut NEW_FUNCTION: Option<NewFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

impl IceCandidate {
    pub fn new(
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Self {
        let handle = unsafe {
            NEW_FUNCTION.unwrap()(Some(candidate.to_string()).into(), sdp_mid.clone().into(), sdp_m_line_index.into())
        };
        Self(DartHandle::new(handle))
    }

    pub fn handle(&self) -> Dart_Handle {
        return self.0.get();
    }

    pub fn candidate(&self) -> String {
        unsafe { Option::from(CANDIDATE_FUNCTION.unwrap()(self.0.get())).unwrap() }
    }

    pub fn sdp_m_line_index(&self) -> Option<u16> {
        unsafe {
            let index: Option<i32> = SDP_M_LINE_INDEX_FUNCTION.unwrap()(self.0.get()).into();
            index.map(|i| i as u16)
        }
    }

    pub fn sdp_mid(&self) -> Option<String> {
        unsafe {
            SDP_MID_FUNCTION.unwrap()(self.0.get()).into()
        }
    }
}
