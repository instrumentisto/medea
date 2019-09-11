//! All errors which medea can return to control API user.
//!
//! # Error codes ranges
//! - __1000...1000__ Unexpected server error
//! - __1001...1099__ Not found errors
//! - __1100...1199__ Spec errors
//! - __1200...1299__ Parse errors
//! - __1300...1399__ Conflicts
//! - __1400...1499__ Misc errors

use std::string::ToString;

use derive_more::Display;
use medea_grpc_proto::control::Error as ErrorProto;

use crate::{
    api::control::{
        endpoints::webrtc_play_endpoint::SrcParseError,
        grpc::server::GrpcControlApiError, local_uri::LocalUriParseError,
        TryFromElementError, TryFromProtobufError,
    },
    signalling::{
        elements::{member::MemberError, MembersLoadError},
        participants::ParticipantServiceErr,
        room::RoomError,
        room_service::RoomServiceError,
    },
};

/// Medea's control API error response.
pub struct ErrorResponse {
    /// [`ErrorCode`] which will be returned with code and message.
    error_code: ErrorCode,

    /// Element ID where some error happened. May be empty.
    element_id: Option<String>,

    /// All [`ErrorCode`]s have [`Display`] implementation. And this
    /// implementation will be used if this field is [`None`]. But
    /// sometimes we want to add some error explanation. Then we set this
    /// field to [`Some`] and this text will be added to
    /// [`Display`] implementation's text.
    ///
    /// By default this field should be [`None`].
    ///
    /// For providing error explanation use [`ErrorResponse::with_explanation`]
    /// method.
    ///
    /// [`Display`]: std::fmt::Display
    explanation: Option<String>,
}

impl ErrorResponse {
    /// New [`ErrorResponse`] with [`ErrorCode`] and element ID.
    pub fn new<T: ToString>(error_code: ErrorCode, element_id: &T) -> Self {
        Self {
            error_code,
            element_id: Some(element_id.to_string()),
            explanation: None,
        }
    }

    /// New [`ErrorResponse`] only with [`ErrorCode`].
    pub fn without_id(error_code: ErrorCode) -> Self {
        Self {
            error_code,
            element_id: None,
            explanation: None,
        }
    }

    /// [`ErrorResponse`] for all unexpected errors.
    ///
    /// Provide unexpected `Error` to this function.
    /// This error will be printed with [`Display`] implementation
    /// of provided `Error` as error explanation.
    ///
    /// [`Display`]: std::fmt::Display
    pub fn unexpected<B: ToString>(unknown_error: &B) -> Self {
        Self {
            error_code: ErrorCode::UnexpectedError,
            explanation: Some(unknown_error.to_string()),
            element_id: None,
        }
    }

    /// [`ErrorResponse`] with some additional info.
    ///
    /// With this method you can add additional text to error message of
    /// [`ErrorCode`].
    pub fn with_explanation(
        error_code: ErrorCode,
        explanation: String,
        id: Option<String>,
    ) -> Self {
        Self {
            error_code,
            explanation: Some(explanation),
            element_id: id.map(|s| s.to_string()),
        }
    }
}

impl Into<ErrorProto> for ErrorResponse {
    fn into(self) -> ErrorProto {
        let mut error = ErrorProto::new();

        if let Some(additional_text) = &self.explanation {
            error.set_text(format!(
                "{} {}",
                self.error_code.to_string(),
                additional_text
            ));
        } else {
            error.set_text(self.error_code.to_string());
        }

        if let Some(id) = self.element_id {
            error.set_element(id);
        }
        error.set_code(self.error_code as u32);

        error
    }
}

/// [Medea]'s [Control API] errors.
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: http://tiny.cc/380uaz
#[derive(Debug, Display)]
pub enum ErrorCode {
    /// Unexpected server error.
    ///
    /// Use this [`ErrorCode`] only with [`ErrorResponse::unexpected`]
    /// function. In error text with this code should be error message
    /// which explain what exactly goes wrong
    /// ([`ErrorResponse::unexpected`] do this).
    ///
    /// Code: __1000__.
    #[display(fmt = "Unexpected error happened.")]
    UnexpectedError = 1000,

    ////////////////////////////////////
    // Not found (1001 - 1099 codes) //
    //////////////////////////////////
    /// Publish endpoint not found.
    ///
    /// Code: __1001__.
    #[display(fmt = "Publish endpoint not found.")]
    PublishEndpointNotFound = 1001,
    /// Play endpoint not found.
    ///
    /// Code: __1002__.
    #[display(fmt = "Play endpoint not found.")]
    PlayEndpointNotFound = 1002,
    /// Member not found.
    ///
    /// Code: __1003__.
    #[display(fmt = "Member not found.")]
    MemberNotFound = 1003,
    /// Room not found.
    ///
    /// Code: __1004__.
    #[display(fmt = "Room not found.")]
    RoomNotFound = 1004,
    /// Endpoint not found.
    ///
    /// Code: __1005__.
    #[display(fmt = "Endpoint not found.")]
    EndpointNotFound = 1005,
    /// Room not found for provided element.
    ///
    /// Code: __1006__.
    #[display(fmt = "Room not found for provided element.")]
    RoomNotFoundForProvidedElement = 1006,

