//! Definitions and implementations of [Control API]'s `Room` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom};

use derive_more::{Display, From};
use medea_control_api_proto::grpc::api as proto;
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl, EndpointId, TryFromProtobufError,
};

use super::{
    member::{MemberElement, MemberSpec},
    pipeline::Pipeline,
    MemberId, RootElement, TryFromElementError,
};
use crate::api::control::endpoints::webrtc_play_endpoint::{
    Unvalidated, Validated, ValidationError,
};

/// ID of [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From)]
pub struct Id(pub String);

/// Element of [`Room`]'s [`Pipeline`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RoomElement<T> {
    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member {
        #[serde(bound = "T: From<Unvalidated> + Default")]
        spec: Pipeline<EndpointId, MemberElement<T>>,
        credentials: String,
        on_leave: Option<CallbackUrl>,
        on_join: Option<CallbackUrl>,
    },
}

impl RoomElement<Unvalidated> {
    pub fn validate(self) -> Result<RoomElement<Validated>, ValidationError> {
        match self {
            RoomElement::Member {
                spec,
                credentials,
                on_leave,
                on_join,
            } => {
                let validated_spec = spec
                    .into_iter()
                    .map(|(key, value)| {
                        value.validate().map(move |res| (key, res))
                    })
                    .collect::<Result<HashMap<_, _>, _>>()?;

                Ok(RoomElement::Member {
                    credentials,
                    on_leave,
                    on_join,
                    spec: Pipeline::new(validated_spec),
                })
            }
        }
    }
}

/// [Control API]'s `Room` element specification.
///
/// Newtype for [`RootElement::Room`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Pipeline<MemberId, RoomElement<Validated>>,
}

impl TryFrom<proto::create_request::El> for RoomSpec {
    type Error = TryFromProtobufError;

    fn try_from(proto: proto::create_request::El) -> Result<Self, Self::Error> {
        use proto::create_request::El;

        let id = match proto {
            El::Room(room) => {
                let mut pipeline = HashMap::new();
                for (id, room_element) in room.pipeline {
                    if let Some(elem) = room_element.el {
                        let member =
                            MemberSpec::try_from((MemberId(id.clone()), elem))?;
                        pipeline.insert(id.into(), member.into());
                    } else {
                        return Err(TryFromProtobufError::EmptyElement(id));
                    }
                }

                let pipeline = Pipeline::new(pipeline);
                return Ok(Self {
                    id: room.id.into(),
                    pipeline,
                });
            }
            El::Member(member) => member.id,
            El::WebrtcPub(webrtc_pub) => webrtc_pub.id,
            El::WebrtcPlay(webrtc_play) => webrtc_play.id,
        };

        Err(TryFromProtobufError::ExpectedOtherElement(
            String::from("Room"),
            id,
        ))
    }
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
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl TryFrom<&RootElement<Validated>> for RoomSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RootElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RootElement<Validated>) -> Result<Self, Self::Error> {
        match from {
            RootElement::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: spec.clone(),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}
