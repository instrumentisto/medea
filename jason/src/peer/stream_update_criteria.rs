use std::ops::BitOrAssign;

use medea_client_api_proto::{Direction, MediaSourceKind, MediaType, Track};

use crate::MediaKind;

bitflags::bitflags! {
    pub struct Inner: u8 {
        const DEVICE_AUDIO =  0b0001;
        const DISPLAY_AUDIO = 0b0010;
        const DEVICE_VIDEO =  0b0100;
        const DISPLAY_VIDEO = 0b1000;
    }
}

/// Used for local stream updates. Allows to specify a set of [`MediaKind`] +
/// [`MediaSourceKind`] pairs.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LocalStreamUpdateCriteria(Inner);

impl LocalStreamUpdateCriteria {
    /// Creates [`LocalStreamUpdateCriteria`] with all [`MediaKind`] +
    /// [`MediaSourceKind`] combinations.
    pub fn all() -> Self {
        Self(Inner::all())
    }

    /// Creates empty [`LocalStreamUpdateCriteria`].
    pub fn empty() -> Self {
        Self(Inner::empty())
    }

    /// Creates [`LocalStreamUpdateCriteria`] with provided [`MediaKind`] +
    /// [`MediaSourceKind`] pair. `None` `source_kind` means both
    /// [`MediaSourceKind`]s.
    pub fn from_kinds(
        media_kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Self {
        use MediaKind as MK;
        use MediaSourceKind as SK;

        let inner = match (source_kind, media_kind) {
            (None, MK::Audio) => Inner::DEVICE_AUDIO | Inner::DISPLAY_AUDIO,
            (Some(SK::Device), MK::Audio) => Inner::DEVICE_AUDIO,
            (Some(SK::Display), MK::Audio) => Inner::DISPLAY_AUDIO,
            (None, MK::Video) => Inner::DEVICE_VIDEO | Inner::DISPLAY_VIDEO,
            (Some(SK::Device), MK::Video) => Inner::DEVICE_VIDEO,
            (Some(SK::Display), MK::Video) => Inner::DISPLAY_VIDEO,
        };
        Self(inner)
    }

    /// Builds [`LocalStreamUpdateCriteria`] from provided tracks. Only
    /// [`Direction::Send`] are taken into account.
    pub fn from_tracks(tracks: &[Track]) -> LocalStreamUpdateCriteria {
        let mut result = LocalStreamUpdateCriteria::empty();
        for track in tracks
            .iter()
            .filter(|t| matches!(t.direction, Direction::Send { .. }))
        {
            match &track.media_type {
                MediaType::Audio(_) => {
                    result.add(MediaKind::Audio, MediaSourceKind::Device);
                }
                MediaType::Video(video) => {
                    result.add(MediaKind::Video, video.source_kind);
                }
            }
        }
        result
    }

    /// Adds provided [`MediaKind`] + [`MediaSourceKind`] pair.
    pub fn add(&mut self, media_kind: MediaKind, source_kind: MediaSourceKind) {
        self.0
            .bitor_assign(Self::from_kinds(media_kind, Some(source_kind)).0)
    }

    /// Checks whether this [`LocalStreamUpdateCriteria`] contains provided
    /// [`MediaKind`] + [`MediaSourceKind`] pair.
    pub fn has(
        self,
        media_kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> bool {
        self.0
            .contains(Self::from_kinds(media_kind, Some(source_kind)).0)
    }
}