    //////////////////////////////////////
    // Spec errors (1100 - 1199 codes) //
    ////////////////////////////////////
    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1100__.
    #[display(fmt = "Expecting Room element but it's not.")]
    NotRoomInSpec = 1100,
    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1101__.
    #[display(fmt = "Expected Member element but it's not.")]
    NotMemberInSpec = 1101,
    /// Invalid source URI in play endpoint.
    ///
    /// Code: __1102__.
    #[display(fmt = "Invalid source ID in publish endpoint spec.")]
    InvalidSrcUri = 1102,
    /// Provided element ID to Room element but element spec is not for Room.
    ///
    /// Code: __1103__.
    #[display(
        fmt = "You provided URI to Room but element's spec is not for Room."
    )]
    ElementIdForRoomButElementIsNot = 1103,
    /// Provided element ID to Member element but element spec is not for
    /// Member.
    ///
    /// Code: __1104__.
    #[display(fmt = "You provided URI to Member but element's spec is not \
                     for Member.")]
    ElementIdForMemberButElementIsNot = 1104,
    /// Provided element ID to Endpoint element but element spec is not for
    /// Endpoint.
    ///
    /// Code: __1105__.
    #[display(fmt = "You provided URI to Endpoint but element's spec is not \
                     for Endpoint.")]
    ElementIdForEndpointButElementIsNot = 1105,
    /// Provided not source URI in [`WebRtcPlayEndpoint`].
    ///
    /// Code: __1106__.
    ///
    /// [`WebRtcPlayEndpoint`]:
    /// crate::signalling::elements::endpoints::webrtc::WebRtcPlayEndpoint
    #[display(fmt = "Provided not source URI.")]
    NotSourceUri = 1106,

    /////////////////////////////////
    // Parse errors (1200 - 1299) //
    ///////////////////////////////
    /// Element's ID don't have "local://" prefix.
    ///
    /// Code: __1200__.
    #[display(fmt = "Element's ID's URI not have 'local://' protocol.")]
    ElementIdIsNotLocal = 1200,
    /// Provided element's URI with too many paths.
    ///
    /// Code: __1201__.
    #[display(fmt = "You provided element's URI with too many paths.")]
    ElementIdIsTooLong = 1201,
    /// Missing some fields in source URI of WebRtcPublishEndpoint.
    ///
    /// Code: __1202__.
    #[display(
        fmt = "Missing some fields in source URI of WebRtcPublishEndpoint."
    )]
    MissingFieldsInSrcUri = 1202,
    /// Empty element ID.
    ///
    /// Code: __1203__.
    #[display(fmt = "Provided empty element URI.")]
    EmptyElementId = 1203,
    /// Provided empty elements URIs list.
    ///
    /// Code: __1204__.
    #[display(fmt = "Provided empty elements URIs list.")]
    EmptyElementsList = 1204,
    /// Provided not the same Room IDs in elements IDs. Probably you try use
    /// Delete method for elements with different Room IDs
    ///
    /// Code: __1205__.
    ///
    /// [`RoomId`]: crate::api::control::room::Id
    #[display(fmt = "Provided not the same Room IDs in elements IDs. \
                     Probably you try use Delete method for elements with \
                     different Room IDs")]
    ProvidedNotSameRoomIds = 1205,

    /////////////////////////////
    // Conflict (1300 - 1399) //
    ///////////////////////////
    /// Member with provided URI already exists.
    ///
    /// Code: __1300__.
    #[display(fmt = "Member with provided URI already exists.")]
    MemberAlreadyExists = 1300,
    /// Endpoint with provided URI already exists.
    ///
    /// Code: __1301__.
    #[display(fmt = "Endpoint with provided URI already exists.")]
    EndpointAlreadyExists = 1301,
    /// Room with provided URI already exists.
    ///
    /// Code: __1302__.
    #[display(fmt = "Room with provided URI already exists.")]
    RoomAlreadyExists = 1302,

    ////////////////////////
    // Misc (1400 - 1499)//
    //////////////////////
    /// Unimplemented API call.
    ///
    /// This code should be with additional text which explains what
    /// exactly unimplemented (you can do it with
    /// [`ErrorResponse::with_explanation`] function).
    ///
    /// Code: __1400__.
    #[display(fmt = "Unimplemented API call.")]
    UnimplementedCall = 1400,
}

impl From<ParticipantServiceErr> for ErrorResponse {
    fn from(err: ParticipantServiceErr) -> Self {
        use ParticipantServiceErr::*;

        match err {
            EndpointNotFound(id) => Self::new(ErrorCode::EndpointNotFound, &id),
            ParticipantNotFound(id) => {
                Self::new(ErrorCode::MemberNotFound, &id)
            }
            ParticipantAlreadyExists(id) => {
                Self::new(ErrorCode::MemberAlreadyExists, &id)
            }
            EndpointAlreadyExists(id) => {
                Self::new(ErrorCode::EndpointAlreadyExists, &id)
            }
            TurnServiceErr(_) | MemberError(_) => Self::unexpected(&err),
        }
    }
}

