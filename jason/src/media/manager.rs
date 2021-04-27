//! Acquiring and storing [`local::Track`]s.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use derive_more::Display;
use medea_client_api_proto::MediaSourceKind;
use tracerr::Traced;

use crate::{
    media::{
        track::MediaStreamTrackState, MediaKind, MediaStreamSettings,
        MultiSourceTracksConstraints,
    },
    platform,
    utils::JsCaused,
};

use super::track::local;

/// Errors that may occur in a [`MediaManager`].
#[derive(Clone, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum MediaManagerError {
    /// Occurs when cannot get access to [MediaDevices][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#mediadevices
    #[display(fmt = "Navigator.mediaDevices() failed: {}", _0)]
    CouldNotGetMediaDevices(platform::Error),

    /// Occurs if the [getUserMedia][1] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    #[display(fmt = "MediaDevices.getUserMedia() failed: {}", _0)]
    GetUserMediaFailed(platform::Error),

    /// Occurs if the [getDisplayMedia()][1] request failed.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[display(fmt = "MediaDevices.getDisplayMedia() failed: {}", _0)]
    GetDisplayMediaFailed(platform::Error),

    /// Occurs when cannot get info about connected [MediaDevices][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#mediadevices
    #[display(fmt = "MediaDevices.enumerateDevices() failed: {}", _0)]
    EnumerateDevicesFailed(platform::Error),

    /// Occurs when local track is [`ended`][1] right after [getUserMedia()][2]
    /// or [getDisplayMedia()][3] request.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    /// [2]: https://tinyurl.com/rnxcavf
    /// [3]: https://w3.org/TR/screen-capture#dom-mediadevices-getdisplaymedia
    #[display(fmt = "{} track is ended", _0)]
    LocalTrackIsEnded(MediaKind),

    /// [`MediaManagerHandle`]'s inner [`Weak`] pointer could not be upgraded.
    #[display(fmt = "MediaManagerHandle is in detached state")]
    Detached,
}

type Result<T> = std::result::Result<T, Traced<MediaManagerError>>;

/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to
/// [`local::Track`]s, so if there are no strong references to some track,
/// then this track is stopped and deleted from [`MediaManager`].
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(Default)]
pub struct MediaManager(Rc<InnerMediaManager>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained tracks storage
    tracks: Rc<RefCell<HashMap<String, Weak<local::Track>>>>,
}

impl InnerMediaManager {
    /// Returns a list of [`platform::InputDeviceInfo`] objects.
    #[inline]
    async fn enumerate_devices() -> Result<Vec<platform::InputDeviceInfo>> {
        platform::enumerate_devices().await
    }

