//! Control API specification Pipeline definition.

use std::{collections::HashMap, convert::TryFrom as _};

use hashbrown::HashMap as HashBrownMap;
use serde::Deserialize;

use super::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    Member, MemberId, MemberSpec, RoomSpec, TryFromElementError,
};

use crate::api::control::Element;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pipeline: HashMap<String, Element>,
}

impl Pipeline {
    /// Get all [`WebRtcPlayEndpoint`]s from this [`Pipeline`].
    pub fn play_endpoints(&self) -> Vec<&WebRtcPlayEndpoint> {
        self.pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Element::WebRtcPlayEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Get all [`WebRtcPublishEndpoint`]s from this [`Pipeline`].
    pub fn publish_endpoints(&self) -> Vec<&WebRtcPublishEndpoint> {
        self.pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Element::WebRtcPublishEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Get all receivers of all [`Member`]'s [`WebRtcPublishEndpoint`]s.
    pub fn get_receivers_for_member(
        &self,
        id: &MemberId,
    ) -> Result<Vec<MemberId>, TryFromElementError> {
        let mut receivers = Vec::new();
        for (member_id, member_element) in &self.pipeline {
            let member = MemberSpec::try_from(member_element.clone())?;
            for endpoint in member.play_endpoints() {
                if &endpoint.src.member_id == id {
                    receivers.push(MemberId(member_id.clone()));
                }
            }
        }

        Ok(receivers)
    }

    /// Get all members from pipeline of [`RoomSpec`].
    pub fn members(
        &self,
        room_spec: &RoomSpec,
    ) -> Result<HashBrownMap<MemberId, Member>, TryFromElementError> {
        let mut members = HashBrownMap::new();
        for (control_id, element) in &self.pipeline {
            let member_spec = MemberSpec::try_from(element.clone())?;
            let member_id = MemberId(control_id.clone());

            members.insert(
                member_id.clone(),
                Member::new(member_id, member_spec, room_spec)?,
            );
        }

        Ok(members)
    }
}
