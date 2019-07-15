//! All errors which medea can return to control API user.
//!
//! # Error codes ranges
//! * __1000...1000__ Unknow server error
//! * __1001...1099__ Not found errors
//! * __1100...1199__ Spec errors
//! * __1200...1299__ Parse errors
//! * __1300...1399__ Conflicts

use protobuf::RepeatedField;

use crate::api::control::{
    grpc::protos::control::Error as ErrorProto, local_uri::LocalUri,
};

#[derive(Debug)]
pub struct Backtrace(pub Vec<String>);

impl Backtrace {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push<T: std::fmt::Debug>(&mut self, error: &T) {
        self.0.push(format!("{:?}", error));
    }

    pub fn merge(&mut self, mut another_backtrace: Backtrace) {
        self.0.append(&mut another_backtrace.0);
    }
}

impl Into<RepeatedField<String>> for Backtrace {
    fn into(self) -> RepeatedField<String> {
        let mut repeated_field = RepeatedField::new();
        self.0.into_iter().for_each(|e| repeated_field.push(e));

        repeated_field
    }
}

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
    PublishEndpointNotFound(LocalUri, Backtrace),
    /// Play endpoint not found.
    ///
    /// Code: __1002__.
    PlayEndpointNotFound(LocalUri, Backtrace),
    /// Member not found.
    ///
    /// Code: __1003__.
    MemberNotFound(LocalUri, Backtrace),
    /// Room not found.
    ///
    /// Code: __1004__.
    RoomNotFound(LocalUri, Backtrace),
    /// Endpoint not found.
    ///
    /// Code: __1005__.
    EndpointNotFound(LocalUri, Backtrace),

    //////////////////////////////////////
    // Spec errors (1100 - 1199 codes) //
    ////////////////////////////////////
    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1100__.
    NotRoomInSpec(LocalUri, Backtrace),
    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1101__.
    NotMemberInSpec(LocalUri, Backtrace),
    /// Medea expects `Endpoint` element in pipeline but received not him.
    ///
    /// Code: __1102__.
    NotEndpointInSpec(LocalUri, Backtrace),
    /// Invalid source URI in play endpoint.
    ///
    /// Code: __1103__.
    InvalidSrcUri(LocalUri, Backtrace),
    /// Provided element ID to Room element but element spec is not for Room.
    ///
    /// Code: __1104__.
    ElementIdForRoomButElementIsNot(String, Backtrace),
    /// Provided element ID to Member element but element spec is not for
    /// Member.
    ///
    /// Code: __1105__.
    ElementIdForMemberButElementIsNot(String, Backtrace),
    /// Provided element ID to Endpoint element but element spec is not for
    /// Endpoint.
    ///
    /// Code: __1106__.
    ElementIdForEndpointButElementIsNot(String, Backtrace),
    /// Invalid ID for element.
    ///
    /// Code: __1107__
    InvalidElementUri(String, Backtrace),

    /////////////////////////////////
    // Parse errors (1200 - 1299) //
    ///////////////////////////////
    /// Element's ID don't have "local://" prefix.
    ///
    /// Code: __1200__.
    ElementIdIsNotLocal(String, Backtrace),
    /// Element's ID have too many paths (slashes).
    ///
    /// Code: __1201__.
    ElementIdIsTooLong(String, Backtrace),
    /// Source URI in publish endpoint missing some fields.
    ///
    /// Code: __1202__.
    MissingFieldsInSrcUri(String, Vec<String>, Backtrace),
    /// Empty element ID.
    ///
    /// Code: __1203__.
    EmptyElementId(Backtrace),

    /////////////////////////////
    // Conflict (1300 - 1399) //
    ///////////////////////////
    /// Member already exists.
    ///
    /// Code: __1300__.
    MemberAlreadyExists(LocalUri, Backtrace),
    /// Endpoint already exists.
    ///
    /// Code: __1301__.
    EndpointAlreadyExists(LocalUri, Backtrace),
    /// Room already exists.
    ///
    /// Code: __1302__.
    RoomAlreadyExists(LocalUri, Backtrace),
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
            ErrorCode::PublishEndpointNotFound(id, backtrace) => {
                error.set_text("Publish endpoint not found".to_string());
                error.set_element(id.to_string());
                error.set_code(1001);
                error.set_backtrace(backtrace.into())
            }
            ErrorCode::PlayEndpointNotFound(id, backtrace) => {
                error.set_text("Play endpoint not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1002);
                error.set_backtrace(backtrace.into())
            }
            ErrorCode::MemberNotFound(id, backtrace) => {
                error.set_text("Member not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1003);
                error.set_backtrace(backtrace.into())
            }
            ErrorCode::RoomNotFound(id, backtrace) => {
                error.set_text("Room not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1004);
                error.set_backtrace(backtrace.into())
            }
            ErrorCode::EndpointNotFound(id, backtrace) => {
                error.set_text("Endpoint not found.".to_string());
                error.set_element(id.to_string());
                error.set_code(1005);
                error.set_backtrace(backtrace.into())
            }

            //////////////////////////////////////
            // Spec errors (1100 - 1199 codes) //
            ////////////////////////////////////
            ErrorCode::NotRoomInSpec(id, backtrace) => {
                error.set_text(
                    "Expecting Room element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1100);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::NotMemberInSpec(id, backtrace) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1101);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::NotEndpointInSpec(id, backtrace) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1102);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::InvalidSrcUri(id, backtrace) => {
                error.set_text(
                    "Invalid source ID in publish endpoint spec.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1103);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::ElementIdForRoomButElementIsNot(id, backtrace) => {
                error.set_text(
                    "You provided ID for Room but element's spec is not for \
                     Room."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1104);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::ElementIdForMemberButElementIsNot(id, backtrace) => {
                error.set_text(
                    "You provided ID for Member but element's spec is not for \
                     Member."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1105);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::ElementIdForEndpointButElementIsNot(id, backtrace) => {
                error.set_text(
                    "You provided ID for Endpoint but element's spec is not \
                     for Endpoint."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1106);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::InvalidElementUri(id, backtrace) => {
                error.set_text("Invalid element's URI".to_string());
                error.set_element(id);
                error.set_code(1107);
                error.set_backtrace(backtrace.into());
            }

            /////////////////////////////////
            // Parse errors (1200 - 1299) //
            ///////////////////////////////
            ErrorCode::ElementIdIsNotLocal(uri, backtrace) => {
                error.set_text(
                    "Element's ID's URI has not have 'local://' protocol."
                        .to_string(),
                );
                error.set_element(uri);
                error.set_code(1200);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::ElementIdIsTooLong(uri, backtrace) => {
                error.set_text(
                    "In provided element's ID too many slashes.".to_string(),
                );
                error.set_element(uri);
                error.set_code(1201);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::MissingFieldsInSrcUri(uri, fields, backtrace) => {
                error.set_text(format!(
                    "Missing {:?} fields in element ID.",
                    fields
                ));
                error.set_element(uri);
                error.set_code(1202);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::EmptyElementId(backtrace) => {
                error.set_text("Provided empty element ID.".to_string());
                error.set_element(String::new());
                error.set_code(1203);
                error.set_backtrace(backtrace.into());
            }

            /////////////////////////////
            // Conflict (1300 - 1399) //
            ///////////////////////////
            ErrorCode::MemberAlreadyExists(id, backtrace) => {
                error.set_text("Member already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1300);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::EndpointAlreadyExists(id, backtrace) => {
                error.set_text("Endpoint already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1301);
                error.set_backtrace(backtrace.into());
            }
            ErrorCode::RoomAlreadyExists(id, backtrace) => {
                error.set_text("Room already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1302);
                error.set_backtrace(backtrace.into());
            }
        }

        error
    }
}
