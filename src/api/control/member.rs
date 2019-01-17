use actix::prelude::*;
use im::hashmap::HashMap;

use crate::errors::AppError;

pub type Id = u64;

#[derive(Clone)]
pub struct Member {
    pub id: Id,
    pub credentials: String,
}

pub struct MemberRepository {
    pub members: HashMap<Id, Member>,
}

impl Actor for MemberRepository {
    type Context = Context<Self>;
}

/// Message for retrieves member by its id.
pub struct GetMember(pub Id);

impl Message for GetMember {
    type Result = Result<Member, AppError>;
}

impl Handler<GetMember> for MemberRepository {
    type Result = Result<Member, AppError>;

    fn handle(&mut self, msg: GetMember, _: &mut Self::Context) -> Self::Result {
        self.members
            .get(&msg.0)
            .map(|member| member.to_owned())
            .ok_or(AppError::NotFound)
    }
}

/// Message for retrieves member by its credential.
pub struct GetMemberByCredentials(pub String);

impl Message for GetMemberByCredentials {
    type Result = Result<Member, AppError>;
}

impl Handler<GetMemberByCredentials> for MemberRepository {
    type Result = Result<Member, AppError>;

    fn handle(&mut self, msg: GetMemberByCredentials, _: &mut Self::Context) -> Self::Result {
        self.members
            .values()
            .find(|member| member.credentials.eq(&msg.0))
            .map(|member| member.to_owned())
            .ok_or(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tokio::prelude::*;
    use tokio::timer::Delay;

    #[test]
    fn test_get_member_by_id() {
        System::run(move || {
            let id = 2;
            let members = HashMap::unit(
                id,
                Member {
                    id,
                    credentials: "credentials".to_owned(),
                },
            );

            let addr = Arbiter::builder().start(move |_| MemberRepository { members });

            tokio::spawn(
                addr.send(GetMember(id))
                    .and_then(|res| {
                        assert!(res.is_ok());
                        let member = res.unwrap();
                        assert_eq!(member.id, 2);
                        Ok(())
                    })
                    .then(move |_| {
                        Delay::new(Instant::now() + Duration::new(0, 1_000_000)).then(move |_| {
                            System::current().stop();
                            future::result(Ok(()))
                        })
                    }),
            );
        });
    }

    #[test]
    fn test_get_member_by_credentials() {
        System::run(move || {
            let id = 2;
            let cred = "credentials";
            let members = HashMap::unit(
                id,
                Member {
                    id,
                    credentials: cred.to_owned(),
                },
            );

            //let addr = SyncArbiter::start(1, || MemberRepository{members});
            let addr = Arbiter::builder().start(move |_| MemberRepository { members });

            tokio::spawn(
                addr.send(GetMemberByCredentials(cred.to_owned()))
                    .and_then(|res| {
                        assert!(res.is_ok());
                        let member = res.unwrap();
                        assert_eq!(member.id, 2);
                        assert_eq!(member.credentials, "credentials");
                        Ok(())
                    })
                    .then(move |_| {
                        Delay::new(Instant::now() + Duration::new(0, 1_000_000)).then(move |_| {
                            System::current().stop();
                            future::result(Ok(()))
                        })
                    }),
            );
        });
    }
}
