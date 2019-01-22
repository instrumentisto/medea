use actix::prelude::*;
use failure::Fail;
use hashbrown::HashMap;

use crate::log::prelude::*;

#[derive(Fail, Debug, PartialEq)]
pub enum ControlError {
    #[fail(display = "Not found member")]
    NotFound,
}

pub type Id = u64;

#[derive(Clone, Debug)]
pub struct Member {
    pub id: Id,
    pub credentials: String,
}

#[derive(Debug)]
pub struct MemberRepository {
    pub members: HashMap<Id, Member>,
}

impl Actor for MemberRepository {
    type Context = Context<Self>;
}

/// Message for retrieves member by its id.
pub struct GetMember(pub Id);

impl Message for GetMember {
    type Result = Result<Member, ControlError>;
}

impl Handler<GetMember> for MemberRepository {
    type Result = Result<Member, ControlError>;

    fn handle(
        &mut self,
        msg: GetMember,
        _: &mut Self::Context,
    ) -> Self::Result {
        debug!("GetMember message received");
        self.members
            .get(&msg.0)
            .map(|member| member.to_owned())
            .ok_or(ControlError::NotFound)
    }
}

/// Message for retrieves member by its credential.
pub struct GetMemberByCredentials(pub String);

impl Message for GetMemberByCredentials {
    type Result = Result<Member, ControlError>;
}

impl Handler<GetMemberByCredentials> for MemberRepository {
    type Result = Result<Member, ControlError>;

    fn handle(
        &mut self,
        msg: GetMemberByCredentials,
        _: &mut Self::Context,
    ) -> Self::Result {
        debug!("GetMemberByCredentials message received");
        self.members
            .values()
            .find(|member| member.credentials.eq(&msg.0))
            .map(|member| member.to_owned())
            .ok_or(ControlError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tokio::prelude::*;
    use tokio::timer::Delay;

    fn members() -> HashMap<Id, Member> {
        let members = hashmap! {
            1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
            2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
        };
        members
    }

    #[test]
    fn returns_member_by_id() {
        System::run(move || {
            let members = members();
            let addr = Arbiter::start(move |_| MemberRepository { members });

            tokio::spawn(
                addr.send(GetMember(1))
                    .and_then(|res| {
                        assert!(res.is_ok());
                        let member = res.unwrap();
                        assert_eq!(member.id, 1);
                        Ok(())
                    })
                    .then(move |_| {
                        Delay::new(Instant::now() + Duration::new(0, 1_000_000))
                            .then(move |_| {
                                System::current().stop();
                                future::result(Ok(()))
                            })
                    }),
            );
        });
    }

    #[test]
    fn returns_member_by_credentials() {
        System::run(move || {
            let members = members();
            let addr = Arbiter::start(move |_| MemberRepository { members });

            tokio::spawn(
                addr.send(GetMemberByCredentials(
                    "responder_credentials".to_owned(),
                ))
                .and_then(|res| {
                    assert!(res.is_ok());
                    let member = res.unwrap();
                    assert_eq!(member.id, 2);
                    assert_eq!(
                        member.credentials,
                        "responder_credentials".to_owned()
                    );
                    Ok(())
                })
                .then(move |_| {
                    Delay::new(Instant::now() + Duration::new(0, 1_000_000))
                        .then(move |_| {
                            System::current().stop();
                            future::result(Ok(()))
                        })
                }),
            );
        });
    }

    #[test]
    fn returns_error_not_found() {
        System::run(move || {
            let members = members();
            let addr = Arbiter::start(move |_| MemberRepository { members });

            tokio::spawn(
                addr.send(GetMember(999))
                    .and_then(|res| {
                        assert!(res.is_err());
                        // let member = res.unwrap();
                        assert_eq!(res.err(), Some(ControlError::NotFound));
                        Ok(())
                    })
                    .then(move |_| {
                        Delay::new(Instant::now() + Duration::new(0, 1_000_000))
                            .then(move |_| {
                                System::current().stop();
                                future::result(Ok(()))
                            })
                    }),
            );
        });
    }
}
