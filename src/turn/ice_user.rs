//! Representation of [Coturn]'s user.
//!
//! [Coturn]: https://github.com/coturn/coturn

use derive_more::{AsRef, Display, From, Into};
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    log::prelude::*,
    utils::{generate_token, MpscOneshotSender},
};

/// Length of the TURN server credentials.
pub static TURN_PASS_LEN: usize = 16;

/// Username for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display, From, Into, PartialEq, Eq)]
#[as_ref(forward)]
pub struct IceUsername(String);

impl IceUsername {
    /// Returns new [`IceUsername`] for the provided [`RoomId`] and [`PeerId`].
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
    /// Generates new [`IcePassword`] with a [`TURN_PASS_LEN`] length for the
    /// [`IceUser`].
    pub fn generate() -> Self {
        Self(generate_token(TURN_PASS_LEN))
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
    /// `None` if [`IceUser`] is static.
    on_drop: Option<MpscOneshotSender<IceUsername>>,
}

impl IceUser {
    /// Build new non static [`IceUser`].
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

    /// Build new static [`IceUser`].
    pub fn new_static(address: String, username: String, pass: String) -> Self {
        Self {
            address,
            username: IceUsername(username),
            pass: IcePassword(pass),
            on_drop: None,
        }
    }

    /// Build vector of [`IceServer`].
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

    pub fn user(&self) -> &IceUsername {
        &self.username
    }

    pub fn pass(&self) -> &IcePassword {
        &self.pass
    }
}

impl Drop for IceUser {
    fn drop(&mut self) {
        if let Some(tx) = self.on_drop.take() {
            let name = std::mem::take(&mut self.username.0);
            if let Err(user) = tx.send(IceUsername(name)) {
                warn!("Failed to cleanup IceUser: {}", user);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::{channel::mpsc, StreamExt as _};

    use super::*;

    #[actix_rt::test]
    async fn ice_user_removes_on_drop() {
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
