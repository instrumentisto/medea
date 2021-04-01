//! HTTP client interacting with Medea via its Control API.

use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    api::Response,
    callback::CallbackItem,
    proto::{CreateResponse, Element, SingleGetResponse},
};

use crate::conf;

/// All errors which can happen while working with a Control API.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Client of a Control API.
pub struct Client(reqwest::Client);

impl Client {
    /// Returns a new Control API [`Client`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(reqwest::Client::new())
    }

    /// Creates the provided media [`Element`] in the provided `path` on a Medea
    /// media server.
    pub async fn create(
        &self,
        path: &str,
        element: Element,
    ) -> Result<CreateResponse> {
        Ok(self
            .0
            .post(&get_url(path))
            .json(&element)
            .send()
            .await?
            .json()
            .await?)
    }

    /// Deletes a media [`Element`] identified by the provided `path`.
    pub async fn delete(&self, path: &str) -> Result<Response> {
        Ok(self.0.delete(&get_url(path)).send().await?.json().await?)
    }

    pub async fn get(&self, path: &str) -> Result<SingleGetResponse> {
        Ok(self.0.get(&get_url(path)).send().await?.json().await?)
    }

    /// Applies the provided media [`Element`] in the provided `path` on a Medea
    /// media server.
    pub async fn apply(
        &self,
        path: &str,
        element: Element,
    ) -> Result<CreateResponse> {
        Ok(self
            .0
            .put(&get_url(path))
            .json(&element)
            .send()
            .await?
            .json()
            .await?)
    }

    // TODO: Server side filtering on GET requests or SSE/WS subscription would
    //       speed up things. We a probably wasting a lot of time on ser/deser
    //       of huge JSON's.
    /// Fetches all callbacks received by Control API mock server.
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

/// Returns URL of a media [`Element`] identified by the provided `path`.
#[must_use]
fn get_url(path: &str) -> String {
    format!("{}/control-api/{}", *conf::CONTROL_API_ADDR, path)
}
