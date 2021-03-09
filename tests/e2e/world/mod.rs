//! E2E tests [`World`][1].
//!
//! [1]: cucumber_rust::World

pub mod member;

use std::collections::HashMap;

use async_trait::async_trait;
use cucumber_rust::WorldInit;
use derive_more::{Display, Error, From};
use medea_control_api_mock::proto;
use uuid::Uuid;

use crate::{
    browser::{self, WindowFactory},
    control,
    object::{self, Jason, Object},
};

pub use self::member::{Builder as MemberBuilder, Member};

/// All errors which can happen while working with [`World`].
#[derive(Debug, Display, Error, From)]
pub enum Error {
    Control(control::Error),
    Object(object::Error),
    Member(member::Error),
    Browser(browser::Error),
    MemberNotFound(#[error(not(source))] String),
}

type Result<T> = std::result::Result<T, Error>;

/// [`World`][1] used by all E2E tests.
///
/// [1]: cucumber_rust::World
#[derive(WorldInit)]
pub struct World {
    /// ID of the `Room` created for this [`World`].
    room_id: String,

    /// Client of a Medea Control API.
    control_client: control::Client,

    /// All [`Member`]s created in this [`World`].
    members: HashMap<String, Member>,

    /// All [`Jason`] [`Object`]s created in this [`World`].
    jasons: HashMap<String, Object<Jason>>,

    /// [WebDriver] client that all [`Object`]s of this [`World`] will be
    /// created with.
    ///
    /// [WebDriver]: https://w3.org/TR/webdriver
    window_factory: WindowFactory,
}

#[async_trait(?Send)]
impl cucumber_rust::World for World {
    type Error = Error;

    async fn new() -> Result<Self> {
        let room_id = Uuid::new_v4().to_string();

        let control_client = control::Client::new();
        control_client
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
            control_client,
            window_factory: WindowFactory::new().await?,
            members: HashMap::new(),
            jasons: HashMap::new(),
        })
    }
}

impl World {
    /// Creates a new [`Member`] from the provided [`MemberBuilder`].
    ///
    /// `Room` for this [`Member`] will be created, but joining won't be done.
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
                                m.id(),
                            ),
                            force_relay: false,
                        },
                    ),
                );
            });
        }

        self.control_client
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
                    m.is_recv().then(|| {
                        let endpoint_id = format!("play-{}", builder.id);
                        let id = format!(
                            "{}/{}/{}",
                            self.room_id,
                            m.id(),
                            endpoint_id,
                        );
                        let elem = proto::Element::WebRtcPlayEndpoint(
                            proto::WebRtcPlayEndpoint {
                                id: endpoint_id,
                                src: format!(
                                    "local://{}/{}/publish",
                                    self.room_id, builder.id,
                                ),
                                force_relay: false,
                            },
                        );
                        (id, elem)
                    })
                })
                .collect();
            for (path, element) in recv_endpoints {
                self.control_client.create(&path, element).await?;
            }
        }
        let jason =
            Object::spawn(Jason, self.window_factory.new_window().await)
                .await?;
        let room = jason.init_room().await?;
        let member = builder.build(room).await?;

        self.jasons.insert(member.id().to_string(), jason);
        self.members.insert(member.id().to_string(), member);

        Ok(())
    }

    /// Returns reference to a [`Member`] with the provided ID.
    ///
    /// Returns [`None`] if a [`Member`] with the provided ID doesn't exist.
    #[inline]
    #[must_use]
    pub fn get_member(&self, member_id: &str) -> Option<&Member> {
        self.members.get(member_id)
    }

    /// Joins a [`Member`] with the provided ID to the `Room` created for this
    /// [`World`].
    pub async fn join_room(&mut self, member_id: &str) -> Result<()> {
        let member = self
            .members
            .get_mut(member_id)
            .ok_or_else(|| Error::MemberNotFound(member_id.to_string()))?;
        member.join_room(&self.room_id).await?;
        Ok(())
    }

    /// Waits until a [`Member`] with the provided ID will connect with his
    /// responders.
    pub async fn wait_for_interconnection(
        &mut self,
        member_id: &str,
    ) -> Result<()> {
        let interconnected_members = self.members.values().filter(|m| {
            m.is_joined() && m.id() != member_id && (m.is_recv() || m.is_send())
        });
        let member = self.members.get(member_id).unwrap();
        for partner in interconnected_members {
            let (send_count, recv_count) =
                member.count_of_tracks_between_members(partner);
            let conn = member
                .connections()
                .wait_for_connection(partner.id().to_string())
                .await?;
            conn.tracks_store()
                .await?
                .wait_for_count(recv_count)
                .await?;

            let partner_conn = partner
                .connections()
                .wait_for_connection(member_id.to_string())
                .await?;
            partner_conn
                .tracks_store()
                .await?
                .wait_for_count(send_count)
                .await?;
        }

        Ok(())
    }

    /// Closes a [`Room`] of the provided [`Member`].
    ///
    /// [`Room`]: crate::object::room::Room
    pub async fn close_room(&mut self, member_id: &str) -> Result<()> {
        let jason = self.jasons.get(member_id).unwrap();
        let member = self.members.get(member_id).unwrap();
        let room = member.room();
        jason.close_room(room).await?;
        Ok(())
    }

    /// Waist for the [`Member`]'s [`Room`] being closed.
    ///
    /// [`Room`]: crate::object::room::Room
    pub async fn wait_for_on_close(&self, member_id: &str) -> Result<String> {
        let member = self
            .members
            .get(member_id)
            .ok_or_else(|| Error::MemberNotFound(member_id.to_string()))?;

        Ok(member.room().wait_for_close().await?)
    }

    /// Disposes a [`Jason`] object of the provided [`Member`] ID.
    pub async fn dispose_jason(&mut self, member_id: &str) -> Result<()> {
        let jason = self.jasons.remove(member_id).unwrap();
        jason.dispose().await?;
        Ok(())
    }

    /// Deletes a Control API element of the [`Member`] with the provided ID.
    pub async fn delete_member_element(&mut self, member_id: &str) {
        let resposne = self
            .control_client
            .delete(&format!("{}/{}", self.room_id, member_id))
            .await
            .unwrap();
        assert!(resposne.error.is_none());
    }

    /// Deletes a Control API element of the [`Room`] with the provided ID.
    ///
    /// [`Room`]: crate::object::room::Room
    pub async fn delete_room_element(&mut self) {
        let resp = self
            .control_client
            .delete(self.room_id.as_str())
            .await
            .unwrap();
        assert!(resp.error.is_none());
    }
}
