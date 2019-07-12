//! URI for pointing to some medea element.

use std::fmt;

use failure::Fail;

use crate::api::error_codes::ErrorCode;

use super::{MemberId, RoomId};

#[derive(Debug, Fail)]
pub enum LocalUriParseError {
    /// Protocol of provided URI is not "local://".
    #[fail(display = "Provided URIs protocol is not 'local://'.")]
    NotLocal(String),

    /// Too many paths in provided URI.
    #[fail(display = "Too many ({}) paths in provided URI.", _0)]
    TooManyFields(usize, String),
}

impl Into<ErrorCode> for LocalUriParseError {
    fn into(self) -> ErrorCode {
        match self {
            LocalUriParseError::NotLocal(text) => {
                ErrorCode::ElementIdIsNotLocal(text)
            }
            LocalUriParseError::TooManyFields(_, text) => {
                ErrorCode::ElementIdIsTooLong(text)
            }
        }
    }
}

/// Uri in format "local://room_id/member_id/endpoint_id"
/// This kind of uri used for pointing to some element in spec (`Room`,
/// `Member`, `WebRtcPlayEndpoint`, `WebRtcPublishEndpoint`, etc).
#[derive(Debug)]
pub struct LocalUri {
    /// ID of [`Room`]
    pub room_id: Option<RoomId>,
    /// ID of `Member`
    pub member_id: Option<MemberId>,
    /// Control ID of [`Endpoint`]
    pub endpoint_id: Option<String>,
}

impl LocalUri {
    /// Create new [`LocalUri`] with provided IDs.
    pub fn new(
        room_id: Option<RoomId>,
        member_id: Option<MemberId>,
        endpoint_id: Option<String>,
    ) -> Self {
        Self {
            room_id,
            member_id,
            endpoint_id,
        }
    }

    /// Parse [`LocalUri`] from str.
    ///
    /// Returns [`LocalUriParse::NotLocal`] when uri is not "local://"
    /// Returns [`LocalUriParse::TooManyFields`] when uri have too many paths.
    pub fn parse(value: &str) -> Result<Self, LocalUriParseError> {
        let protocol_name: String = value.chars().take(8).collect();
        if protocol_name != "local://" {
            return Err(LocalUriParseError::NotLocal(value.to_string()));
        }

        let uri_body = value.chars().skip(8).collect::<String>();
        let mut uri_body_splitted: Vec<&str> = uri_body.rsplit('/').collect();
        let uri_body_splitted_len = uri_body_splitted.len();

        if uri_body_splitted_len > 3 {
            return Err(LocalUriParseError::TooManyFields(
                uri_body_splitted_len,
                value.to_string(),
            ));
        }

        let room_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(|p| RoomId(p.to_string()));
        let member_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(|p| MemberId(p.to_string()));
        let endpoint_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string());

        Ok(Self {
            room_id,
            member_id,
            endpoint_id,
        })
    }

    /// Return true if this [`LocalUri`] pointing to `Room` element.
    pub fn is_room_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_none()
            && self.endpoint_id.is_none()
    }

    /// Return true if this [`LocalUri`] pointing to `Member` element.
    pub fn is_member_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_none()
    }

    /// Return true if this [`LocalUri`] pointing to `Endpoint` element.
    pub fn is_endpoint_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_some()
    }
}

impl fmt::Display for LocalUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://")?;
        if let Some(room_id) = &self.room_id {
            write!(f, "{}", room_id)?;
            if let Some(member_id) = &self.member_id {
                write!(f, "/{}", member_id)?;
                if let Some(endpoint_id) = &self.endpoint_id {
                    write!(f, "/{}", endpoint_id)?
                }
            }
        }

        Ok(())
    }
}
