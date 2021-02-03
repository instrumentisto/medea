mod callback_subscriber;
mod room;

use std::marker::PhantomData;

use serde_json::Value as Json;

use crate::browser::{JsExecutable, WebClient};

pub use self::{callback_subscriber::CallbackSubscriber, room::Room};

pub struct Entity<T> {
    id: String,
    client: WebClient,
    _entity_type: PhantomData<T>,
}

impl<T> Entity<T> {
    pub fn new(uri: String, client: WebClient) -> Self {
        Self {
            id: uri,
            client,
            _entity_type: PhantomData::default(),
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    async fn execute(&mut self, js: JsExecutable) -> Json {
        self.client
            .execute(
                JsExecutable::new(
                    r#"
                    () => {
                        const [id] = args;
                        return window.holders.get(id);
                    }
                "#,
                    vec![self.id.clone().into()],
                )
                .and_then(js),
            )
            .await
    }

    async fn execute_async(&mut self, js: JsExecutable) -> Json {
        self.client
            .execute_async(self.get_obj().and_then(js))
            .await
            .unwrap()
    }

    fn get_obj(&self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    const [id] = args;
                    return window.holders.get(id);
                }
            "#,
            vec![self.id.clone().into()],
        )
    }
}

pub trait Builder {
    fn build(self) -> JsExecutable;
}
