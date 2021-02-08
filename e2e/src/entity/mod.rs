//! Implementation of the all browser-side entities.

pub mod connection;
pub mod connections_store;
pub mod jason;
pub mod room;

use std::{marker::PhantomData, sync::mpsc};

use derive_more::{Display, Error, From};
use serde_json::Value as Json;
use tokio::task;
use uuid::Uuid;

use crate::browser::{self, JsExecutable, WindowWebClient};

/// All errors which can happen while working with entities.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// Error while interacting with browser.
    Browser(browser::Error),

    /// Failed JS object type casting.
    TypeCast,
}

/// Pointer to the JS object on browser-side.
#[derive(Clone, Debug, Display)]
pub struct EntityPtr(String);

/// Representation of some object from the browser-side.
///
/// JS object on browser-side will be removed on this [`Entity`] [`Drop::drop`].
pub struct Entity<T> {
    /// Pointer to the JS object on browser-side.
    ptr: EntityPtr,

    /// [`WindowWebClient`] where this [`Entity`] is exists.
    client: WindowWebClient,

    /// Type of [`Entity`].
    _entity_type: PhantomData<T>,
}

impl<T> Drop for Entity<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.clone();
        let client = self.client.clone();
        let (tx, rx) = mpsc::channel();
        tokio::spawn(async move {
            client
                .execute(JsExecutable::new(
                    r#"
                    async () => {
                        const [id] = args;
                        window.holders.delete(id);
                    }
                "#,
                    vec![ptr.to_string().into()],
                ))
                .await
                .unwrap();
            tx.send(()).unwrap();
        });
        task::block_in_place(move || {
            rx.recv().unwrap();
        });
    }
}

impl<T> Entity<T> {
    /// Returns [`Entity`] with a provided URI and [`WindowWebClient`].
    pub fn new(uri: String, client: WindowWebClient) -> Self {
        Self {
            ptr: EntityPtr(uri),
            client,
            _entity_type: PhantomData::default(),
        }
    }

    /// Returns new [`Entity`] which will be created by the provided
    /// [`JsExecutable`].
    ///
    /// JS object which this [`Entity`] represents will be passed to the
    /// provided [`JsExecutable`] as lambda argument.
    pub async fn spawn_entity<O>(
        &self,
        exec: JsExecutable,
    ) -> Result<Entity<O>, Error> {
        let id = Uuid::new_v4().to_string();
        self.execute(exec.and_then(JsExecutable::new(
            r#"
                async (obj) => {
                    const [id] = args;
                    window.holders.set(id, obj);
                }
            "#,
            vec![id.clone().into()],
        )))
        .await?;

        Ok(Entity::new(id, self.client.clone()))
    }

    /// Returns `true` if this [`Entity`] is `undefined`.
    pub async fn is_undefined(&self) -> Result<bool, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (o) => {
                    return o === undefined;
                }
            "#,
                vec![],
            ))
            .await?
            .as_bool()
            .ok_or(Error::TypeCast)?)
    }

    /// Executes provided [`JsExecutable`] in the browser.
    ///
    /// JS object which this [`Entity`] represents will be passed to the
    /// provided [`JsExecutable`] as lambda argument.
    async fn execute(&self, js: JsExecutable) -> Result<Json, Error> {
        Ok(self.client.execute(self.get_obj().and_then(js)).await?)
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
    /// Spawns provided `obj` [`Entity`] in the provided [`WindowWebClient`].
    pub async fn spawn(
        obj: T,
        client: WindowWebClient,
    ) -> Result<Entity<T>, Error> {
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
            .await?;

        Ok(Entity::new(id, client))
    }
}

/// Abstraction which will be used for JS object creating for the [`Entity`].
pub trait Builder {
    /// Returns [`JsExecutable`] with which JS object for this object will be
    /// created.
    fn build(self) -> JsExecutable;
}
