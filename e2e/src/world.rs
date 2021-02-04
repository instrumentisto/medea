//! Implementation of world for the tests.

use std::{collections::HashMap, convert::Infallible};

use async_trait::async_trait;
use cucumber_rust::{World, WorldInit};
use medea_control_api_mock::proto;
use uuid::Uuid;

use crate::{
    browser::{JsExecutable, WebClient},
    control::ControlApi,
    entity::{room::Room, Builder, Entity},
    model::member::Member,
};
use medea_control_api_mock::api::endpoint::AudioSettings;

/// World which will be used by all E2E tests.
#[derive(WorldInit)]
pub struct BrowserWorld {
    room_id: String,
    entity_factory: EntityFactory,
    control_api: ControlApi,
    members: HashMap<String, Member>,
}

impl BrowserWorld {
    pub async fn new(mut client: WebClient) -> Self {
        client
            .execute(JsExecutable::new(
                r#"
                async () => {
                    window.holders = new Map();
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();

        let room_id = Uuid::new_v4().to_string();
        let control_api = ControlApi::new();
        control_api
            .create(
                &room_id,
                proto::Element::Room(proto::Room {
                    id: room_id.clone(),
                    pipeline: HashMap::new(),
                }),
            )
            .await
            .unwrap();

        Self {
            room_id,
            entity_factory: EntityFactory(client),
            control_api,
            members: HashMap::new(),
        }
    }

    pub async fn create_member(&mut self, mut member: Member) {
        let mut pipeline = HashMap::new();
        if member.is_send() {
            pipeline.insert(
                "publish".to_string(),
                proto::Endpoint::WebRtcPublishEndpoint(
                    proto::WebRtcPublishEndpoint {
                        id: "publish".to_string(),
                        p2p: proto::P2pMode::Always,
                        force_relay: false,
                        audio_settings: proto::AudioSettings::default(),
                        video_settings: proto::VideoSettings::default(),
                    },
                ),
            );
        }
        if member.is_recv() {
            self.members.values().filter(|m| m.is_send()).for_each(|m| {
                let endpoint_id = format!("play-{}", m.id());
                pipeline.insert(
                    endpoint_id.clone(),
                    proto::Endpoint::WebRtcPlayEndpoint(
                        proto::WebRtcPlayEndpoint {
                            id: endpoint_id,
                            src: format!(
                                "local://{}/{}/publish",
                                self.room_id,
                                m.id()
                            ),
                            force_relay: false,
                        },
                    ),
                );
            });
        }

        self.control_api
            .create(
                &format!("{}/{}", self.room_id, member.id()),
                proto::Element::Member(proto::Member {
                    id: member.id().to_string(),
                    pipeline,
                    credentials: Some(proto::Credentials::Plain(
                        "test".to_string(),
                    )),
                    on_join: None,
                    on_leave: None,
                    idle_timeout: None,
                    reconnect_timeout: None,
                    ping_interval: None,
                }),
            )
            .await
            .unwrap();

        if member.is_send() {
            let recv_endpoints: HashMap<_, _> = self
                .members
                .values()
                .filter(|m| m.is_recv())
                .map(|m| {
                    let endpoint_id = format!("play-{}", member.id());
                    (
                        format!("{}/{}/{}", self.room_id, m.id(), endpoint_id),
                        proto::Element::WebRtcPlayEndpoint(
                            proto::WebRtcPlayEndpoint {
                                id: endpoint_id,
                                src: format!(
                                    "local://{}/{}/publish",
                                    self.room_id,
                                    member.id()
                                ),
                                force_relay: false,
                            },
                        ),
                    )
                })
                .collect();
            for (path, element) in recv_endpoints {
                self.control_api.create(&path, element).await.unwrap();
            }
        }
        let room = self.entity_factory.new_entity(Room).await;
        member.set_room(room);

        self.members.insert(member.id().to_string(), member);
    }

    pub fn get_member(&mut self, member_id: &str) -> &mut Member {
        self.members.get_mut(member_id).unwrap()
    }

    pub async fn join_room(&mut self, member_id: &str) {
        let member = self.members.get_mut(member_id).unwrap();
        member.join_room(&self.room_id).await;
    }
}

#[async_trait(?Send)]
impl World for BrowserWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        // TODO: unwrap
        Ok(Self::new(WebClient::new().await.unwrap()).await)
    }
}

struct EntityFactory(WebClient);

impl EntityFactory {
    pub async fn new_entity<T>(&mut self, obj: T) -> Entity<T>
    where
        T: Builder,
    {
        let id = Uuid::new_v4().to_string();
        self.0
            .execute(obj.build().and_then(JsExecutable::new(
                r#"
                    async (obj) => {
                        const [id] = args;
                        window.holders.set(id, obj);
                    }
                "#,
                vec![id.clone().into()],
            )))
            .await
            .unwrap();

        Entity::new(id, self.0.clone())
    }
}
