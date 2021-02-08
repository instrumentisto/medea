use derive_more::{Display, Error, From};

use crate::{
    conf,
    entity::{
        self,
        connections_store::ConnectionStore,
        room::{MediaKind, MediaSourceKind, Room},
        Entity,
    },
};

#[derive(Debug, Display, Error, From)]
pub enum Error {
    Entity(entity::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub struct MemberBuilder {
    pub id: String,
    pub is_send: bool,
    pub is_recv: bool,
}

impl MemberBuilder {
    pub async fn build(self, room: Entity<Room>) -> Result<Member> {
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

pub struct Member {
    id: String,
    is_send: bool,
    is_recv: bool,
    is_joined: bool,
    room: Entity<Room>,
    connection_store: Entity<ConnectionStore>,
}

impl Member {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn is_recv(&self) -> bool {
        self.is_recv
    }

    pub fn is_joined(&self) -> bool {
        self.is_joined
    }

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

    pub async fn disable_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<()> {
        self.room.disable_media(kind, source_kind).await?;
        Ok(())
    }

    pub fn connections(&self) -> &Entity<ConnectionStore> {
        &self.connection_store
    }
}
