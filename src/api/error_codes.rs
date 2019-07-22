//! All errors which medea can return to control API user.
//!
//! # Error codes ranges
//! * __1000...1000__ Unknow server error
//! * __1001...1099__ Not found errors
//! * __1100...1199__ Spec errors
//! * __1200...1299__ Parse errors
//! * __1300...1399__ Conflicts

use medea_grpc_proto::control::Error as ErrorProto;

use crate::{
    api::control::{
        endpoints::webrtc_play_endpoint::SrcParseError,
        local_uri::{
            IsEndpointId, IsMemberId, IsRoomId, LocalUri, LocalUriParseError,
            LocalUriType,
        },
        TryFromElementError, TryFromProtobufError,
    },
    signalling::{
        elements::{member::MemberError, MembersLoadError},
        participants::ParticipantServiceErr,
        room::RoomError,
        room_service::RoomServiceError,
    },
};

/// Medea control API errors.
// TODO: write macro for generating error codes.
pub enum ErrorCode {
    /// Unknown server error.
    ///
    /// Code: __1000__.
    UnknownError(String),

    ////////////////////////////////////
    // Not found (1001 - 1099 codes) //
    //////////////////////////////////
    /// Publish endpoint not found.
    ///
    /// Code: __1001__.
    PublishEndpointNotFound(LocalUri<IsEndpointId>),
    /// Play endpoint not found.
    ///
    /// Code: __1002__.
    PlayEndpointNotFound(LocalUri<IsEndpointId>),
    /// Member not found.
    ///
    /// Code: __1003__.
    MemberNotFound(LocalUri<IsMemberId>),
    /// Room not found.
    ///
    /// Code: __1004__.
    RoomNotFound(LocalUri<IsRoomId>),
    /// Endpoint not found.
    ///
    /// Code: __1005__.
    EndpointNotFound(LocalUri<IsEndpointId>),

    //////////////////////////////////////
    // Spec errors (1100 - 1199 codes) //
    ////////////////////////////////////
    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1100__.
    NotRoomInSpec(LocalUriType),
    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1101__.
    NotMemberInSpec(LocalUriType),
    /// Medea expects `Endpoint` element in pipeline but received not him.
    ///
    /// Code: __1102__.
    NotEndpointInSpec(LocalUriType),
    /// Invalid source URI in play endpoint.
    ///
    /// Code: __1103__.
    InvalidSrcUri(LocalUri<IsEndpointId>),
    /// Provided element ID to Room element but element spec is not for Room.
    ///
    /// Code: __1104__.
    ElementIdForRoomButElementIsNot(String),
    /// Provided element ID to Member element but element spec is not for
    /// Member.
    ///
    /// Code: __1105__.
    ElementIdForMemberButElementIsNot(String),
    /// Provided element ID to Endpoint element but element spec is not for
    /// Endpoint.
    ///
    /// Code: __1106__.
    ElementIdForEndpointButElementIsNot(String),
    /// Invalid ID for element.
    ///
    /// Code: __1107__
    InvalidElementUri(String),
    /// Provided not source URI in [`WebRtcPlayEndpoint`].
    ///
    /// Code: __1108__.
    NotSourceUri(String),

    /////////////////////////////////
    // Parse errors (1200 - 1299) //
    ///////////////////////////////
    /// Element's ID don't have "local://" prefix.
    ///
    /// Code: __1200__.
    ElementIdIsNotLocal(String),
    /// Element's ID have too many paths (slashes).
    ///
    /// Code: __1201__.
    ElementIdIsTooLong(String),
    /// Missing some fields in element's ID.
    ///
    /// Code: __1202__.
    MissingFieldsInSrcUri(String),
    /// Empty element ID.
    ///
    /// Code: __1203__.
    EmptyElementId,

