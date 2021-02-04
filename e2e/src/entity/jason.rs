use crate::entity::{Builder, Entity};
use crate::browser::JsExecutable;

pub struct Jason;

impl Builder for Jason {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    return new window.rust.Jason();
                }
            "#,
            vec![],
        )
    }
}
