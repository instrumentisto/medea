//! All errors which Medea can return to Control API user.
//!
//! # Error codes ranges
//! - `1000` ... `1999` Client errors
//! - `2000` ... `2999` Server errors

use std::string::ToString;

use derive_more::Display;
use medea_control_api_proto::grpc::api::Error as ErrorProto;

use crate::{
    api::control::{
        endpoints::webrtc_play_endpoint::SrcParseError,
        grpc::server::GrpcControlApiError,
        refs::{fid::ParseFidError, local_uri::LocalUriParseError},
        TryFromElementError, TryFromProtobufError,
    },
    signalling::{
        elements::{member::MemberError, MembersLoadError},
        participants::ParticipantServiceErr,
        room::RoomError,
        room_service::RoomServiceError,
    },
};

/// Medea's Control API error response.
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
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Display)]
pub enum ErrorCode {
    /// Unimplemented API call.
    ///
    /// This code should be with additional text which explains what
    /// exactly unimplemented (you can do it with
    /// [`ErrorResponse::with_explanation`] function).
    ///
    /// Code: __1000__.
    #[display(fmt = "Unimplemented API call.")]
    UnimplementedCall = 1000,

    /// Request doesn't contain any elements.
    ///
    /// Code: __1001__.
    #[display(fmt = "Request doesn't contain any elements")]
    NoElement = 1001,

    /// Provided uri can't point to provided element.
    ///
    /// Code: __1002__.
    #[display(fmt = "Provided uri can't point to provided element")]
    ElementIdMismatch = 1002,

    /// Room not found.
    ///
    /// Code: __1003__.
    #[display(fmt = "Room not found.")]
    RoomNotFound = 1003,

    /// Member not found.
    ///
    /// Code: __1004__.
    #[display(fmt = "Member not found.")]
    MemberNotFound = 1004,

    /// Endpoint not found.
    ///
    /// Code: __1005__.
    #[display(fmt = "Endpoint not found.")]
    EndpointNotFound = 1005,

    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1006__.
    #[display(fmt = "Expecting Room element but it's not.")]
    NotRoomInSpec = 1006,

    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1007__.
    #[display(fmt = "Expected Member element but it's not.")]
    NotMemberInSpec = 1007,

    /// Invalid source URI in [`WebRtcPlayEndpoint`].
    ///
    /// Code: __1008__.
    #[display(fmt = "Invalid source URI in 'WebRtcPlayEndpoint'.")]
    InvalidSrcUri = 1008,

    /// Provided not source URI in [`WebRtcPlayEndpoint`].
    ///
    /// Code: __1009__.
    ///
    /// [`WebRtcPlayEndpoint`]:
    /// crate::signalling::elements::endpoints::webrtc::WebRtcPlayEndpoint
    #[display(fmt = "Provided not source URI in 'WebRtcPlayEndpoint'.")]
    NotSourceUri = 1009,

    /// Element's URI don't have `local://` prefix.
    ///
    /// Code: __1010__.
    #[display(fmt = "Element's URI don't have 'local://' prefix.")]
    ElementIdIsNotLocal = 1010,

    /// Provided element's URI with too many paths.
    ///
    /// Code: __1011__.
    #[display(fmt = "You provided element's URI with too many paths.")]
    ElementIdIsTooLong = 1011,