    /////////////////////////////
    // Conflict (1300 - 1399) //
    ///////////////////////////
    /// Member already exists.
    ///
    /// Code: __1300__.
    MemberAlreadyExists(LocalUri<IsMemberId>),
    /// Endpoint already exists.
    ///
    /// Code: __1301__.
    EndpointAlreadyExists(LocalUri<IsEndpointId>),
    /// Room already exists.
    ///
    /// Code: __1302__.
    RoomAlreadyExists(LocalUri<IsRoomId>),
}

impl Into<ErrorProto> for ErrorCode {
    fn into(self) -> ErrorProto {
        let mut error = ErrorProto::new();
        match self {
            ErrorCode::UnknownError(msg) => {
                error.set_text(format!(
                    "Unexpected error happened. Here is it '{}'.",
                    msg
                ));
                error.set_element(String::new());
                error.set_code(1000);
            }

            ////////////////////////////////////
            // Not found (1001 - 1099 codes) //
            //////////////////////////////////
            ErrorCode::PublishEndpointNotFound(id) => {
                error.set_text("Publish endpoint not found".to_string());
                error.set_element(id.to_string());
                error.set_code(1001);
            }
            ErrorCode::PlayEndpointNotFound(id) => {
                error.set_text("Play endpoint not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1002);
            }
            ErrorCode::MemberNotFound(id) => {
                error.set_text("Member not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1003);
            }
            ErrorCode::RoomNotFound(id) => {
                error.set_text("Room not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1004);
            }
            ErrorCode::EndpointNotFound(id) => {
                error.set_text("Endpoint not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1005);
            }

            //////////////////////////////////////
            // Spec errors (1100 - 1199 codes) //
            ////////////////////////////////////
            ErrorCode::NotRoomInSpec(id) => {
                error.set_text(
                    "Expecting Room element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1100);
            }
            ErrorCode::NotMemberInSpec(id) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1101);
            }
            ErrorCode::NotEndpointInSpec(id) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1102);
            }
            ErrorCode::InvalidSrcUri(id) => {
                error.set_text(
                    "Invalid source ID in publish endpoint spec.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1103);
            }
            ErrorCode::ElementIdForRoomButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Room but element's spec is not for \
                     Room."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1104);
            }
            ErrorCode::ElementIdForMemberButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Member but element's spec is not for \
                     Member."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1105);
            }
            ErrorCode::ElementIdForEndpointButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Endpoint but element's spec is not \
                     for Endpoint."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1106);
            }
            ErrorCode::InvalidElementUri(id) => {
                error.set_text("Invalid element's URI".to_string());
                error.set_element(id);
                error.set_code(1107);
            }
            ErrorCode::NotSourceUri(id) => {
                error.set_text("Provided not source URI".to_string());
                error.set_element(id);
                error.set_code(1108);
            }

            /////////////////////////////////
            // Parse errors (1200 - 1299) //
            ///////////////////////////////
            ErrorCode::ElementIdIsNotLocal(uri) => {
                error.set_text(
                    "Element's ID's URI has not have 'local://' protocol."
                        .to_string(),
                );
                error.set_element(uri);
                error.set_code(1200);
            }
            ErrorCode::ElementIdIsTooLong(uri) => {
                error.set_text(
                    "In provided element's ID too many slashes.".to_string(),
                );
                error.set_element(uri);
                error.set_code(1201);
            }
            ErrorCode::MissingFieldsInSrcUri(uri) => {
                error
                    .set_text("Missing some fields in element ID.".to_string());
                error.set_element(uri);
                error.set_code(1202);
            }
            ErrorCode::EmptyElementId => {
                error.set_text("Provided empty element ID.".to_string());
                error.set_element(String::new());
                error.set_code(1203);
            }

            /////////////////////////////
            // Conflict (1300 - 1399) //
            ///////////////////////////
            ErrorCode::MemberAlreadyExists(id) => {
                error.set_text("Member already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1300);
            }
            ErrorCode::EndpointAlreadyExists(id) => {
                error.set_text("Endpoint already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1301);
            }
            ErrorCode::RoomAlreadyExists(id) => {
                error.set_text("Room already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1302);
            }
        }

        error
    }
}

impl From<ParticipantServiceErr> for ErrorCode {
    fn from(err: ParticipantServiceErr) -> Self {
        match err {
            ParticipantServiceErr::EndpointNotFound(id) => {
                ErrorCode::EndpointNotFound(id)
            }
            ParticipantServiceErr::ParticipantNotFound(id) => {
                ErrorCode::MemberNotFound(id)
            }
            ParticipantServiceErr::ParticipantAlreadyExists(id) => {
                ErrorCode::MemberAlreadyExists(id)
            }
            ParticipantServiceErr::EndpointAlreadyExists(id) => {
                ErrorCode::EndpointAlreadyExists(id)
            }
            _ => ErrorCode::UnknownError(err.to_string()),
        }
    }
}

impl From<TryFromProtobufError> for ErrorCode {
    fn from(err: TryFromProtobufError) -> Self {
        match err {
            TryFromProtobufError::SrcUriError(e) => e.into(),
            _ => ErrorCode::UnknownError(err.to_string()),
        }
    }
}

impl From<LocalUriParseError> for ErrorCode {
    fn from(err: LocalUriParseError) -> Self {
        match err {
            LocalUriParseError::NotLocal(text) => {
                ErrorCode::ElementIdIsNotLocal(text)
            }
            LocalUriParseError::TooManyFields(_, text) => {
                ErrorCode::ElementIdIsTooLong(text)
            }
            LocalUriParseError::Empty => ErrorCode::EmptyElementId,
            LocalUriParseError::MissingFields(text) => {
                ErrorCode::MissingFieldsInSrcUri(text)
            }
        }
    }
}

impl From<RoomError> for ErrorCode {
    fn from(err: RoomError) -> Self {
        match err {
            RoomError::MemberError(e) => e.into(),
            RoomError::MembersLoadError(e) => e.into(),
            RoomError::ParticipantServiceErr(e) => e.into(),
            _ => ErrorCode::UnknownError(err.to_string()),
        }
    }
}

impl From<MembersLoadError> for ErrorCode {
    fn from(err: MembersLoadError) -> Self {
        match err {
            MembersLoadError::TryFromError(e, id) => match e {
                TryFromElementError::NotEndpoint => {
                    ErrorCode::NotEndpointInSpec(id)
                }
                TryFromElementError::NotMember => {
                    ErrorCode::NotMemberInSpec(id)
                }
                TryFromElementError::NotRoom => ErrorCode::NotRoomInSpec(id),
            },
            MembersLoadError::MemberNotFound(id) => {
                ErrorCode::MemberNotFound(id)
            }
            MembersLoadError::PublishEndpointNotFound(id) => {
                ErrorCode::PublishEndpointNotFound(id)
            }
            MembersLoadError::PlayEndpointNotFound(id) => {
                ErrorCode::PlayEndpointNotFound(id)
            }
        }
    }
}

impl From<MemberError> for ErrorCode {
    fn from(err: MemberError) -> Self {
        match err {
            MemberError::PlayEndpointNotFound(id) => {
                ErrorCode::PlayEndpointNotFound(id)
            }
            MemberError::PublishEndpointNotFound(id) => {
                ErrorCode::PublishEndpointNotFound(id)
            }
        }
    }
}

impl From<SrcParseError> for ErrorCode {
    fn from(err: SrcParseError) -> Self {
        match err {
            SrcParseError::NotSrcUri(text) => ErrorCode::NotSourceUri(text),
            SrcParseError::LocalUriParseError(_, err) => err.into(),
        }
    }
}

impl From<RoomServiceError> for ErrorCode {
    fn from(err: RoomServiceError) -> Self {
        match err {
            RoomServiceError::RoomNotFound(id) => ErrorCode::RoomNotFound(id),
            RoomServiceError::RoomAlreadyExists(id) => {
                ErrorCode::RoomAlreadyExists(id)
            }
            RoomServiceError::RoomError(e) => e.into(),
            _ => ErrorCode::UnknownError(err.to_string()),
        }
    }
}
