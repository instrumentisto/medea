//! All errors which medea can return to control API user.
//!
//! # Error codes ranges
//! * __1000...1000__ Unknow server error
//! * __1001...1099__ Not found errors
//! * __1100...1199__ Spec errors
//! * __1200...1299__ Parse errors
//! * __1300...1399__ Conflicts

use crate::api::control::{
    grpc::protos::control::Error as ErrorProto, local_uri::LocalUri,
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
    PublishEndpointNotFound(LocalUri),
    /// Play endpoint not found.
    ///
    /// Code: __1002__.
    PlayEndpointNotFound(LocalUri),
    /// Member not found.
    ///
    /// Code: __1003__.
    MemberNotFound(LocalUri),
    /// Room not found.
    ///
    /// Code: __1004__.
    RoomNotFound(LocalUri),
    /// Endpoint not found.
    ///
    /// Code: __1005__.
    EndpointNotFound(LocalUri),

    //////////////////////////////////////
    // Spec errors (1100 - 1199 codes) //
    ////////////////////////////////////
    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1100__.
    NotRoomInSpec(LocalUri),
    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1101__.
    NotMemberInSpec(LocalUri),
    /// Medea expects `Endpoint` element in pipeline but received not him.
    ///
    /// Code: __1102__.
    NotEndpointInSpec(LocalUri),
    /// Invalid source URI in play endpoint.
    ///
    /// Code: __1103__.
    InvalidSrcUri(LocalUri),
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
    /// Source URI in publish endpoint missing some fields.
    ///
    /// Code: __1202__.
    MissingFieldsInSrcUri(String, Vec<String>),
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
    MemberAlreadyExists(LocalUri),
    /// Endpoint already exists.
    ///
    /// Code: __1301__.
    EndpointAlreadyExists(LocalUri),
    /// Room already exists.
    ///
    /// Code: __1302__.
    RoomAlreadyExists(LocalUri),
}

impl Into<ErrorProto> for ErrorCode {
    fn into(self) -> ErrorProto {
        // TODO: configure backtrace
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
            ErrorCode::MissingFieldsInSrcUri(uri, fields) => {
                error.set_text(format!(
                    "Missing {:?} fields in element ID.",
                    fields
                ));
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
