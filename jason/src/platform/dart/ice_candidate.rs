use dart_sys::Dart_Handle;
use derive_more::From;
use libc::c_char;

use crate::{
    api::dart::utils::string_into_c_str,
    platform::dart::utils::{
        dart_api::Dart_HandleFromPersistent_DL_Trampolined,
        handle::DartHandle,
        nullable::{NullableChar, NullableInt},
        option::{DartIntOption, DartStringOption},
    },
};

type CandidateFunction = extern "C" fn(Dart_Handle) -> DartStringOption;

type SdpMLineIndexFunction = extern "C" fn(Dart_Handle) -> DartIntOption;

type SdpMidFunction = extern "C" fn(Dart_Handle) -> DartStringOption;

static mut CANDIDATE_FUNCTION: Option<CandidateFunction> = None;

static mut SDP_M_LINE_INDEX_FUNCTION: Option<SdpMLineIndexFunction> = None;

static mut SDP_MID_FUNCTION: Option<SdpMidFunction> = None;

#[derive(From)]
pub struct IceCandidate(DartHandle);

impl From<Dart_Handle> for IceCandidate {
    fn from(handle: Dart_Handle) -> Self {
        Self(DartHandle::new(handle))
    }
}

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__candidate(
    f: CandidateFunction,
) {
    CANDIDATE_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_m_line_index(
    f: SdpMLineIndexFunction,
) {
    SDP_M_LINE_INDEX_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_IceCandidate__sdp_mid(f: SdpMidFunction) {
    SDP_MID_FUNCTION = Some(f);
}

type NewFunction =
    extern "C" fn(*const c_char, NullableChar, NullableInt) -> Dart_Handle;
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
            NEW_FUNCTION.unwrap()(
                string_into_c_str(candidate.to_owned()),
                sdp_mid.clone().into(),
                sdp_m_line_index.into(),
            )
        };
        Self(DartHandle::new(handle))
    }

    pub fn handle(&self) -> Dart_Handle {
        return self.0.get();
    }

    pub fn candidate(&self) -> String {
        unsafe {
            Option::from(CANDIDATE_FUNCTION.unwrap()(self.0.get())).unwrap()
        }
    }

    pub fn sdp_m_line_index(&self) -> Option<u16> {
        unsafe {
            let index: Option<i32> =
                SDP_M_LINE_INDEX_FUNCTION.unwrap()(self.0.get()).into();
            index.map(|i| i as u16)
        }
    }

    pub fn sdp_mid(&self) -> Option<String> {
        unsafe { SDP_MID_FUNCTION.unwrap()(self.0.get()).into() }
    }
}
