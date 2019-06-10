//! Room definitions and implementations.

use std::{convert::TryFrom, sync::Arc};

use hashbrown::HashMap;
use serde::Deserialize;

use super::{
    member::MemberSpec, pipeline::Pipeline, Element, MemberId,
    TryFromElementError,
};

/// ID of [`Room`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Element::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Arc<Pipeline>,
}

impl RoomSpec {
    /// Returns all [`Member`]s of this [`RoomSpec`].
    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, MemberSpec>, TryFromElementError> {
        let mut members: HashMap<MemberId, MemberSpec> = HashMap::new();
        for (control_id, element) in self.pipeline.iter() {
            let member_spec = MemberSpec::try_from(element)?;
            let member_id = MemberId(control_id.clone());

            members.insert(member_id.clone(), member_spec);
        }

        Ok(members)
    }

    /// Returns ID of this [`RoomSpec`]
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl TryFrom<&Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: Arc::new(spec.clone()),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

#[cfg(test)]
mod room_spec_tests {
    use super::*;

    //    #[test]
    //    fn properly_get_receivers_for_member() {
    //        let spec = r#"
    //            kind: Room
    //            id: test-call
    //            spec:
    //              pipeline:
    //                caller:
    //                  kind: Member
    //                  credentials: test
    //                  spec:
    //                    pipeline:
    //                      publish:
    //                        kind: WebRtcPublishEndpoint
    //                        spec:
    //                          p2p: Always
    //                some-member:
    //                  kind: Member
    //                  credentials: test
    //                  spec:
    //                    pipeline:
    //                      publish:
    //                        kind: WebRtcPublishEndpoint
    //                        spec:
    //                          p2p: Always
    //                responder:
    //                  kind: Member
    //                  credentials: test
    //                  spec:
    //                    pipeline:
    //                      play:
    //                        kind: WebRtcPlayEndpoint
    //                        spec:
    //                          src: "local://test-call/caller/publish"
    //                      play2:
    //                        kind: WebRtcPlayEndpoint
    //                        spec:
    //                          src: "local://test-call/some-member/publish"
    //        "#;
    //        let spec: Element = serde_yaml::from_str(spec).unwrap();
    //        let room = RoomSpec::try_from(&spec).unwrap();
    //
    //        let caller_member_id = MemberId("caller".to_string());
    //        let responder_member_id = MemberId("responder".to_string());
    //
    //        let room_members = room.members().unwrap();
    //        let caller_receivers = room_members
    //            .get(&caller_member_id)
    //            .unwrap()
    //            .receivers()
    //            .unwrap();
    //        assert_eq!(caller_receivers, vec![responder_member_id.clone()]);
    //
    //        let responder_receivers = room_members
    //            .get(&responder_member_id)
    //            .unwrap()
    //            .receivers()
    //            .unwrap();
    //        assert_eq!(responder_receivers, vec![]);
    //    }
}
