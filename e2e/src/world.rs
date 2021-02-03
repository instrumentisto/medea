//! Implementation of world for the tests.

use std::convert::Infallible;

use async_trait::async_trait;
use cucumber_rust::{World, WorldInit};
use uuid::Uuid;

use crate::{
    browser::{JsExecutable, WebClient},
    entity::{Builder, Entity},
};

/// World which will be used by all E2E tests.
#[derive(WorldInit)]
pub struct BrowserWorld {
    entity_factory: EntityFactory,
}

impl BrowserWorld {
    pub async fn new(mut client: WebClient) -> Self {
        client
            .execute_async(JsExecutable::new(
                r#"
                async () => {
                    window.holders = new Map();
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();
        Self {
            entity_factory: EntityFactory(client),
        }
    }
}

#[async_trait(?Send)]
impl World for BrowserWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self::new(WebClient::new().await).await)
    }
}

struct EntityFactory(WebClient);

impl EntityFactory {
    pub async fn new_entity<T>(&mut self, obj: T) -> Entity<T>
    where
        T: Builder,
    {
        let id = Uuid::new_v4().to_string();
        self.0
            .execute_async(obj.build().and_then(JsExecutable::new(
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

        Entity::new(id, self.0.clone())
    }
}
