use crate::{
    browser::JsExecutable,
    entity::{Builder, Entity},
};

pub struct CallbackSubscriber;

impl Builder for CallbackSubscriber {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    return {
                        fired: function() {
                            this.isFired = true;
                            for (sub of this.subs) {
                                sub();
                            }
                        },
                        isFired: false,
                        subs: [],
                    }
                }
            "#,
            vec![],
        )
    }
}

impl Entity<CallbackSubscriber> {
    pub async fn wait_for_call(&mut self) {
        self.execute_async(JsExecutable::new(
            r#"
                async (me) => {
                    if (!me.isFired) {
                        let waiter = new Promise((resolve, reject) => {
                            me.subs.push(resolve);
                        });
                        await waiter;
                    }
                }
            "#,
            vec![],
        ))
        .await;
    }
}
