//! Implementation of all kinds of references to some resource used in Medea's
//! Control API.

#![allow(clippy::use_self)]

pub mod fid;
pub mod local_uri;
pub mod src_uri;

use super::{EndpointId, MemberId, RoomId};

#[doc(inline)]
pub use self::{
    fid::{Fid, StatefulFid},
    local_uri::{LocalUri, StatefulLocalUri},
    src_uri::SrcUri,
};

/// State of [`LocalUri`] which points to [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToRoom(RoomId);

/// State of [`LocalUri`] which points to [`Member`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToMember(RoomId, MemberId);

/// State of [`LocalUri`] which points to [`Endpoint`].
///
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToEndpoint(RoomId, MemberId, EndpointId);

/// Generates functions for transition between [`ToRoom`],
/// [`ToMember`] and [`ToEndpoint`] states of Medea references and handy getters
/// for data of this references.
///
/// Supposed that container for which you want to implement all this methods
/// is something like:
///
/// ```rust
/// pub struct SomeReference<T> {
///     state: T
/// }
/// ```
///
/// This is necessary so that you can write different implementations of
/// serializing and deserializing for references, but at the same time have some
/// standard API for working with them.
#[macro_export]
macro_rules! impls_for_stateful_refs {
    ($container:tt) => {
        impl $container<ToRoom> {
            /// Creates new [`LocalUri`] in [`ToRoom`] state.
            pub fn new(room_id: $crate::api::control::RoomId) -> Self {
                Self {
                    state: ToRoom(room_id),
                }
            }

            /// Returns reference to [`RoomId`].
            pub fn room_id(&self) -> &$crate::api::control::RoomId {
                &self.state.0
            }

            /// Returns [`RoomId`].
            pub fn take_room_id(self) -> $crate::api::control::RoomId {
                self.state.0
            }

            /// Pushes [`MemberId`] to the end of URI and returns
            /// [`LocalUri`] in [`ToMember`] state.
            pub fn push_member_id(
                self,
                member_id: $crate::api::control::MemberId,
            ) -> $container<ToMember> {
                $container::<$crate::api::control::refs::ToMember>::new(
                    self.state.0,
                    member_id,
                )
            }
        }

        impl $container<$crate::api::control::refs::ToMember> {
            /// Create new [`LocalUri`] in [`ToMember`] state.
            pub fn new(
                room_id: $crate::api::control::RoomId,
                member_id: $crate::api::control::MemberId,
            ) -> Self {
                Self {
                    state: $crate::api::control::refs::ToMember(
                        room_id, member_id,
                    ),
                }
            }

            /// Returns reference to [`RoomId`].
            pub fn room_id(&self) -> &$crate::api::control::RoomId {
                &self.state.0
            }

            /// Returns reference to [`MemberId`].
            pub fn member_id(&self) -> &$crate::api::control::MemberId {
                &self.state.1
            }

            /// Return [`MemberId`] and [`LocalUri`] in state [`ToRoom`].
            pub fn take_member_id(
                self,
            ) -> (
                $crate::api::control::MemberId,
                $container<$crate::api::control::refs::ToRoom>,
            ) {
                (
                    self.state.1,
                    $container::<$crate::api::control::refs::ToRoom>::new(
                        self.state.0,
                    ),
                )
            }

            /// Push endpoint ID to the end of URI and returns
            /// [`LocalUri`] in [`ToEndpoint`] state.
            pub fn push_endpoint_id(
                self,
                endpoint_id: $crate::api::control::EndpointId,
            ) -> $container<$crate::api::control::refs::ToEndpoint> {
                let (member_id, room_uri) = self.take_member_id();
                let room_id = room_uri.take_room_id();
                $container::<$crate::api::control::refs::ToEndpoint>::new(
                    room_id,
                    member_id,
                    endpoint_id,
                )
            }

            /// Returns [`RoomId`] and [`MemberId`].
            pub fn take_all(
                self,
            ) -> ($crate::api::control::RoomId, $crate::api::control::MemberId)
            {
                let (member_id, room_url) = self.take_member_id();

                (room_url.take_room_id(), member_id)
            }
        }

        impl $container<$crate::api::control::refs::ToEndpoint> {
            /// Creates new [`LocalUri`] in [`ToEndpoint`] state.
            pub fn new(
                room_id: $crate::api::control::RoomId,
                member_id: $crate::api::control::MemberId,
                endpoint_id: $crate::api::control::EndpointId,
            ) -> Self {
                Self {
                    state: $crate::api::control::refs::ToEndpoint(
                        room_id,
                        member_id,
                        endpoint_id,
                    ),
                }
            }

            /// Returns reference to [`RoomId`].
            pub fn room_id(&self) -> &$crate::api::control::RoomId {
                &self.state.0
            }

            /// Returns reference to [`MemberId`].
            pub fn member_id(&self) -> &$crate::api::control::MemberId {
                &self.state.1
            }

            /// Returns reference to [`EndpointId`].
            pub fn endpoint_id(&self) -> &$crate::api::control::EndpointId {
                &self.state.2
            }

            /// Returns [`Endpoint`] id and [`LocalUri`] in [`ToMember`] state.
            ///
            /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
            pub fn take_endpoint_id(
                self,
            ) -> (
                $crate::api::control::EndpointId,
                $container<$crate::api::control::refs::ToMember>,
            ) {
                (
                    self.state.2,
                    $container::<$crate::api::control::refs::ToMember>::new(
                        self.state.0,
                        self.state.1,
                    ),
                )
            }

            /// Returns [`EndpointId`], [`RoomId`] and [`MemberId`].
            pub fn take_all(
                self,
            ) -> (
                $crate::api::control::RoomId,
                $crate::api::control::MemberId,
                $crate::api::control::EndpointId,
            ) {
                let (endpoint_id, member_url) = self.take_endpoint_id();
                let (member_id, room_url) = member_url.take_member_id();

                (room_url.take_room_id(), member_id, endpoint_id)
            }
        }
    };
}
