//! Implementation of [`World`][1] for the tests.
//!
//! [1]: cucumber_rust::World

mod member;

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use cucumber_rust::WorldInit;
use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    callback::{CallbackEvent, CallbackItem},
    proto,
};
use tokio_1 as tokio;
use uuid::Uuid;

use crate::{
    browser::{self, WindowFactory},
    control,
    object::{self, Jason, Object},
};

use self::member::Member;

#[doc(inline)]
pub use self::member::MemberBuilder;
use medea_control_api_mock::proto::PublishPolicy;

/// Returns Control API path for the provided `room_id`, `member_id` and
/// `endpoint_id`.
macro_rules! control_api_path {
    ($room_id:expr) => {
        format!("{}", $room_id)
    };
    ($room_id:expr, $member_id:expr) => {
        format!("{}/{}", $room_id, $member_id)
    };
    ($room_id:expr, $member_id:expr, $endpoint_id:expr) => {
        format!("{}/{}/{}", $room_id, $member_id, $endpoint_id)
    };
}

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
                    idle_timeout: Some(Duration::from_millis(500)),
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
        let window = self.window_factory.new_window().await;
        let jason =
            Object::spawn(Jason, window.clone())
                .await?;
        let room = jason.init_room().await?;
        let member = builder.build(room, window).await?;

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

    /// Returns [`Future`] which will be resolved when `OnLeave` Control API
    /// callback will be received for the provided [`Member`] ID.
    ///
    /// Panics if `OnLeave` reason is not equal to the provided one.
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

    /// Returns [`Future`] which will be resolved when `OnJoin` Control API
    /// callback will be received for the provided [`Member`] ID.
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

    /// Returns all [`CallbackItem`]s sent by Control API for this [`World`]'s
    /// `Room`.
    pub async fn get_callbacks(&mut self) -> Vec<CallbackItem> {
        self.control_client
            .callbacks()
            .await
            .unwrap()
            .into_iter()
            .filter(|i| i.fid.contains(&self.room_id))
            .collect()
    }

    /// Creates `WebRtcPublishEndpoint`s and `WebRtcPlayEndpoint`s for the
    /// provided [`MembersPair`].
    pub async fn interconnect_members(
        &mut self,
        pair: MembersPair,
    ) -> Result<()> {
        if let Some(publish_endpoint) = pair.left.publish_endpoint() {
            self.control_client
                .create(
                    &control_api_path!(self.room_id, pair.left.id, "publish"),
                    publish_endpoint.into(),
                )
                .await?;
        }
        if let Some(publish_endpoint) = pair.right.publish_endpoint() {
            self.control_client
                .create(
                    &control_api_path!(self.room_id, pair.right.id, "publish"),
                    publish_endpoint.into(),
                )
                .await?;
        }

        if let Some(play_endpoint) =
            pair.left.play_endpoint_for(&self.room_id, &pair.right)
        {
            self.control_client
                .create(
                    &control_api_path!(
                        self.room_id,
                        pair.left.id,
                        play_endpoint.id
                    ),
                    play_endpoint.into(),
                )
                .await?;
        }
        if let Some(play_endpoint) =
            pair.right.play_endpoint_for(&self.room_id, &pair.left)
        {
            self.control_client
                .create(
                    &control_api_path!(
                        self.room_id,
                        pair.right.id,
                        play_endpoint.id
                    ),
                    play_endpoint.into(),
                )
                .await?;
        }

        {
            let left_member = self.members.get_mut(&pair.left.id).unwrap();
            left_member.set_is_send(pair.left.is_send());
            left_member.set_is_recv(pair.right.recv);
        }
        {
            let right_member = self.members.get_mut(&pair.right.id).unwrap();
            right_member.set_is_send(pair.right.is_send());
            right_member.set_is_recv(pair.right.recv);
        }

        self.control_client
            .get(&control_api_path!(self.room_id))
            .await
            .unwrap();

        Ok(())
    }
}

/// `Member`s pairing configuration.
///
/// Based on this configuration [`World`] can dynamically create `Endpoint`s for
/// this `Member`s.
pub struct MembersPair {
    pub left: PairedMember,
    pub right: PairedMember,
}

/// `Endpoint`s configuration of `Member`.
pub struct PairedMember {
    pub id: String,
    pub send_audio: Option<proto::AudioSettings>,
    pub send_video: Option<proto::VideoSettings>,
    pub recv: bool,
}

impl PairedMember {
    /// Returns `true` if this [`PairedMember`] should publish media.
    fn is_send(&self) -> bool {
        self.send_audio.is_some() || self.send_video.is_some()
    }

    /// Returns [`proto::WebRtcPublishEndpoint`] for this [`PairedMember`] if
    /// publishing is enabled.
    fn publish_endpoint(&self) -> Option<proto::WebRtcPublishEndpoint> {
        if self.is_send() {
            Some(proto::WebRtcPublishEndpoint {
                id: "publish".to_string(),
                p2p: proto::P2pMode::Always,
                force_relay: false,
                audio_settings: self.send_audio.clone().unwrap_or_else(|| {
                    proto::AudioSettings {
                        publish_policy: PublishPolicy::Disabled,
                    }
                }),
                video_settings: self.send_video.clone().unwrap_or_else(|| {
                    proto::VideoSettings {
                        publish_policy: PublishPolicy::Disabled,
                    }
                }),
            })
        } else {
            None
        }
    }

    /// Returns [`proto::WebRtcPlayEndpoint`] for this [`PairedMember`] which
    /// will receive media from the provided [`PairedMember`] if receiving is
    /// enabled.
    fn play_endpoint_for(
        &self,
        room_id: &str,
        publisher: &PairedMember,
    ) -> Option<proto::WebRtcPlayEndpoint> {
        if self.recv {
            Some(proto::WebRtcPlayEndpoint {
                id: format!("play-{}", publisher.id),
                src: format!(
                    "local://{}/{}/{}",
                    room_id, publisher.id, "publish"
                ),
                force_relay: false,
            })
        } else {
            None
        }
    }
}
