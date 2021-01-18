//! Representation of [Coturn]'s user.
//!
//! [Coturn]: https://github.com/coturn/coturn

use derive_more::{AsRef, Display, From, Into};
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    log::prelude::*,
    turn::{
        cli::CoturnTelnetClient,
        repo::{TurnDatabase, TurnDatabaseErr},
        COTURN_REALM,
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
    turn_db: TurnDatabase,

    /// Client of [Coturn] server admin interface.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    coturn_cli: CoturnTelnetClient,

    /// Username of the [`IceUser`] for which this [`IceUserHandle`] is
    /// created.
    username: IceUsername,
}

impl IceUserHandle {
    /// Returns new [`IceUserHandle`] for the provided [`IceUsername`].
    pub fn new(
        turn_db: TurnDatabase,
        coturn_cli: CoturnTelnetClient,
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
        let turn_db = self.turn_db.clone();
        let coturn_cli = self.coturn_cli.clone();
        let username = self.username.clone();

        tokio::spawn(async move {
            if let Err(e) = turn_db.remove(&username).await {
                warn!("Failed to remove IceUser from the database: {:?}", e);
            }
            if let Err(e) = coturn_cli.delete_session(&username).await {
                warn!("Failed to remove IceUser from Coturn: {:?}", e);
            }
        });
    }
}

/// Username for authorization on [Coturn] server.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(AsRef, Clone, Debug, Display, From, Into)]
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
    pub async fn new_non_static(
        address: String,
        room_id: &RoomId,
        peer_id: PeerId,
        db: TurnDatabase,
        cli: CoturnTelnetClient,
    ) -> Result<Self, TurnDatabaseErr> {
        let username = IceUsername::new(&room_id, peer_id);
        let pass = IcePassword::generate();

        db.insert(&username, &pass).await?;

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
