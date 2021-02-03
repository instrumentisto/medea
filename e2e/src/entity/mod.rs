//! Implementation of the all browser-side entities.

pub mod room;

use std::marker::PhantomData;

use serde_json::Value as Json;

use crate::browser::{self, JsExecutable, WebClient};

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
                .execute(JsExecutable::new(
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
    /// Returns [`Entity`] with a provided URI and [`WebClient`].
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

    /// Executes provided [`JsExecutable`] in the browser.
    async fn execute(
        &mut self,
        js: JsExecutable,
    ) -> Result<Json, browser::Error> {
        self.client.execute(self.get_obj().and_then(js)).await
    }

    /// Returns [`JsExecutable`] which will obtain JS object of this [`Entity`].
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

/// Abstraction which will be used for JS object creating for the [`Entity`].
pub trait Builder {
    /// Returns [`JsExecutable`] with which JS object for this object will be
    /// created.
    fn build(self) -> JsExecutable;
}
