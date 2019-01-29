//! Member definitions and implementations.

use failure::Fail;
use hashbrown::HashMap;

use crate::log::prelude::*;

/// Error that can be returned by Control API.
#[derive(Fail, Debug, PartialEq)]
pub enum ControlError {
    /// [`Member`] is not found in repository.
    #[fail(display = "Not found member")]
    NotFound,
}

/// ID of [`Member`].
pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Clone, Debug)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,
    /// Credentials to authorize [`Member`] with.
    pub credentials: String,
}

/// Repository that stores store [`Member`]s.
pub struct MemberRepository {
    pub members: HashMap<Id, Member>,
}

impl MemberRepository {
    /// Returns [`Member`] by its ID.
    pub fn get_member(&self, id: Id) -> Result<Member, ControlError> {
        debug!("retrieve member by id: {}", id);
        self.members
            .get(&id)
            .map(|member| member.to_owned())
            .ok_or(ControlError::NotFound)
    }

    /// Returns [`Member`] by its credentials.
    pub fn get_member_by_credentials(
        &self,
        credentials: String,
    ) -> Result<Member, ControlError> {
        debug!("retrieve member by credentials: {}", credentials);
        self.members
            .values()
            .find(|member| member.credentials.eq(&credentials))
            .map(|member| member.to_owned())
            .ok_or(ControlError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members() -> HashMap<Id, Member> {
        let members = hashmap! {
            1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
            2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
        };
        members
    }

    #[test]
    fn returns_member_by_id() {
        let members = members();
        let repo = &MemberRepository { members };

        let res = repo.get_member(1);
        assert!(res.is_ok());
        let member = res.unwrap();
        assert_eq!(member.id, 1);
    }

    #[test]
    fn returns_member_by_credentials() {
        let members = members();
        let repo = &MemberRepository { members };

        let res =
            repo.get_member_by_credentials("responder_credentials".to_owned());
        assert!(res.is_ok());
        let member = res.unwrap();
        assert_eq!(member.id, 2);
        assert_eq!(member.credentials, "responder_credentials".to_owned());
    }

    #[test]
    fn returns_error_not_found() {
        let members = members();
        let repo = &MemberRepository { members };

        let res = repo.get_member(999);
        assert!(res.is_err());
        assert_eq!(res.err(), Some(ControlError::NotFound));
    }
}