impl From<TryFromProtobufError> for ErrorResponse {
    fn from(err: TryFromProtobufError) -> Self {
        use TryFromProtobufError::*;

        match err {
            SrcUriError(e) => e.into(),
            NotMemberElementInRoomElement(id) => Self::with_explanation(
                ErrorCode::UnimplementedCall,
                "Not Member elements in Room element currently is \
                 unimplemented."
                    .to_string(),
                Some(id),
            ),
        }
    }
}

impl From<LocalUriParseError> for ErrorResponse {
    fn from(err: LocalUriParseError) -> Self {
        use LocalUriParseError::*;

        match err {
            NotLocal(text) => Self::new(ErrorCode::ElementIdIsNotLocal, &text),
            TooManyPaths(text) => {
                Self::new(ErrorCode::ElementIdIsTooLong, &text)
            }
            Empty => Self::without_id(ErrorCode::EmptyElementId),
            MissingPaths(text) => {
                Self::new(ErrorCode::MissingFieldsInSrcUri, &text)
            }
            UrlParseErr(id, _) => Self::new(ErrorCode::InvalidSrcUri, &id),
        }
    }
}

impl From<RoomError> for ErrorResponse {
    fn from(err: RoomError) -> Self {
        use RoomError::*;

        match err {
            MemberError(e) => e.into(),
            MembersLoadError(e) => e.into(),
            ParticipantServiceErr(e) => e.into(),
            WrongRoomId(_, _)
            | PeerNotFound(_)
            | NoTurnCredentials(_)
            | ConnectionNotExists(_)
            | UnableToSendEvent(_)
            | PeerError(_)
            | TryFromElementError(_)
            | BadRoomSpec(_)
            | TurnServiceError(_)
            | ClientError(_) => Self::unexpected(&err),
        }
    }
}

impl From<MembersLoadError> for ErrorResponse {
    fn from(err: MembersLoadError) -> Self {
        use MembersLoadError::*;

        match err {
            TryFromError(e, id) => match e {
                TryFromElementError::NotMember => {
                    Self::new(ErrorCode::NotMemberInSpec, &id)
                }
                TryFromElementError::NotRoom => {
                    Self::new(ErrorCode::NotRoomInSpec, &id)
                }
            },
            MemberNotFound(id) => Self::new(ErrorCode::MemberNotFound, &id),
            PublishEndpointNotFound(id) => {
                Self::new(ErrorCode::PublishEndpointNotFound, &id)
            }
            PlayEndpointNotFound(id) => {
                Self::new(ErrorCode::PlayEndpointNotFound, &id)
            }
        }
    }
}

impl From<MemberError> for ErrorResponse {
    fn from(err: MemberError) -> Self {
        use MemberError::*;

        match err {
            PlayEndpointNotFound(id) => {
                Self::new(ErrorCode::PlayEndpointNotFound, &id)
            }
            PublishEndpointNotFound(id) => {
                Self::new(ErrorCode::PublishEndpointNotFound, &id)
            }
            EndpointNotFound(id) => Self::new(ErrorCode::EndpointNotFound, &id),
        }
    }
}

impl From<SrcParseError> for ErrorResponse {
    fn from(err: SrcParseError) -> Self {
        use SrcParseError::*;

        match err {
            NotSrcUri(text) => Self::new(ErrorCode::NotSourceUri, &text),
            LocalUriParseError(_, err) => err.into(),
        }
    }
}

impl From<RoomServiceError> for ErrorResponse {
    fn from(err: RoomServiceError) -> Self {
        use RoomServiceError::*;

        match err {
            RoomNotFound(id) => Self::new(ErrorCode::RoomNotFound, &id),
            RoomAlreadyExists(id) => {
                Self::new(ErrorCode::RoomAlreadyExists, &id)
            }
            RoomError(e) => e.into(),
            EmptyUrisList => Self::without_id(ErrorCode::EmptyElementsList),
            RoomNotFoundForElement(id) => {
                Self::new(ErrorCode::RoomNotFoundForProvidedElement, &id)
            }
            NotSameRoomIds(ids, expected_room_id) => Self::with_explanation(
                ErrorCode::ProvidedNotSameRoomIds,
                format!(
                    "Expected Room ID: '{}'. IDs with different Room ID: {:?}",
                    expected_room_id,
                    ids.into_iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                ),
                None,
            ),
            RoomMailboxErr(_) | FailedToLoadStaticSpecs(_) => {
                Self::unexpected(&err)
            }
        }
    }
}

impl From<GrpcControlApiError> for ErrorResponse {
    fn from(err: GrpcControlApiError) -> Self {
        use GrpcControlApiError::*;

        match err {
            LocalUri(e) => e.into(),
            TryFromProtobuf(e) => e.into(),
            RoomServiceError(e) => e.into(),
            RoomServiceMailboxError(_)
            | TryFromElement(_)
            | UnknownMailboxErr(_) => Self::unexpected(&err),
        }
    }
}
