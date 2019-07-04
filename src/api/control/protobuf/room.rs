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

    fn id(&self) -> &RoomId {
        unimplemented!()
    }

    fn get_member_by_id(&self, _id: &MemberId) -> Option<Box<&dyn MemberSpec>> {
        unimplemented!()
    }
}