    /// Missing some fields in source URI of WebRtcPublishEndpoint.
    ///
    /// Code: __1012__.
    #[display(
        fmt = "Missing some fields in source URI of WebRtcPublishEndpoint."
    )]
    MissingFieldsInSrcUri = 1012,

    /// Empty element ID.
    ///
    /// Code: __1013__.
    #[display(fmt = "Provided empty element URI.")]
    EmptyElementId = 1013,

    /// Provided empty elements URIs list.
    ///
    /// Code: __1014__.
    #[display(fmt = "Provided empty elements URIs list.")]
    EmptyElementsList = 1014,

    /// Provided not the same Room IDs in elements IDs. Probably you try use
    /// `Delete` method for elements with different Room IDs
    ///
    /// Code: __1015__.
    ///
    /// [`RoomId`]: crate::api::control::room::Id
    #[display(fmt = "Provided not the same Room IDs in elements IDs. \
                     Probably you try use 'Delete' method for elements with \
                     different Room IDs")]
    ProvidedNotSameRoomIds = 1015,

    /// Room with provided URI already exists.
    ///
    /// Code: __1016__.
    #[display(fmt = "Room with provided URI already exists.")]
    RoomAlreadyExists = 1016,

    /// Member with provided URI already exists.
    ///
    /// Code: __1017__.
    #[display(fmt = "Member with provided URI already exists.")]
    MemberAlreadyExists = 1017,

    /// Endpoint with provided URI already exists.
    ///
    /// Code: __1018__.
    #[display(fmt = "Endpoint with provided URI already exists.")]
    EndpointAlreadyExists = 1018,

    /// Unexpected server error.
    ///
    /// Use this [`ErrorCode`] only with [`ErrorResponse::unexpected`]
    /// function. In error text with this code should be error message
    /// which explain what exactly goes wrong
    /// ([`ErrorResponse::unexpected`] do this).
    ///
    /// Code: __2000__.
    #[display(fmt = "Unexpected error happened.")]
    UnexpectedError = 2000,
}

impl From<ParticipantServiceErr> for ErrorResponse {
    fn from(err: ParticipantServiceErr) -> Self {
        use ParticipantServiceErr::*;

        match err {
            EndpointNotFound(id) => Self::new(ErrorCode::EndpointNotFound, &id),
            ParticipantNotFound(id) => {
                Self::new(ErrorCode::MemberNotFound, &id)
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
                String::from(
                    "Not Member elements in Room element currently is \
                     unimplemented.",
                ),
                Some(id),
            ),
            UnimplementedEndpoint(id) => Self::with_explanation(
                ErrorCode::UnimplementedCall,
                String::from("Endpoint is not implemented."),
                Some(id),
            ),
            ExpectedOtherElement(element, id) => Self::with_explanation(
                ErrorCode::ElementIdMismatch,
                format!(
                    "Provided uri can not point to element of type [{}]",
                    element
                ),
                Some(id),
            ),
            EmptyElement(id) => Self::with_explanation(
                ErrorCode::NoElement,
                String::from("No element was provided"),
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

impl From<ParseFidError> for ErrorResponse {
    fn from(err: ParseFidError) -> Self {
        use ParseFidError::*;

        match err {
            TooManyPaths(text) => {
                Self::new(ErrorCode::ElementIdIsTooLong, &text)
            }
            Empty => Self::without_id(ErrorCode::EmptyElementId),
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
            MemberAlreadyExists(id) => {
                Self::new(ErrorCode::MemberAlreadyExists, &id)
            }
            EndpointAlreadyExists(id) => {
                Self::new(ErrorCode::EndpointAlreadyExists, &id)
            }
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
            EndpointNotFound(id) => Self::new(ErrorCode::EndpointNotFound, &id),
        }
    }
}

impl From<MemberError> for ErrorResponse {
    fn from(err: MemberError) -> Self {
        match err {
            MemberError::EndpointNotFound(id) => {
                Self::new(ErrorCode::EndpointNotFound, &id)
            }
        }
    }
}

impl From<SrcParseError> for ErrorResponse {
    fn from(err: SrcParseError) -> Self {
        use SrcParseError::*;

        match err {
            NotSrcUri(text) => Self::new(ErrorCode::NotSourceUri, &text),
            LocalUriParseError(err) => err.into(),
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
            NotSameRoomIds(id1, id2) => Self::with_explanation(
                ErrorCode::ProvidedNotSameRoomIds,
                format!(
                    "All URI's must have equal room_id. Provided Id's are \
                     different: [{}] != [{}]",
                    id1, id2
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
            Fid(e) => e.into(),
            TryFromProtobuf(e) => e.into(),
            RoomServiceError(e) => e.into(),
            RoomServiceMailboxError(_) | TryFromElement(_) => {
                Self::unexpected(&err)
            }
        }
    }
}
