//! Room definitions and implementations.

use serde::Deserialize;
use std::convert::TryFrom;

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
    pub spec: Pipeline,
}

impl RoomSpec {
    /// Try to find [`MemberSpec`] by ID.
    ///
    /// Return `None` if [`MemberSpec`] not presented in [`RoomSpec`].
    /// Return `Some(TryFromElementError::NotMember)` if element with this ID
    ///         finded but its not [`MemberSpec`].
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Option<Result<MemberSpec, TryFromElementError>> {
        Some(MemberSpec::try_from(
            self.spec.pipeline.get(&id.0).cloned()?,
        ))
    }

    /// Get all receivers of all [`Member`]'s [`WebRtcPublishEndpoint`]s.
    pub fn get_receivers_for_member(
        &self,
        id: &MemberId,
    ) -> Result<Vec<MemberId>, TryFromElementError> {
        let mut receivers = Vec::new();
        for (member_id, member_element) in &self.spec.pipeline {
            let member = MemberSpec::try_from(member_element.clone())?;
            for endpoint in member.play_endpoints() {
                if &endpoint.src.member_id == id {
                    receivers.push(MemberId(member_id.clone()));
                }
            }
        }

        Ok(receivers)
    }
}

impl TryFrom<Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self { id, spec }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_properly_get_receivers_for_member() {
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
