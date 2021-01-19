//! Representation of [Coturn]'s user.
//!
//! [Coturn]: https://github.com/coturn/coturn

use derive_more::{AsRef, Display, From, Into};
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    log::prelude::*,
    turn::{
        repo::TurnDatabaseErr, TurnDatabase, TurnSessionManager, COTURN_REALM,
    },
    utils::generate_pass,
};

/// Length of the TURN server credentials.
static TURN_PASS_LEN: usize = 16;

/// [`IceUser`] handle which will remove it's credentials from remote storage
/// and forcibly closes it's sessions on [Coturn] server.
#[derive(Debug)]
struct IceUserHandle {
    /// Turn credentials repository.
    turn_db: Box<dyn TurnDatabase>,

    /// Client of [Coturn] server admin interface.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    coturn_cli: Box<dyn TurnSessionManager>,

    /// Username of the [`IceUser`] for which this [`IceUserHandle`] is
    /// created.
    username: IceUsername,
}

impl IceUserHandle {
    /// Returns new [`IceUserHandle`] for the provided [`IceUsername`].
    pub fn new(
        turn_db: Box<dyn TurnDatabase>,
        coturn_cli: Box<dyn TurnSessionManager>,
        username: IceUsername,
    ) -> Self {
        Self {
            turn_db,
            coturn_cli,
            username,
        }
    }
}

impl Drop for IceUserHandle {
    fn drop(&mut self) {
        let remove_task = self.turn_db.remove(&self.username);
        let delete_task = self.coturn_cli.delete_session(&self.username);

        tokio::spawn(async move {
            if let Err(e) = remove_task.await {
                warn!("Failed to remove IceUser from the database: {:?}", e);
            }
            if let Err(e) = delete_task.await {
                warn!("Failed to remove IceUser from Coturn: {:?}", e);
            }
        });
    }
}

/// Username for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display, From, Into)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[as_ref(forward)]
pub struct IceUsername(String);

impl IceUsername {
    /// Returns new [`IceUsername`] for the provided [`RoomId`] and [`PeerId`].
    fn new(room_id: &RoomId, peer_id: PeerId) -> Self {
        Self(format!("{}_{}", room_id, peer_id))
    }

    /// Returns Redis key for this [`IceUsername`].
    pub fn as_redis_key(&self) -> String {
        format!("turn/realm/{}/user/{}/key", COTURN_REALM, self)
    }
}

/// Password for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display)]
pub struct IcePassword(String);

impl IcePassword {
    /// Returns [`IcePassword`] for the [`IceUserKind::Static`] [`IceUser`].
    fn new_static(pass: String) -> Self {
        Self(pass)
    }

    /// Generates new [`IcePassword`] with a [`TURN_PASS_LEN`] length for the
    /// [`IceUser`].
    fn generate() -> Self {
        Self(generate_pass(TURN_PASS_LEN))
    }
}

/// Kind of [`IceUser`].
#[derive(Debug)]
enum IceUserKind {
    /// Static users are hardcoded on Turn server and do not
    /// require any additional management.
    Static,

    /// Non static users are meant to be saved and deleted from some remote
    /// storage
    NonStatic(IceUserHandle),
}

/// Credentials on Turn server.
///
/// If this [`IceUser`] is [`IceUserKind::NonStatic`], then on [`Drop::drop`]
/// all records about this user and it's sessions will be removed.
#[derive(Debug)]
pub struct IceUser {
    /// Address of Turn server.
    address: String,

    /// Username for authorization.
    username: IceUsername,

    /// Password for authorization.
    pass: IcePassword,

    /// Kind of this [`IceUser`].
    kind: IceUserKind,
}

impl IceUser {
    /// Build new non static [`IceUser`].
    ///
    /// Creates new credentials for provided [`RoomId`] and [`PeerId`], inserts
    /// it to the [`TurnDatabase`].
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    pub async fn new_non_static(
        address: String,
        room_id: &RoomId,
        peer_id: PeerId,
        db: Box<dyn TurnDatabase>,
        cli: Box<dyn TurnSessionManager>,
    ) -> Result<Self, TurnDatabaseErr> {
        let username = IceUsername::new(&room_id, peer_id);
        let pass = IcePassword::generate();

        let insert_ice_user_fut = db.insert(&username, &pass);
        insert_ice_user_fut.await?;

        Ok(Self {
            address,
            kind: IceUserKind::NonStatic(IceUserHandle::new(
                db,
                cli,
                username.clone(),
            )),
            username,
            pass,
        })
    }

    /// Build new static [`IceUser`].
    pub fn new_static(address: String, username: String, pass: String) -> Self {
        Self {
            address,
            username: IceUsername(username),
            pass: IcePassword::new_static(pass),
            kind: IceUserKind::Static,
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
}

#[cfg(test)]
mod tests {
    use std::mem::forget;

    use futures::future;

    use super::*;

    use crate::turn::{MockTurnDatabase, MockTurnSessionManager};

    /// Tests that on [`IceUser::new_non_static`] record in the [`TurnDatabase`]
    /// will be created.
    #[tokio::test]
    async fn ice_user_creates_record_in_db() {
        let room_id = RoomId::from("foobar");
        let peer_id = PeerId(0);
        let ice_username = IceUsername::new(&room_id, peer_id);

        let mut db = MockTurnDatabase::new();
        db.expect_insert().times(1).returning(move |user, _| {
            assert_eq!(*user, ice_username);

            Box::pin(future::ok(()))
        });

        forget(
            IceUser::new_non_static(
                String::new(),
                &room_id,
                peer_id,
                Box::new(db),
                Box::new(MockTurnSessionManager::new()),
            )
            .await
            .unwrap(),
        );
    }

    /// Tests that on [`IceUser`] drop record from the [`TurnDatabase`] will be
    /// removed and [`IceUser`]'s sessions will be removed.
    #[tokio::test]
    async fn ice_user_removes_on_drop() {
        let room_id = RoomId::from("foobar");
        let peer_id = PeerId(0);
        let ice_username = IceUsername::new(&room_id, peer_id);

        let mut db = MockTurnDatabase::new();
        db.expect_remove().times(1).returning({
            let ice_username = ice_username.clone();
            move |user| {
                assert_eq!(*user, ice_username);
                Box::pin(future::ok(()))
            }
        });
        db.expect_insert()
            .returning(|_, _| Box::pin(future::ok(())));
        let mut session_manager = MockTurnSessionManager::new();
        session_manager.expect_delete_session().times(1).returning(
            move |user| {
                assert_eq!(*user, ice_username);
                Box::pin(future::ok(()))
            },
        );

        drop(
            IceUser::new_non_static(
                String::new(),
                &room_id,
                peer_id,
                Box::new(db),
                Box::new(session_manager),
            )
            .await
            .unwrap(),
        );
    }
}
