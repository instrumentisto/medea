use crate::api::control::{
    grpc::protos::control::Error as ErrorProto,
    local_uri::{LocalUri, LocalUriParseError},
};

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
    EndpointNotFound(LocalUri),

    //////////////////////////////////////
    // Spec errors (1200 - 1299 codes) //
    ////////////////////////////////////
    /// Medea expects `Room` element in pipeline but received not him.
    ///
    /// Code: __1200__.
    NotRoomInSpec(LocalUri),
    /// Medea expects `Member` element in pipeline but received not him.
    ///
    /// Code: __1201__.
    NotMemberInSpec(LocalUri),
    /// Medea expects `Endpoint` element in pipeline but received not him.
    ///
    /// Code: __1202__.
    NotEndpointInSpec(LocalUri),
    /// Invalid source URI in play endpoint.
    ///
    /// Code: __1203__.
    InvalidSrcUri(LocalUri),
    // TODO: simplify names
    ElementIdForRoomButElementIsNot(String),
    ElementIdForMemberButElementIsNot(String),
    ElementIdForEndpointButElementIsNot(String),
    InvalidElementUri(String),

    /////////////////////////////////
    // Parse errors (1300 - 1399) //
    ///////////////////////////////
    ElementIdIsNotLocal(String),
    ElementIdIsTooLong(String),
    MissingFieldsInSrcUri(String, Vec<String>),

    /////////////////////////////
    // Conflict (1400 - 1499) //
    ///////////////////////////
    MemberAlreadyExists(LocalUri),
    EndpointAlreadyExists(LocalUri),
    RoomAlreadyExists(LocalUri),
}

impl Into<ErrorProto> for ErrorCode {
    fn into(self) -> ErrorProto {
        let mut error = ErrorProto::new();
        error.set_status(0);
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
                error.set_code(1001)
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
                error.set_code(1405);
            }

            //////////////////////////////////////
            // Spec errors (1200 - 1299 codes) //
            ////////////////////////////////////
            ErrorCode::NotRoomInSpec(id) => {
                error.set_text(
                    "Expecting Room element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1200);
            }
            ErrorCode::NotMemberInSpec(id) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1201);
            }
            ErrorCode::NotEndpointInSpec(id) => {
                error.set_text(
                    "Expecting Member element but it's not.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1202);
            }
            ErrorCode::InvalidSrcUri(id) => {
                error.set_text(
                    "Invalid source ID in publish endpoint spec.".to_string(),
                );
                error.set_element(id.to_string());
                error.set_code(1203);
            }
            ErrorCode::ElementIdForRoomButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Room but element's spec is not for \
                     Room."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1204);
            }
            ErrorCode::ElementIdForMemberButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Member but element's spec is not for \
                     Member."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1205);
            }
            ErrorCode::ElementIdForEndpointButElementIsNot(id) => {
                error.set_text(
                    "You provided ID for Endpoint but element's spec is not \
                     for Endpoint."
                        .to_string(),
                );
                error.set_element(id);
                error.set_code(1206);
            }
            ErrorCode::InvalidElementUri(id) => {
                error.set_text("Invalid element's URI".to_string());
                error.set_element(id);
                error.set_code(1207);
            }

            /////////////////////////////////
            // Parse errors (1300 - 1399) //
            ///////////////////////////////
            ErrorCode::ElementIdIsNotLocal(uri) => {
                error.set_text(
                    "Element's ID's URI has not have 'local://' protocol."
                        .to_string(),
                );
                error.set_element(uri);
                error.set_code(1300);
            }
            ErrorCode::ElementIdIsTooLong(uri) => {
                error.set_text(
                    "In provided element's ID too many slashes.".to_string(),
                );
                error.set_element(uri);
                error.set_code(1301);
            }
            ErrorCode::MissingFieldsInSrcUri(uri, fields) => {
                error.set_text(format!(
                    "Missing {:?} fields in element ID.",
                    fields
                ));
                error.set_element(uri);
                error.set_code(1302);
            }

            /////////////////////////////
            // Conflict (1400 - 1499) //
            ///////////////////////////
            ErrorCode::MemberAlreadyExists(id) => {
                error.set_text("Member already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1400);
            }
            ErrorCode::EndpointAlreadyExists(id) => {
                error.set_text("Endpoint already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1401);
            }
            ErrorCode::RoomAlreadyExists(id) => {
                error.set_text("Room already exists.".to_string());
                error.set_element(id.to_string());
                error.set_code(1402);
            }
        }

        error
    }
}
