//! Medea media server member representation.

use derive_more::{Display, Error, From};

use crate::{
    conf,
    object::{
        self, connections_store::ConnectionStore, MediaKind, MediaSourceKind,
        Object, Room,
    },
};

/// All errors which can happen while working with a [`Member`].
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// [`Room`] or a [`ConnectionStore`] object errored.
    Object(object::Error),
}

/// Shortcut for a [`Result`] containing an [`Error`](enum@Error).
///
/// [`Result`]: std::result::Result
type Result<T> = std::result::Result<T, Error>;

/// Builder of a [`Member`].
pub struct Builder {
    /// ID with which a [`Member`] will be created.
    pub id: String,

    /// Indicator whether a [`Member`] will publish media.
    pub is_send: bool,

    /// Indicator whether a [`Member`] will receive media.
    pub is_recv: bool,
}

impl Builder {
    /// Creates a new [`Member`] out of this [`Builder`] configuration.
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

/// [`Object`] representing a `Member` connected to a media server.
pub struct Member {
    /// ID of this [`Member`] on a media server.
    id: String,

    /// Indicator whether this [`Member`] should publish media.
    is_send: bool,

    /// Indicator whether this [`Member`] should receive media.
    is_recv: bool,

    /// Indicator whether this [`Member`] is joined a [`Room`] on a media
    /// server.
    is_joined: bool,

    /// [`Room`]'s [`Object`] that this [`Member`] is intended to join.
    room: Object<Room>,

    /// Storage of [`Connection`]s thrown by this [`Member`]'s [`Room`].
    ///
    /// [`Connection`]: object::connection::Connection
    connection_store: Object<ConnectionStore>,
}

impl Member {
    /// Returns ID of this [`Member`] on a media server.
    #[inline]
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Indicates whether this [`Member`] should publish media.
    #[inline]
    #[must_use]
    pub fn is_send(&self) -> bool {
        self.is_send
    }

    /// Indicator whether this [`Member`] should receive media.
    #[inline]
    #[must_use]
    pub fn is_recv(&self) -> bool {
        self.is_recv
    }

    /// Indicates whether this [`Member`] is joined a [`Room`] on a media
    /// server.
    #[inline]
    #[must_use]
    pub fn is_joined(&self) -> bool {
        self.is_joined
    }

    /// Joins a [`Room`] with the provided ID.
    pub async fn join_room(&mut self, room_id: &str) -> Result<()> {
        self.room
            .join(format!(
                "{}/{}/{}?token=test",
                *conf::CLIENT_API_ADDR,
                room_id,
                self.id,
            ))
            .await?;
        self.is_joined = true;
        Ok(())
    }

    /// Disables media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If the provided `source_kind` is [`None`], then media publishing will be
    /// disabled for all [`MediaSourceKind`]s.
    #[inline]
    pub async fn disable_media_send(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<()> {
        self.room.disable_media_send(kind, source_kind).await?;
        Ok(())
    }

    /// Returns reference to the Storage of [`Connection`]s thrown by this
    /// [`Member`]'s [`Room`].
    ///
    /// [`Connection`]: object::connection::Connection
    #[inline]
    #[must_use]
    pub fn connections(&self) -> &Object<ConnectionStore> {
        &self.connection_store
    }
}