use dart_sys::{Dart_Handle, _Dart_Handle};
use derive_more::From;

use crate::{
    platform::dart::utils::handle::DartHandle,
    utils::dart::{from_dart_string, into_dart_string},
};

#[derive(From)]
pub struct IceCandidate(DartHandle);

impl From<Dart_Handle> for IceCandidate {
    fn from(handle: Dart_Handle) -> Self {
        Self(DartHandle::new(handle))
    }
}

type CandidateFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut CANDIDATE_FUNCTION: Option<CandidateFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__candidate(
    f: CandidateFunction,
) {
    CANDIDATE_FUNCTION = Some(f);
}

type SdpMLineIndexFunction = extern "C" fn(Dart_Handle) -> i32;
static mut SDP_M_LINE_INDEX_FUNCTION: Option<SdpMLineIndexFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_m_line_index(
    f: SdpMLineIndexFunction,
) {
    SDP_M_LINE_INDEX_FUNCTION = Some(f);
}

type SdpMidFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut SDP_MID_FUNCTION: Option<SdpMidFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_mid(f: SdpMidFunction) {
    SDP_MID_FUNCTION = Some(f);
}

impl IceCandidate {
    pub fn new(
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Self {
        // TODO: Do with me something
        unsafe {
            // NEW.unwrap(into_dart_string(candidate.to_string()))
        }
        todo!()
    }

    pub fn handle(&self) -> Dart_Handle {
        return self.0.get();
    }

    pub fn candidate(&self) -> String {
        unsafe { from_dart_string(CANDIDATE_FUNCTION.unwrap()(self.0.get())) }
    }

    pub fn sdp_m_line_index(&self) -> Option<u16> {
        unsafe {
            // TODO: make it optional
            Some(SDP_M_LINE_INDEX_FUNCTION.unwrap()(self.0.get()) as u16)
        }
    }

    pub fn sdp_mid(&self) -> Option<String> {
        unsafe {
            // TODO: make it optional
            Some(from_dart_string(SDP_MID_FUNCTION.unwrap()(self.0.get())))
        }
    }
}
