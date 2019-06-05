//! Member definitions and implementations.

use crate::media::IceUser;

/// ID of [`Member`].
pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Clone, Debug)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,

    /// Turn server credentials.
    pub ice_user: Option<IceUser>,
}

impl Member {
    /// Returns new instance of [`Memebr`] with given credentials.
    pub fn new(id: Id, credentials: String) -> Self {
        Self {
            id,
            credentials,
            ice_user: None,
        }
    }
}
