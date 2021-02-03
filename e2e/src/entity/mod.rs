//! Implementation of the all browser-side entities.

use std::marker::PhantomData;

use serde_json::Value as Json;

use crate::browser::{JsExecutable, WebClient};

/// Representation of some object from the browser-side.
pub struct Entity<T> {
    id: String,
    client: WebClient,
    _entity_type: PhantomData<T>,
}

impl<T> Drop for Entity<T> {
    fn drop(&mut self) {
        let id = self.id.clone();
        let mut client = self.client.clone();
        tokio::spawn(async move {
            client
                .execute_async(JsExecutable::new(
                    r#"
                    async () => {
                        const [id] = args;
                        window.holders.remove(id);
                    }
                "#,
                    vec![id.into()],
                ))
                .await
                .unwrap();
        });
    }
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
