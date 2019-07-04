use hashbrown::HashMap;

use crate::api::{
    control::model::{member::MemberSpec, room::RoomSpec, MemberId, RoomId},
    grpc::protos::control::CreateRequest,
};

use super::member::GrpcMember;

#[allow(dead_code)]
struct CreateRequestSpec(CreateRequest);

impl RoomSpec for CreateRequestSpec {
    fn members(&self) -> HashMap<MemberId, Box<dyn MemberSpec>> {
        if self.0.has_room() {
            let member = self.0.get_room();
            member
                .get_pipeline()
                .iter()
                .filter_map(|(id, e)| {
                    if e.has_member() {
                        let member = e.get_member();
                        return Some((
                            MemberId(id.clone()),
                            Box::new(GrpcMember(member.clone()))
                                as Box<dyn MemberSpec>,
                        ));
                    }
                    None
                })
                .collect()
        } else {
            HashMap::new()
        }
    }

    fn id(&self) -> RoomId {
        if self.0.has_id() {
            RoomId(self.0.get_id().to_string())
        } else {
            panic!()
        }
    }

    fn get_member_by_id(&self, id: &MemberId) -> Option<Box<dyn MemberSpec>> {
        if self.0.has_room() {
            let room = self.0.get_room();
            let element = room.pipeline.get(&id.0)?;
            if element.has_member() {
                let member = element.get_member().clone();
                let member = GrpcMember(member);
                Some(Box::new(member) as Box<dyn MemberSpec>)
            } else {
                None
            }
        } else {
            panic!()
        }
    }
}
