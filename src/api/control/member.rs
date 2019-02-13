//! Member definitions and implementations.
use std::sync::{Arc, Mutex};

use hashbrown::HashMap;

use crate::log::prelude::*;

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

/// Repository that stores [`Member`]s.
#[derive(Clone, Default)]
pub struct MemberRepository {
    members: Arc<Mutex<HashMap<Id, Member>>>,
}

impl MemberRepository {
    /// Creates new [`Member`]s repository with passed-in [`Member`]s.
    pub fn new(members: HashMap<Id, Member>) -> Self {
        MemberRepository {
            members: Arc::new(Mutex::new(members)),
        }
    }

    /// Returns [`Member`] by its ID.
    #[allow(dead_code)]
    pub fn get(&self, id: Id) -> Option<Member> {
        debug!("retrieve member by id: {}", id);
        let members = self.members.lock().unwrap();
        members.get(&id).map(|m| m.clone())
    }

    /// Returns [`Member`] by its credentials.
    pub fn get_by_credentials(&self, credentials: &str) -> Option<Member> {
        debug!("retrieve member by credentials: {}", credentials);
        let members = self.members.lock().unwrap();
        members
            .values()
            .find(|m| m.credentials.eq(credentials))
            .map(|m| m.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_members() -> HashMap<Id, Member> {
        hashmap! {
            1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
            2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
        }
    }

    #[test]
    fn returns_member_by_id() {
        let repo = MemberRepository::new(test_members());

        let res = repo.get(1);
        assert!(res.is_some());
        let member = res.unwrap();
        assert_eq!(member.id, 1);
    }

    #[test]
    fn returns_member_by_credentials() {
        let repo = MemberRepository::new(test_members());

        let res = repo.get_by_credentials("responder_credentials");
        assert!(res.is_some());
        let member = res.unwrap();
        assert_eq!(member.id, 2);
        assert_eq!(member.credentials, "responder_credentials");
    }

    #[test]
    fn returns_error_not_found() {
        let repo = MemberRepository::new(test_members());

        let res = repo.get(999);
        assert!(res.is_none());
    }
}
