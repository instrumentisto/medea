//! Medea media server member representation.

use std::{cell::RefCell, collections::HashMap};

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
        let mut media_state = HashMap::new();
        media_state.insert((MediaKind::Audio, MediaSourceKind::Device), true);
        media_state.insert((MediaKind::Video, MediaSourceKind::Device), true);
        media_state.insert((MediaKind::Video, MediaSourceKind::Display), false);
        Ok(Member {
            id: self.id,
            is_send: self.is_send,
            is_recv: self.is_recv,
            is_joined: false,
            send_state: RefCell::new(media_state.clone()),
            recv_state: RefCell::new(media_state),
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

    /// Publishing media state of this [`Member`].
    ///
    /// If value is `true` then this [`MediaKind`] and [`MediaSourceKind`] is
    /// enabled.
    send_state: RefCell<HashMap<(MediaKind, MediaSourceKind), bool>>,

    /// Receiving media state of this [`Member`].
    ///
    /// If value is `true` then this [`MediaKind`] and [`MediaSourceKind`] is
    /// enabled.
    recv_state: RefCell<HashMap<(MediaKind, MediaSourceKind), bool>>,

    /// Representation of the `Room` JS object.
    room: Object<Room>,

    /// Storage for the [`Connection`]s throws by this [`Member`]'s `Room`.
    ///
    /// [`Connection`]: crate::object::connection::Connection
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

    /// Returns list of [`MediaKind`]s and [`MediaSourceKind`] based on the
    /// provided [`Option`]s.
    fn kinds_and_source_kinds(
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
    ) -> Vec<(MediaKind, MediaSourceKind)> {
        let mut kinds_and_source_kinds = Vec::new();
        if let Some(kind) = kind {
            if let Some(source_kind) = source_kind {
                kinds_and_source_kinds.push((kind, source_kind));
            } else {
                kinds_and_source_kinds.push((kind, MediaSourceKind::Device));
            }
        } else if let Some(source_kind) = source_kind {
            kinds_and_source_kinds.push((MediaKind::Audio, source_kind));
            kinds_and_source_kinds.push((MediaKind::Video, source_kind));
        } else {
            kinds_and_source_kinds
                .push((MediaKind::Video, MediaSourceKind::Device));
            kinds_and_source_kinds
                .push((MediaKind::Audio, MediaSourceKind::Device));
        }

        kinds_and_source_kinds
    }

    /// Updates [`Member::send_state`].
    fn update_media_state(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        enabled: bool,
    ) {
        for (kind, source_kind) in
            Self::kinds_and_source_kinds(kind, source_kind)
        {
            *self
                .send_state
                .borrow_mut()
                .entry((kind, source_kind))
                .or_insert_with(|| enabled) = enabled;
        }
    }

    /// Updates [`Member::recv_state`].
    fn update_recv_media_state(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        enabled: bool,
    ) {
        for (kind, source_kind) in
            Self::kinds_and_source_kinds(kind, source_kind)
        {
            *self
                .recv_state
                .borrow_mut()
                .entry((kind, source_kind))
                .or_insert_with(|| enabled) = enabled;
        }
    }

    /// Returns count of [`LocalTrack`]s and [`RemoteTrack`]s of this [`Member`]
    /// with a provided partner [`Member`].
    ///
    /// [`LocalTrack`]: crate::object::local_track::LocalTrack
    /// [`RemoteTrack`]: crate::object::remote_track::RemoteTrack
    pub fn count_of_tracks_between_members(
        &self,
        another: &Self,
    ) -> (u64, u64) {
        let send_count = self
            .send_state
            .borrow()
            .iter()
            .filter(|(key, enabled)| {
                another
                    .recv_state
                    .borrow()
                    .get(key)
                    .copied()
                    .unwrap_or(false)
                    && **enabled
            })
            .count() as u64;
        let recv_count = self
            .recv_state
            .borrow()
            .iter()
            .filter(|(key, enabled)| {
                another
                    .send_state
                    .borrow()
                    .get(key)
                    .copied()
                    .unwrap_or(false)
                    && **enabled
            })
            .count() as u64;

        (send_count, recv_count)
    }

    /// Toggles media state of this [`Member`]'s [`Room`].
    pub async fn toggle_media(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        enabled: bool,
    ) -> Result<()> {
        self.update_media_state(kind, source_kind, enabled);
        if enabled {
            if let Some(kind) = kind {
                self.room.enable_media_send(kind, source_kind).await?;
            } else {
                self.room
                    .enable_media_send(MediaKind::Video, source_kind)
                    .await?;
                self.room
                    .enable_media_send(MediaKind::Audio, source_kind)
                    .await?;
            }
        } else if let Some(kind) = kind {
            self.room.disable_media_send(kind, source_kind).await?;
        } else {
            self.room
                .disable_media_send(MediaKind::Audio, source_kind)
                .await?;
            self.room
                .disable_media_send(MediaKind::Video, source_kind)
                .await?;
        }
        Ok(())
    }

    /// Toggles mute state of this [`Member`]'s [`Room`].
    pub async fn toggle_mute(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        muted: bool,
    ) -> Result<()> {
        if muted {
            if let Some(kind) = kind {
                self.room.mute_media(kind, source_kind).await?;
            } else {
                self.room.mute_media(MediaKind::Audio, source_kind).await?;
                self.room.mute_media(MediaKind::Video, source_kind).await?;
            }
        } else if let Some(kind) = kind {
            self.room.unmute_media(kind, source_kind).await?;
        } else {
            self.room
                .unmute_media(MediaKind::Audio, source_kind)
                .await?;
            self.room
                .unmute_media(MediaKind::Video, source_kind)
                .await?;
        }

        Ok(())
    }

    /// Toggles remote media state of this [`Member`]'s [`Room`].
    pub async fn toggle_remote_media(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        enabled: bool,
    ) -> Result<()> {
        self.update_recv_media_state(kind, source_kind, enabled);
        if enabled {
            if let Some(kind) = kind {
                self.room.enable_remote_media(kind, source_kind).await?;
            } else {
                self.room
                    .enable_remote_media(MediaKind::Audio, source_kind)
                    .await?;
                self.room
                    .enable_remote_media(MediaKind::Video, source_kind)
                    .await?;
            }
        } else if let Some(kind) = kind {
            self.room.disable_remote_media(kind, source_kind).await?;
        } else {
            self.room
                .disable_remote_media(MediaKind::Audio, source_kind)
                .await?;
            self.room
                .disable_remote_media(MediaKind::Video, source_kind)
                .await?;
        }

        Ok(())
    }

    /// Returns reference to the Storage of [`Connection`]s thrown by this
    /// [`Member`]'s [`Room`].
    ///
    /// [`Connection`]: object::connection::Connection
    pub fn connections(&self) -> &Object<ConnectionStore> {
        &self.connection_store
    }

    /// Returns reference to the [`Room`] of this [`Member`].
    pub fn room(&self) -> &Object<Room> {
        &self.room
    }
}
