use crate::{
    conf,
    entity::{room::Room, Entity},
};

pub struct Member {
    id: String,
    is_send: bool,
    is_recv: bool,
    room: Option<Entity<Room>>,
}

impl Member {
    pub fn new(id: String, is_send: bool, is_recv: bool) -> Self {
        Self {
            id,
            is_send,
            is_recv,
            room: None,
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

    pub fn set_room(&mut self, room: Entity<Room>) {
        self.room = Some(room);
    }

    pub async fn join_room(&mut self, room_id: &str) {
        self.room
            .as_mut()
            .unwrap()
            .join(format!(
                "ws://{}/{}/{}?token=test",
                *conf::CLIENT_API_ADDR,
                room_id,
                self.id
            ))
            .await;
    }
}
