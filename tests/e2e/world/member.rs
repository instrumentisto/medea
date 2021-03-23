//! Medea media server member representation.

use std::{cell::RefCell, collections::HashMap};

use derive_more::{Display, Error, From};

use crate::{
    browser::{mock, Window},
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
    pub async fn build(
        self,
        room: Object<Room>,
        window: Window,
    ) -> Result<Member> {
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
            window,
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

    /// Media publishing state of this [`Member`].
    ///
    /// If value is `true` then this [`MediaKind`] and [`MediaSourceKind`] is
    /// enabled.
    send_state: RefCell<HashMap<(MediaKind, MediaSourceKind), bool>>,

    /// Media receiving state of this [`Member`].
    ///
    /// If value is `true` then this [`MediaKind`] and [`MediaSourceKind`] is
    /// enabled.
    recv_state: RefCell<HashMap<(MediaKind, MediaSourceKind), bool>>,

    /// [`Room`]'s [`Object`] that this [`Member`] is intended to join.
    room: Object<Room>,

    /// Storage of [`Connection`]s thrown by this [`Member`]'s [`Room`].
    ///
    /// [`Connection`]: object::connection::Connection
    connection_store: Object<ConnectionStore>,

    /// [`Window`] in which this [`Member`] is exists.
    window: Window,
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

    /// Updates flag indicating that this [`Member`] should publish media.
    #[inline]
    pub fn set_is_send(&mut self, is_send: bool) {
        self.is_send = is_send;
    }

    /// Updates flag indicating that this [`Member`] should receive media.
    #[inline]
    pub fn set_is_recv(&mut self, is_recv: bool) {
        self.is_recv = is_recv;
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

    /// Updates [`Member::send_state`].
    fn update_media_state(
        &self,
        kind: Option<MediaKind>,
        source_kind: Option<MediaSourceKind>,
        enabled: bool,
    ) {
        for (kind, source_kind) in kinds_combinations(kind, source_kind) {
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
        for (kind, source_kind) in kinds_combinations(kind, source_kind) {
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
    #[must_use]
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
    #[inline]
    #[must_use]
    pub fn connections(&self) -> &Object<ConnectionStore> {
        &self.connection_store
    }

    /// Returns reference to the [`Room`] of this [`Member`].
    #[inline]
    #[must_use]
    pub fn room(&self) -> &Object<Room> {
        &self.room
    }

    /// Returns WebAPI `WebSocket` mock object for [`Window`] of this
    /// [`Member`].
    #[inline]
    #[must_use]
    pub fn ws_mock(&self) -> mock::WebSocket {
        self.window.websocket_mock()
    }

    /// Returns a [MediaDevices.getUserMedia()][1] mock for [`Window`] of this
    /// [`Member`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    #[inline]
    #[must_use]
    pub fn media_devices_mock(&self) -> mock::MediaDevices {
        self.window.media_devices_mock()
    }
}

/// Returns list of [`MediaKind`]s and [`MediaSourceKind`] based on the provided
/// [`Option`]s.
#[must_use]
fn kinds_combinations(
    kind: Option<MediaKind>,
    source_kind: Option<MediaSourceKind>,
) -> Vec<(MediaKind, MediaSourceKind)> {
    let mut out = Vec::with_capacity(2);
    if let Some(kind) = kind {
        if let Some(source_kind) = source_kind {
            out.push((kind, source_kind));
        } else {
            out.push((kind, MediaSourceKind::Device));
        }
    } else if let Some(source_kind) = source_kind {
        out.push((MediaKind::Audio, source_kind));
        out.push((MediaKind::Video, source_kind));
    } else {
        out.push((MediaKind::Video, MediaSourceKind::Device));
        out.push((MediaKind::Audio, MediaSourceKind::Device));
    }
    out
}
