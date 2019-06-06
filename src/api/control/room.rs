//! Room definitions and implementations.

use std::convert::TryFrom;

use hashbrown::HashMap;
use serde::Deserialize;

use super::{
    pipeline::Pipeline, Element, Member, MemberId, TryFromElementError,
};

/// ID of [`Room`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Element::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct RoomSpec {
    id: Id,
    pipeline: Pipeline,
}

impl RoomSpec {
    /// Get all receivers of all [`Member`]'s [`WebRtcPublishEndpoint`]s.
    pub fn get_receivers_for_member(
        &self,
        id: &MemberId,
    ) -> Result<Vec<MemberId>, TryFromElementError> {
        self.pipeline.get_receivers_for_member(id)
    }

    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, Member>, TryFromElementError> {
        self.pipeline.members(self)
    }

    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl TryFrom<Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self { id, pipeline: spec }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

#[cfg(test)]
mod room_spec_tests {
    use super::*;

    #[test]
    fn properly_get_receivers_for_member() {
        let spec = r#"
            kind: Room
            id: test-call
            spec:
              pipeline:
                caller:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      publish:
                        kind: WebRtcPublishEndpoint
                        spec:
                          p2p: Always
                some-member:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      publish:
                        kind: WebRtcPublishEndpoint
                        spec:
                          p2p: Always
                responder:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      play:
                        kind: WebRtcPlayEndpoint
                        spec:
                          src: "local://test-call/caller/publish"
                      play2:
                        kind: WebRtcPlayEndpoint
                        spec:
                          src: "local://test-call/some-member/publish"
        "#;
        let spec: Element = serde_yaml::from_str(spec).unwrap();
        let room = RoomSpec::try_from(spec).unwrap();

        let caller_member_id = MemberId("caller".to_string());
        let responder_member_id = MemberId("responder".to_string());
        let some_member_id = MemberId("some-member".to_string());

        let caller_receivers =
            room.get_receivers_for_member(&caller_member_id).unwrap();
        assert_eq!(caller_receivers, vec![responder_member_id.clone()]);

        let responder_receivers =
            room.get_receivers_for_member(&responder_member_id).unwrap();
        assert_eq!(responder_receivers, vec![]);

        let some_member_receivers =
            room.get_receivers_for_member(&some_member_id).unwrap();
        assert_eq!(some_member_receivers, vec![responder_member_id.clone()]);
    }
}