    /// Obtains [`local::Track`]s based on a provided
    /// [`MediaStreamSettings`]. This can be the tracks that were acquired
    /// earlier, or new tracks, acquired via [getUserMedia()][1] or/and
    /// [getDisplayMedia()][2] requests.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] if [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    async fn get_tracks(
        &self,
        mut caps: MediaStreamSettings,
    ) -> Result<Vec<(Rc<local::Track>, bool)>> {
        let tracks_from_storage = self
            .get_from_storage(&mut caps)
            .into_iter()
            .map(|t| (t, false));
        match caps.into() {
            None => Ok(tracks_from_storage.collect()),
            Some(MultiSourceTracksConstraints::Display(caps)) => {
                Ok(tracks_from_storage
                    .chain(
                        self.get_display_media(caps)
                            .await?
                            .into_iter()
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
            Some(MultiSourceTracksConstraints::Device(caps)) => {
                Ok(tracks_from_storage
                    .chain(
                        self.get_user_media(caps)
                            .await?
                            .into_iter()
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
            Some(MultiSourceTracksConstraints::DeviceAndDisplay(
                device_caps,
                display_caps,
            )) => {
                let device_tracks = self.get_user_media(device_caps).await?;
                let display_tracks =
                    self.get_display_media(display_caps).await?;
                Ok(tracks_from_storage
                    .chain(
                        device_tracks
                            .into_iter()
                            .chain(display_tracks.into_iter())
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
        }
    }

    /// Tries to find [`local::Track`]s that satisfies [`MediaStreamSettings`],
    /// from tracks that were acquired earlier to avoid redundant
    /// [getUserMedia()][1]/[getDisplayMedia()][2] calls.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    fn get_from_storage(
        &self,
        caps: &mut MediaStreamSettings,
    ) -> Vec<Rc<local::Track>> {
        // cleanup weak links
        self.tracks
            .borrow_mut()
            .retain(|_, track| Weak::strong_count(track) > 0);

        let mut tracks = Vec::new();
        let storage: Vec<_> = self
            .tracks
            .borrow()
            .iter()
            .map(|(_, track)| Weak::upgrade(track).unwrap())
            .collect();

        if caps.is_audio_enabled() {
            let track = storage
                .iter()
                .find(|&track| caps.get_audio().satisfies(track.as_ref()))
                .cloned();

            if let Some(track) = track {
                caps.set_audio_publish(false);
                tracks.push(track);
            }
        }

        tracks.extend(
            storage
                .iter()
                .filter(|&track| {
                    caps.unconstrain_if_satisfies_video(track.as_ref())
                })
                .cloned(),
        );

        tracks
    }

    /// Obtains new [`local::Track`]s making [getUserMedia()][1] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    async fn get_user_media(
        &self,
        caps: platform::MediaStreamConstraints,
    ) -> Result<Vec<Rc<local::Track>>> {
        let tracks = platform::get_user_media(caps).await?;

        Ok(self.parse_and_save_tracks(tracks, MediaSourceKind::Device)?)
    }

    /// Obtains [`local::Track`]s making [getDisplayMedia()][1] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    async fn get_display_media(
        &self,
        caps: platform::DisplayMediaStreamConstraints,
    ) -> Result<Vec<Rc<local::Track>>> {
        let tracks = platform::get_display_media(caps).await?;

        Ok(self.parse_and_save_tracks(tracks, MediaSourceKind::Display)?)
    }

    /// Retrieves tracks from provided [`platform::MediaStreamTrack`]s, saves
    /// tracks weak references in [`MediaManager`] tracks storage.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::LocalTrackIsEnded`] if at least one track from
    /// the provided [`platform::MediaStreamTrack`]s is in [`ended`][1] state.
    ///
    /// In case of error all tracks are ended and are not saved in
    /// [`MediaManager`]'s tracks storage.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    #[allow(clippy::needless_pass_by_value)]
    fn parse_and_save_tracks(
        &self,
        tracks: Vec<platform::MediaStreamTrack>,
        kind: MediaSourceKind,
    ) -> Result<Vec<Rc<local::Track>>> {
        use MediaManagerError::LocalTrackIsEnded;

        let mut storage = self.tracks.borrow_mut();

        // Tracks returned by getDisplayMedia()/getUserMedia() request should be
        // `live`. Otherwise, we should err without caching tracks in
        // `MediaManager`. Tracks will be stopped on `Drop`.
        for track in &tracks {
            if track.ready_state() != MediaStreamTrackState::Live {
                return Err(tracerr::new!(LocalTrackIsEnded(track.kind())));
            }
        }

        let tracks = tracks
            .into_iter()
            .map(|tr| Rc::new(local::Track::new(tr, kind)))
            .inspect(|track| {
                storage.insert(track.id(), Rc::downgrade(track));
            })
            .collect();

        Ok(tracks)
    }
}

impl MediaManager {
    /// Obtains [`local::Track`]s based on a provided [`MediaStreamSettings`].
    /// This can be the tracks that were acquired earlier, or new tracks,
    /// acquired via [getUserMedia()][1] or/and [getDisplayMedia()][2] requests.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] if [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub async fn get_tracks<I: Into<MediaStreamSettings>>(
        &self,
        caps: I,
    ) -> Result<Vec<(Rc<local::Track>, bool)>> {
        self.0.get_tracks(caps.into()).await
    }

    /// Instantiates a new [`MediaManagerHandle`] for external usage.
    #[inline]
    #[must_use]
    pub fn new_handle(&self) -> MediaManagerHandle {
        MediaManagerHandle(Rc::downgrade(&self.0))
    }
}

/// External handle to a [`MediaManager`].
///
/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to [`local::Track`]s, so if there
/// are no strong references to some track, then this track is stopped and
/// deleted from [`MediaManager`].
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(Clone)]
pub struct MediaManagerHandle(Weak<InnerMediaManager>);

#[allow(clippy::unused_self)]
impl MediaManagerHandle {
    /// Returns a list of [`platform::InputDeviceInfo`] objects representing
    /// available media input and output devices, such as microphones, cameras,
    /// and so forth.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::CouldNotGetMediaDevices`] or
    /// [`MediaManagerError::EnumerateDevicesFailed`] if devices enumeration
    /// failed.
    pub async fn enumerate_devices(
        &self,
    ) -> Result<Vec<platform::InputDeviceInfo>> {
        InnerMediaManager::enumerate_devices()
            .await
            .map_err(tracerr::wrap!(=> MediaManagerError))
    }

    /// Returns [`local::LocalMediaTrack`]s objects, built from the provided
    /// [`MediaStreamSettings`].
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::Detached`] if [`Weak`] pointer upgrade fails.
    ///
    /// With [`MediaManagerError::CouldNotGetMediaDevices`] if media devices
    /// request to User Agent failed.
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] if [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub async fn init_local_tracks(
        &self,
        caps: MediaStreamSettings,
    ) -> Result<Vec<local::LocalMediaTrack>> {
        let this = self
            .0
            .upgrade()
            .ok_or_else(|| tracerr::new!(MediaManagerError::Detached))?;
        this.get_tracks(caps)
            .await
            .map(|tracks| {
                tracks
                    .into_iter()
                    .map(|(t, _)| local::LocalMediaTrack::new(t))
                    .collect::<Vec<_>>()
            })
            .map_err(tracerr::wrap!(=> MediaManagerError))
    }
}
