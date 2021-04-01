//! Definitions and implementations of [Control API]'s `Room` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom, time::Duration};

use medea_client_api_proto::{MemberId, RoomId as Id};
use medea_control_api_proto::grpc::api as proto;
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl, member::Credential, EndpointId,
    TryFromProtobufError,
};

use super::{
    member::{MemberElement, MemberSpec},
    pipeline::Pipeline,
    RootElement, TryFromElementError,
};

/// Element of [`Room`]'s [`Pipeline`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RoomElement {
    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member {
        spec: Pipeline<EndpointId, MemberElement>,
        credentials: Credential,
        on_leave: Option<CallbackUrl>,
        on_join: Option<CallbackUrl>,
        #[serde(default, with = "humantime_serde")]
        idle_timeout: Option<Duration>,
        #[serde(default, with = "humantime_serde")]
        reconnect_timeout: Option<Duration>,
        #[serde(default, with = "humantime_serde")]
        ping_interval: Option<Duration>,
    },
}

/// [Control API]'s `Room` element specification.
///
/// Newtype for [`RootElement::Room`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Pipeline<MemberId, RoomElement>,
}

impl RoomSpec {
    /// Returns all [`MemberSpec`]s of this [`RoomSpec`].
    ///
    /// # Errors
    ///
    /// Errors with [`TryFromElementError::NotMember`] if no [`MemberSpec`]
    /// was found in this [`RoomSpec`]'s pipeline.
    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, MemberSpec>, TryFromElementError> {
        let mut members: HashMap<MemberId, MemberSpec> = HashMap::new();
        for (control_id, element) in self.pipeline.iter() {
            let member_spec = MemberSpec::try_from(element)?;

            members.insert(control_id.clone(), member_spec);
        }

        Ok(members)
    }

    /// Returns ID of this [`RoomSpec`]
    #[inline]
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl TryFrom<&RootElement> for RoomSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RootElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RootElement) -> Result<Self, Self::Error> {
        match from {
            RootElement::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: spec.clone(),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

/// Implement [`TryFrom`] proto element for [`RoomSpec`].
macro_rules! impl_from_el_for_room_spec {
    ($proto_el:path) => {
        impl TryFrom<$proto_el> for RoomSpec {
            type Error = TryFromProtobufError;

            fn try_from(proto: $proto_el) -> Result<Self, Self::Error> {
                use $proto_el as proto_el;

                let id = match proto {
                    proto_el::Room(room) => {
                        let mut pipeline = HashMap::new();
                        for (id, room_element) in room.pipeline {
                            if let Some(elem) = room_element.el {
                                let member = MemberSpec::try_from((
                                    MemberId(id.clone()),
                                    elem,
                                ))?;
                                pipeline.insert(id.into(), member.into());
                            } else {
                                return Err(
                                    TryFromProtobufError::EmptyElement(id),
                                );
                            }
                        }

                        let pipeline = Pipeline::new(pipeline);
                        return Ok(Self {
                            id: room.id.into(),
                            pipeline,
                        });
                    }
                    proto_el::Member(member) => member.id,
                    proto_el::WebrtcPub(webrtc_pub) => webrtc_pub.id,
                    proto_el::WebrtcPlay(webrtc_play) => webrtc_play.id,
                };

                Err(TryFromProtobufError::ExpectedOtherElement(
                    String::from("Room"),
                    id,
                ))
            }
        }
    };
}

impl_from_el_for_room_spec!(proto::create_request::El);
impl_from_el_for_room_spec!(proto::apply_request::El);
