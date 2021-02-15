//! Implementation of [`World`][1] for the tests.
//!
//! [1]: cucumber_rust::World

mod member;

use std::{collections::HashMap, time::Duration};

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

use self::member::Member;

#[doc(inline)]
pub use self::member::MemberBuilder;
use medea_control_api_mock::callback::{CallbackEvent, CallbackItem};

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

/// World which will be used by all E2E tests.
#[derive(WorldInit)]
pub struct World {
    /// ID of `Room` created for this [`World`].
    room_id: String,

    /// Client for the Control API.
    control_client: control::Client,

    /// All [`Member`]s created in this [`World`].
    members: HashMap<String, Member>,

    /// All [`Jason`]s created in this [`World`].
    jasons: HashMap<String, Object<Jason>>,

    /// [WebDriver] client where all objects from this world will be created.
    ///
    /// [WebDriver]: https://www.w3.org/TR/webdriver/
    window_factory: WindowFactory,
}

#[async_trait(? Send)]
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

        self.control_client
            .create(
                &format!("{}/{}", self.room_id, builder.id),
                proto::Element::Member(proto::Member {
                    id: builder.id.clone(),
                    pipeline,
                    credentials: Some(proto::Credentials::Plain(
                        "test".to_string(),
                    )),
                    on_join: Some("grpc://127.0.0.1:9099".to_string()),
                    on_leave: Some("grpc://127.0.0.1:9099".to_string()),
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

    /// Returns reference to the [`Member`] with a provided ID.
    ///
    /// Returns [`None`] if [`Member`] with a provided ID is not exists.
    pub fn get_member(&self, member_id: &str) -> Option<&Member> {
        self.members.get(member_id)
    }

    /// [`Member`] with a provided ID will be joined to the `Room` created for
    /// this [`World`].
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

    /// Closes [`Room`] of the provided [`Member`].
    ///
    /// [`Room`]: crate::object::room::Room
    pub async fn close_room(&mut self, member_id: &str) -> Result<()> {
        let jason = self.jasons.get(member_id).unwrap();
        let member = self.members.get(member_id).unwrap();
        let room = member.room();
        jason.close_room(room).await?;
        Ok(())
    }

    /// Wait for [`Member`]'s [`Room`] close.
    ///
    /// [`Room`]: crate::object::room::Room
    pub async fn wait_for_on_close(&self, member_id: &str) -> Result<String> {
        let member = self
            .members
            .get(member_id)
            .ok_or_else(|| Error::MemberNotFound(member_id.to_string()))?;

        Ok(member.room().wait_for_close().await?)
    }

    /// Disposes [`Jason`] object of the provided [`Member`] ID.
    pub async fn dispose_jason(&mut self, member_id: &str) -> Result<()> {
        let jason = self.jasons.remove(member_id).unwrap();
        jason.dispose().await?;
        Ok(())
    }

    /// Deletes Control API element of the [`Member`] with a provided ID.
    pub async fn delete_member_element(&mut self, member_id: &str) {
        let resposne = self
            .control_client
            .delete(&format!("{}/{}", self.room_id, member_id))
            .await
            .unwrap();
        assert!(resposne.error.is_none());
    }

    /// Deletes Control API element of the [`Room`] with a provided ID.
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

    pub async fn wait_for_on_leave(
        &mut self,
        member_id: String,
        reason: String,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            let callbacks = self.get_callbacks().await;
            let on_leave = callbacks
                .into_iter()
                .filter(|e| e.fid.contains(&member_id))
                .find_map(|e| {
                    if let CallbackEvent::OnLeave(on_leave) = e.event {
                        Some(on_leave)
                    } else {
                        None
                    }
                });
            if let Some(on_leave) = on_leave {
                assert_eq!(on_leave.reason.to_string(), reason);
                break;
            }
        }
    }

    pub async fn wait_for_on_join(&mut self, member_id: String) {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            let callbacks = self.get_callbacks().await;
            let on_join_found = callbacks
                .into_iter()
                .filter(|e| e.fid.contains(&member_id))
                .any(|e| matches!(e.event, CallbackEvent::OnJoin(_)));
            if on_join_found {
                break;
            }
        }
    }

    pub async fn get_callbacks(&mut self) -> Vec<CallbackItem> {
        self.control_client
            .callbacks()
            .await
            .unwrap()
            .into_iter()
            .filter(|i| i.fid.contains(&self.room_id))
            .collect()
    }
}
