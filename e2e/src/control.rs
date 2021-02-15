//! HTTP client that interacts with Medea via Control API.

use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    api::{Response, SingleGetResponse},
    proto::{CreateResponse, Element},
};

use crate::conf;
use medea_control_api_mock::callback::CallbackItem;

/// All errors which can happen while working with Control API.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Client for the Control API.
pub struct Client(reqwest::Client);

impl Client {
    /// Returns new [`Client`] client.
    pub fn new() -> Self {
        Self(reqwest::Client::new())
    }

    /// Creates provided [`Element`] in the provided `path`.
    pub async fn create(
        &self,
        path: &str,
        element: Element,
    ) -> Result<CreateResponse> {
        Ok(self
            .0
            .post(&get_url_to_element(path))
            .json(&element)
            .send()
            .await?
            .json()
            .await?)
    }

    /// Deletes [`Element`] in the provided `path`.
    pub async fn delete(&self, path: &str) -> Result<Response> {
        Ok(self
            .0
            .delete(&get_url_to_element(path))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Returns [`Element`] from the provided `path`.
    #[allow(dead_code)]
    pub async fn get(&self, path: &str) -> Result<SingleGetResponse> {
        Ok(self
            .0
            .get(&get_url_to_element(path))
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn callbacks(&self) -> Result<Vec<CallbackItem>> {
        Ok(self
            .0
            .get(&format!("{}/callbacks", *conf::CONTROL_API_ADDR))
            .send()
            .await?
            .json()
            .await?)
    }
}

/// Returns URL to the Control API for the provided [`Element`] path.
fn get_url_to_element(path: &str) -> String {
    format!("{}/control-api/{}", *conf::CONTROL_API_ADDR, path)
}
