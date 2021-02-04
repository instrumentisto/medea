use crate::{
    conf,
    entity::{
        connections_store::ConnectionStore,
        room::{MediaKind, MediaSourceKind, Room},
        Entity,
    },
};

pub struct Member {
    id: String,
    is_send: bool,
    is_recv: bool,
    room: Option<Entity<Room>>,
    connection_store: Option<Entity<ConnectionStore>>,
}

impl Member {
    pub fn new(id: String, is_send: bool, is_recv: bool) -> Self {
        Self {
            id,
            is_send,
            is_recv,
            room: None,
            connection_store: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn is_recv(&self) -> bool {
        self.is_recv
    }

    pub async fn set_room(&mut self, mut room: Entity<Room>) {
        self.connection_store = Some(room.connections_store().await);
        self.room = Some(room);
    }

    pub async fn join_room(&mut self, room_id: &str) {
        self.room
            .as_mut()
            .unwrap()
            .join(format!(
                "{}/{}/{}?token=test",
                *conf::CLIENT_API_ADDR,
                room_id,
                self.id
            ))
            .await;
    }

    pub async fn disable_media(
        &mut self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) {
        self.room
            .as_mut()
            .unwrap()
            .disable_media(kind, source_kind)
            .await;
    }

    pub fn connections(&mut self) -> &mut Entity<ConnectionStore> {
        self.connection_store.as_mut().unwrap()
    }
}
