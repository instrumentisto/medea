//! Implementation of world for the tests.

mod member;

use std::collections::HashMap;

use async_trait::async_trait;
use cucumber_rust::{World, WorldInit};
use derive_more::{Display, Error, From};
use medea_control_api_mock::proto;
use uuid::Uuid;

use crate::{
    browser::{self, RootWebClient},
    control::{self, ControlApi},
    object::{self, jason::Jason, Object},
};

use self::member::Member;

#[doc(inline)]
pub use self::member::MemberBuilder;

#[derive(Debug, Display, Error, From)]
pub enum Error {
    Control(control::Error),
    Object(object::Error),
    Member(member::Error),
    Browser(browser::Error),
    MemberNotFound(#[error(not(source))] String),
}

type Result<T> = std::result::Result<T, Error>;

/// World which will be used by all E2E tests.
#[derive(WorldInit)]
pub struct BrowserWorld {
    /// ID of `Room` created for this [`BrowserWorld`].
    room_id: String,

    /// Client for the Control API.
    control_api: ControlApi,

    /// All [`Member`]s created in this world.
    members: HashMap<String, Member>,

    /// All [`Jason`]s created in this world.
    jasons: Vec<Object<Jason>>,

    /// [WebDriver] client where all objects from this world will be created.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    client: RootWebClient,
}

impl BrowserWorld {
    /// Returns new [`BrowserWorld`] for the provided [`RootWebClient`].
    pub async fn new(client: RootWebClient) -> Result<Self> {
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
            .await?;

        Ok(Self {
            room_id,
            control_api,
            client,
            members: HashMap::new(),
            jasons: Vec::new(),
        })
    }

    /// Creates new [`Member`] from the provided [`MemberBuilder`].
    ///
    /// `Room` for this [`Member`] will be created, but joining will not be
    /// performed.
    pub async fn create_member(
        &mut self,
        builder: MemberBuilder,
    ) -> Result<()> {
        let mut pipeline = HashMap::new();
        if builder.is_send {
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
        if builder.is_recv {
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
                &format!("{}/{}", self.room_id, builder.id),
                proto::Element::Member(proto::Member {
                    id: builder.id.clone(),
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
            .await?;

        if builder.is_send {
            let recv_endpoints: HashMap<_, _> = self
                .members
                .values()
                .filter_map(|m| {
                    if m.is_recv() {
                        let endpoint_id = format!("play-{}", builder.id);
                        Some((
                            format!(
                                "{}/{}/{}",
                                self.room_id,
                                m.id(),
                                endpoint_id
                            ),
                            proto::Element::WebRtcPlayEndpoint(
                                proto::WebRtcPlayEndpoint {
                                    id: endpoint_id,
                                    src: format!(
                                        "local://{}/{}/publish",
                                        self.room_id, builder.id
                                    ),
                                    force_relay: false,
                                },
                            ),
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            for (path, element) in recv_endpoints {
                self.control_api.create(&path, element).await?;
            }
        }
        let jason =
            Object::spawn(Jason, self.client.new_window().await).await?;
        let room = jason.init_room().await?;
        let member = builder.build(room).await?;

        self.members.insert(member.id().to_string(), member);
        self.jasons.push(jason);

        Ok(())
    }

    /// Returns reference to the [`Member`] with a provided ID.
    ///
    /// Returns [`None`] if [`Member`] with a provided ID is not exists.
    pub fn get_member(&self, member_id: &str) -> Option<&Member> {
        self.members.get(member_id)
    }

    /// [`Member`] with a provided ID will be joined to the `Room` created for
    /// this [`BrowserWorld`].
    pub async fn join_room(&mut self, member_id: &str) -> Result<()> {
        let member = self
            .members
            .get_mut(member_id)
            .ok_or_else(|| Error::MemberNotFound(member_id.to_string()))?;
        member.join_room(&self.room_id).await?;
        Ok(())
    }

    /// [`Future`] which will be resolved when [`Member`] with a provided ID
    /// will connect with his partners.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_interconnection(
        &mut self,
        member_id: &str,
    ) -> Result<()> {
        let interconnected_members: Vec<_> = self
            .members
            .values()
            .filter_map(|m| {
                if m.is_joined()
                    && m.id() != member_id
                    && (m.is_recv() || m.is_send())
                {
                    Some(m.id().to_string())
                } else {
                    None
                }
            })
            .collect();
        let member = self
            .members
            .get_mut(member_id)
            .ok_or_else(|| Error::MemberNotFound(member_id.to_string()))?;
        let connections = member.connections();
        for id in interconnected_members {
            connections.wait_for_connection(id).await?;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl World for BrowserWorld {
    type Error = Error;

    async fn new() -> Result<Self> {
        Ok(Self::new(RootWebClient::new().await?).await?)
    }
}
