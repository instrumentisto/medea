use crate::{
    browser::JsExecutable,
    entity::{room::Room, Builder, Entity},
};

pub struct Jason;

impl Builder for Jason {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    let jason = new window.rust.Jason();
                    return jason;
                }
            "#,
            vec![],
        )
    }
}

impl Entity<Jason> {
    pub async fn init_room(&mut self) -> Entity<Room> {
        self.spawn_entity(Room::new(&self)).await
    }
}
