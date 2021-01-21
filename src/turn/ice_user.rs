//! Representation of [Coturn]'s user.
//!
//! [Coturn]: https://github.com/coturn/coturn

use std::mem;

use derive_more::{AsRef, Display, From, Into};
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    log::prelude as log,
    utils::{generate_token, MpscOneshotSender},
};

/// Username for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display, Eq, From, Into, PartialEq)]
#[as_ref(forward)]
pub struct IceUsername(String);

impl IceUsername {
    /// Returns new [`IceUsername`] for the provided [`RoomId`] and [`PeerId`].
    #[must_use]
    fn new(room_id: &RoomId, peer_id: PeerId) -> Self {
        Self(format!("{}_{}", room_id, peer_id))
    }
}

/// Password for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display)]
pub struct IcePassword(String);

impl IcePassword {
    /// Length of an [`IcePassword`] on [Coturn] server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub const LENGTH: usize = 16;

    /// Generates a new random [`IcePassword`] with for the [`IceUser`].
    #[inline]
    #[must_use]
    pub fn generate() -> Self {
        Self(generate_token(Self::LENGTH))
    }
}

/// Credentials on Turn server.
#[derive(Debug)]
pub struct IceUser {
    /// Address of Turn server.
    address: String,

    /// Username for authorization.
    username: IceUsername,

    /// Password for authorization.
    pass: IcePassword,

    /// Sender into which [`IceUsername`] is sent in [`Drop`] implementation.
    ///
    /// [`None`] if [`IceUser`] is static.
    on_drop: Option<MpscOneshotSender<IceUsername>>,
}

impl IceUser {
    /// Builds a new non-static [`IceUser`].
    #[inline]
    #[must_use]
    pub fn new_non_static(
        address: String,
        room_id: &RoomId,
        peer_id: PeerId,
        pass: IcePassword,
        on_drop: MpscOneshotSender<IceUsername>,
    ) -> Self {
        Self {
            address,
            username: IceUsername::new(&room_id, peer_id),
            pass,
            on_drop: Some(on_drop),
        }
    }

    /// Build a new static [`IceUser`].
    #[inline]
    #[must_use]
    pub fn new_static(address: String, username: String, pass: String) -> Self {
        Self {
            address,
            username: IceUsername(username),
            pass: IcePassword(pass),
            on_drop: None,
        }
    }

    /// Builds a list of [`IceServer`]s of this [`IceUser`].
    #[must_use]
    pub fn servers_list(&self) -> Vec<IceServer> {
        let stun_url = vec![format!("stun:{}", self.address)];
        let stun = IceServer {
            urls: stun_url,
            username: None,
            credential: None,
        };
        let turn_urls = vec![
            format!("turn:{}", self.address),
            format!("turn:{}?transport=tcp", self.address),
        ];
        let turn = IceServer {
            urls: turn_urls,
            username: Some(self.username.to_string()),
            credential: Some(self.pass.to_string()),
        };
        vec![stun, turn]
    }

    /// Returns [`IceUsername`] of this [`IceUser`].
    #[inline]
    #[must_use]
    pub fn user(&self) -> &IceUsername {
        &self.username
    }

    /// Returns [`IcePassword`] of this [`IceUser`].
    #[inline]
    #[must_use]
    pub fn pass(&self) -> &IcePassword {
        &self.pass
    }
}

impl Drop for IceUser {
    fn drop(&mut self) {
        if let Some(tx) = self.on_drop.take() {
            let name = mem::take(&mut self.username.0);
            if let Err(user) = tx.send(IceUsername(name)) {
                log::warn!("Failed to cleanup IceUser: {}", user);
            }
        }
    }
}

#[cfg(test)]
mod spec {
    use futures::{channel::mpsc, StreamExt as _};

    use super::*;

    #[actix_rt::test]
    async fn removes_from_coturn_on_drop() {
        let (tx, mut rx) = mpsc::unbounded();

        let user = IceUser::new_non_static(
            String::new(),
            &RoomId::from("foobar"),
            PeerId(0),
            IcePassword::generate(),
            MpscOneshotSender::from(tx),
        );
        let user_name = user.username.clone();

        drop(user);

        assert_eq!(rx.next().await.unwrap(), user_name);
        assert!(rx.next().await.is_none());
    }
}
