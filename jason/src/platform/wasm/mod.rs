use std::{
    borrow::Cow,
    convert::{TryFrom as _, TryInto as _},
    time::Duration,
};

pub mod constraints;
pub mod ice_server;
pub mod input_device_info;
pub mod media_track;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver;
pub mod transport;
pub mod utils;

use derive_more::Display;
use futures::Future;
use js_sys::{Promise, Reflect};
use tracerr::Traced;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

use input_device_info::InputDeviceInfo;

use crate::{
    core::media::MediaManagerError,
    platform::{
        DisplayMediaStreamConstraints, MediaStreamConstraints, MediaStreamTrack,
    },
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see:
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

pub fn init_logger() {
    wasm_logger::init(wasm_logger::Config::default());
}

#[inline]
pub fn spawn<F>(task: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(task);
}

/// [`Future`] which resolves after the provided [`JsDuration`].
///
/// [`Future`]: std::future::Future
pub async fn delay_for(delay: Duration) {
    let delay_ms = delay.as_millis().try_into().unwrap_or(i32::max_value());
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes, delay_ms,
            )
            .unwrap();
    }))
    .await
    .unwrap();
}

pub async fn enumerate_devices(
) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
    use MediaManagerError::{CouldNotGetMediaDevices, EnumerateDevicesFailed};

    let devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;
    let devices = JsFuture::from(
        devices
            .enumerate_devices()
            .map_err(Error::from)
            .map_err(EnumerateDevicesFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map_err(Error::from)
    .map_err(EnumerateDevicesFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::Array::from(&devices)
        .values()
        .into_iter()
        .filter_map(|info| {
            let info = web_sys::MediaDeviceInfo::from(info.unwrap());
            InputDeviceInfo::try_from(info).ok()
        })
        .collect())
}

pub async fn get_user_media(
    caps: MediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    use MediaManagerError::{CouldNotGetMediaDevices, GetUserMediaFailed};

    let media_devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;

    let stream = JsFuture::from(
        media_devices
            .get_user_media_with_constraints(&caps.into())
            .map_err(Error::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map(web_sys::MediaStream::from)
    .map_err(Error::from)
    .map_err(GetUserMediaFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::try_iter(&stream.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .collect())
}

pub async fn get_display_media(
    caps: DisplayMediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    use MediaManagerError::{
        CouldNotGetMediaDevices, GetDisplayMediaFailed, GetUserMediaFailed,
    };

    let media_devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;

    let stream = JsFuture::from(
        media_devices
            .get_display_media_with_constraints(&caps.into())
            .map_err(Error::from)
            .map_err(GetDisplayMediaFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map(web_sys::MediaStream::from)
    .map_err(Error::from)
    .map_err(GetUserMediaFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::try_iter(&stream.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .collect())
}

/// Wrapper for JS value which returned from JS side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: Cow<'static, str>,

    /// Message of JS error.
    pub message: Cow<'static, str>,
}

impl From<JsValue> for Error {
    fn from(val: JsValue) -> Self {
        match val.dyn_into::<js_sys::Error>() {
            Ok(err) => Self {
                name: Cow::Owned(err.name().into()),
                message: Cow::Owned(err.message().into()),
            },
            Err(val) => match val.as_string() {
                Some(reason) => Self {
                    name: "Unknown JS error".into(),
                    message: reason.into(),
                },
                None => Self {
                    name: "Unknown JS error".into(),
                    message: format!("{:?}", val).into(),
                },
            },
        }
    }
}

impl From<Error> for js_sys::Error {
    fn from(err: Error) -> Self {
        let error = Self::new(&err.message);
        error.set_name(&err.name);
        error
    }
}

/// Returns property of JS object by name if its defined.
/// Converts the value with a given predicate.
pub fn get_property_by_name<T, F, U>(
    value: &T,
    name: &str,
    into: F,
) -> Option<U>
where
    T: AsRef<wasm_bindgen::JsValue>,
    F: Fn(wasm_bindgen::JsValue) -> Option<U>,
{
    Reflect::get(value.as_ref(), &JsValue::from_str(name))
        .ok()
        .map_or_else(|| None, into)
}

/// Returns [`Window`] object.
///
/// # Panics
///
/// When global [`Window`] object is inaccessible.
pub fn window() -> Window {
    // Cannot use `lazy_static` since `window` is `!Sync`.
    // Safe to unwrap.
    web_sys::window().unwrap()
}

/// Wrapper around interval timer ID.
pub struct IntervalHandle(pub i32);

impl Drop for IntervalHandle {
    /// Clears interval with provided ID.
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}
