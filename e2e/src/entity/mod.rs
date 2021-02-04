//! Implementation of the all browser-side entities.

pub mod connection;
pub mod connections_store;
pub mod jason;
pub mod room;

use std::marker::PhantomData;

use derive_more::Display;
use serde_json::Value as Json;

use crate::browser::{self, JsExecutable, WebClient};
use uuid::Uuid;

/// Representation of some object from the browser-side.
pub struct Entity<T> {
    ptr: EntityPtr,
    client: WebClient,
    _entity_type: PhantomData<T>,
}

impl<T> Drop for Entity<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.clone();
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
                    vec![ptr.to_string().into()],
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
            ptr: EntityPtr(uri),
            client,
            _entity_type: PhantomData::default(),
        }
    }

    pub fn ptr(&self) -> EntityPtr {
        self.ptr.clone()
    }

    pub async fn spawn_ent<O>(&mut self, exec: JsExecutable) -> Entity<O> {
        let id = Uuid::new_v4().to_string();
        self.execute(exec.and_then(JsExecutable::new(
            r#"
                async (obj) => {
                    const [id] = args;
                    window.holders.insert(id, obj);
                }
            "#,
            vec![id.clone().into()]
        ))).await;

        Entity::new(id, self.client.clone())
    }

    pub async fn spawn_entity<O: Builder>(&mut self, obj: O) -> Entity<O> {
        Entity::spawn(obj, self.client.clone()).await
    }

    pub async fn is_undefined(&mut self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (o) => {
                    return o === undefined;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
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
            vec![self.ptr.to_string().into()],
        )
    }
}

impl<T: Builder> Entity<T> {
    pub async fn spawn(obj: T, mut client: WebClient) -> Entity<T> {
        let id = Uuid::new_v4().to_string();
        client
            .execute(obj.build().and_then(JsExecutable::new(
                r#"
                    async (obj) => {
                        const [id] = args;
                        window.holders.set(id, obj);
                    }
                "#,
                vec![id.clone().into()],
            )))
            .await
            .unwrap();

        Entity::new(id, client)
    }
}

/// Abstraction which will be used for JS object creating for the [`Entity`].
pub trait Builder {
    /// Returns [`JsExecutable`] with which JS object for this object will be
    /// created.
    fn build(self) -> JsExecutable;
}

#[derive(Clone, Debug, Display)]
pub struct EntityPtr(String);
