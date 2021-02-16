//! Implementation of the all browser-side objects.

pub mod connection;
pub mod connections_store;
mod jason;
mod room;

use std::{marker::PhantomData, sync::mpsc};

use derive_more::{Display, Error, From};
use serde_json::Value as Json;
use tokio_e2e::task;
use uuid::Uuid;

use crate::browser::{self, Statement};

pub use self::{
    jason::Jason,
    room::{MediaKind, MediaSourceKind, Room},
};

/// All errors which can happen while working with objects.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    /// Error while interacting with browser.
    Browser(browser::Error),

    /// Failed JS object type casting.
    TypeCast,
}

/// Pointer to the JS object on browser-side.
#[derive(Clone, Debug, Display)]
pub struct ObjectPtr(String);

/// Representation of some object from the browser-side.
///
/// JS object on browser-side will be removed on this [`Object`] [`Drop::drop`].
pub struct Object<T> {
    /// Pointer to the JS object on browser-side.
    ptr: ObjectPtr,

    /// [`Window`] where this [`Object`] is exists.
    window: browser::Window,

    /// Type of [`Object`].
    _kind: PhantomData<T>,
}

impl<T> Drop for Object<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.clone();
        let window = self.window.clone();
        let (tx, rx) = mpsc::channel();
        tokio_e2e::spawn(async move {
            window
                .execute(Statement::new(
                    r#"
                    async () => {
                        const [id] = args;
                        window.registry.delete(id);
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

impl<T> Object<T> {
    /// Returns [`Object`] with a provided ID and [`Window`].
    pub fn new(id: String, window: browser::Window) -> Self {
        Self {
            ptr: ObjectPtr(id),
            window,
            _kind: PhantomData::default(),
        }
    }

    /// Executes provided statement that returns [`Object`].
    pub async fn execute_and_fetch<O>(
        &self,
        statement: Statement,
    ) -> Result<Object<O>, Error> {
        let id = Uuid::new_v4().to_string();
        self.execute(statement.and_then(Statement::new(
            r#"
                async (obj) => {
                    const [id] = args;
                    window.registry.set(id, obj);
                }
            "#,
            vec![id.clone().into()],
        )))
        .await?;

        Ok(Object::new(id, self.window.clone()))
    }

    /// Returns `true` if this [`Object`] is `undefined`.
    pub async fn is_undefined(&self) -> Result<bool, Error> {
        Ok(self
            .execute(Statement::new(
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

    /// Executes provided [`Statement`] in the browser.
    ///
    /// JS object which this [`Object`] represents will be passed to the
    /// provided [`Statement`] as lambda argument.
    async fn execute(&self, js: Statement) -> Result<Json, Error> {
        Ok(self.window.execute(self.get_obj().and_then(js)).await?)
    }

    /// Returns [`Statement`] which will obtain JS object of this [`Object`].
    fn get_obj(&self) -> Statement {
        Statement::new(
            r#"
                async () => {
                    const [id] = args;
                    return window.registry.get(id);
                }
            "#,
            vec![self.ptr.to_string().into()],
        )
    }
}

impl<T: Builder> Object<T> {
    /// Spawns provided `obj` [`Object`] in the provided [`Window`].
    pub async fn spawn(
        obj: T,
        window: browser::Window,
    ) -> Result<Object<T>, Error> {
        let id = Uuid::new_v4().to_string();
        window
            .execute(obj.build().and_then(Statement::new(
                r#"
                    async (obj) => {
                        const [id] = args;
                        window.registry.set(id, obj);
                    }
                "#,
                vec![id.clone().into()],
            )))
            .await?;

        Ok(Object::new(id, window))
    }
}

/// Abstraction which will be used for JS object creating for the [`Object`].
pub trait Builder {
    /// Returns [`Statement`] with which JS object for this object will be
    /// created.
    fn build(self) -> Statement;
}
