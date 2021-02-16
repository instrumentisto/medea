//! Representation of Media Server Member used in tests.

use derive_more::{Display, Error, From};

use crate::{
    conf,
    object::{
        self, connections_store::ConnectionStore, MediaKind, MediaSourceKind,
        Object, Room,
    },
};

/// All errors which can happen while working with [`Member`].
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// [`Room`] or [`ConnectionStore`] object errored.
    Object(object::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Builder for the [`Member`].
pub struct MemberBuilder {
    /// ID with which [`Member`] will be created.
    pub id: String,

    /// Flag which indicates that [`Member`] will publish media.
    pub is_send: bool,

    /// Flag which indicates that [`Member`] will receive media.
    pub is_recv: bool,
}

impl MemberBuilder {
    /// Creates new [`Member`] with a [`MemberBuilder`] configuration.
    pub async fn build(self, room: Object<Room>) -> Result<Member> {
        let connection_store = room.connections_store().await?;
        Ok(Member {
            id: self.id,
            is_send: self.is_send,
            is_recv: self.is_recv,
            is_joined: false,
            room,
            connection_store,
        })
    }
}

/// Object which represents some connected to the Media Server `Member`.
pub struct Member {
    /// ID of [`Member`] on the Media Server.
    id: String,

    /// Flag which indicates that [`Member`] should publish media.
    is_send: bool,

    /// Flag which indicates that [`Member`] should receive media.
    is_recv: bool,

    /// Flag which indicates that [`Member`] is joined to the `Room`.
    is_joined: bool,

    /// Representation of the `Room` JS object.
    room: Object<Room>,

    /// Storage for the [`Connection`]s throws by this [`Member`]'s `Room`.
    ///
    /// [`Connection`]: crate::object::connection::Connection
    connection_store: Object<ConnectionStore>,
}

impl Member {
    /// Returns ID of [`Member`] on the Media Server.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns flag which indicates that [`Member`] should publish media.
    pub fn is_send(&self) -> bool {
        self.is_send
    }

    /// Returns flag which indicates that [`Member`] should receive media.
    pub fn is_recv(&self) -> bool {
        self.is_recv
    }

    /// Returns flag which indicates that [`Member`] is joined to the `Room`.
    pub fn is_joined(&self) -> bool {
        self.is_joined
    }

    /// Joins into `Room` with a provided ID.
    pub async fn join_room(&mut self, room_id: &str) -> Result<()> {
        self.room
            .join(format!(
                "{}/{}/{}?token=test",
                *conf::CLIENT_API_ADDR,
                room_id,
                self.id
            ))
            .await?;
        self.is_joined = true;
        Ok(())
    }

    /// Disabled media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media publishing will be
    /// disabled for all [`MediaSourceKind`]s.
    pub async fn disable_media_send(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<()> {
        self.room.disable_media_send(kind, source_kind).await?;
        Ok(())
    }

    /// Returns reference to the storage for the [`Connection`]s throws by this
    /// [`Member`]'s `Room`.
    ///
    /// [`Connection`]: crate::object::connection::Connection
    pub fn connections(&self) -> &Object<ConnectionStore> {
        &self.connection_store
    }
}