//! Wrapper for array of [`RtcIceServer`][1].
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtciceserver-dictionary
use std::ops::Deref;

use medea_client_api_proto::IceServer;
use wasm_bindgen::JsValue;
use web_sys::RtcIceServer;

/// Wrapper for array of [`RtcIceServer`][1].
///
/// [1]: https://www.w3.org/TR/webrtc/#rtciceserver-dictionary
pub struct RtcIceServers(js_sys::Array);

impl From<Vec<IceServer>> for RtcIceServers {
    fn from(servers: Vec<IceServer>) -> Self {
        let inner = js_sys::Array::new();

        for ice_server in servers {
            let mut server = RtcIceServer::new();

            let urls = js_sys::Array::new();
            for url in ice_server.urls {
                urls.push(&JsValue::from(url));
            }

            server.urls(&urls);

            if let Some(credential) = ice_server.credential {
                server.credential(&credential);
            }
            if let Some(username) = ice_server.username {
                server.username(&username);
            }

            inner.push(&server.into());
        }

        Self(inner)
    }
}

impl Deref for RtcIceServers {
    type Target = js_sys::Array;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
