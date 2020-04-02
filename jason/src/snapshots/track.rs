use futures::Stream;
use medea_client_api_proto::{
    snapshots::track::TrackSnapshotAccessor, Direction, MediaType, TrackId,
};
use medea_reactive::ObservableCell;

#[derive(Debug)]
pub struct ObservableTrackSnapshot {
    id: TrackId,
    is_muted: ObservableCell<bool>,
    direction: Direction,
    media_type: MediaType,
}

impl ObservableTrackSnapshot {
    pub fn on_track_update(&self) -> impl Stream<Item = bool> {
        self.is_muted.subscribe()
    }

    pub fn get_direction(&self) -> &Direction {
        &self.direction
    }

    pub fn get_media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn get_is_muted(&self) -> bool {
        self.is_muted.get()
    }

    pub fn get_id(&self) -> TrackId {
        self.id
    }
}

impl TrackSnapshotAccessor for ObservableTrackSnapshot {
    fn new(
        id: TrackId,
        is_muted: bool,
        direction: Direction,
        media_type: MediaType,
    ) -> Self {
        Self {
            id,
            is_muted: ObservableCell::new(is_muted),
            direction,
            media_type,
        }
    }

    fn set_is_muted(&mut self, is_muted: bool) {
        self.is_muted.set(is_muted);
    }
}
