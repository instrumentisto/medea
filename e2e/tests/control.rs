//! HTTP client interacting with Medea via its Control API.

use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    api::Response,
    callback::CallbackItem,
    proto::{CreateResponse, Element, SingleGetResponse},
};

/// All errors which can happen while working with a Control API.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Client of a Control API.
pub struct Client {
    inner: reqwest::Client,
    control_api_address: String,
}

impl Client {
    /// Returns a new Control API [`Client`].
    #[inline]
    #[must_use]
    pub fn new(control_api_address: &str) -> Self {
        Self {
            inner: reqwest::Client::new(),
            control_api_address: control_api_address.to_owned(),
        }
    }

    /// Creates the provided media [`Element`] in the provided `path` on a Medea
    /// media server.
    pub async fn create(
        &self,
        path: &str,
        element: Element,
    ) -> Result<CreateResponse> {
        Ok(self
            .inner
            .post(&get_url(&self.control_api_address, path))
            .json(&element)
            .send()
            .await?
            .json()
            .await?)
    }

    /// Deletes a media [`Element`] identified by the provided `path`.
    pub async fn delete(&self, path: &str) -> Result<Response> {
        Ok(self
            .inner
            .delete(&get_url(&self.control_api_address, path))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Returns a media [`Element`] identified by the provided `path`.
    pub async fn get(&self, path: &str) -> Result<SingleGetResponse> {
        Ok(self
            .inner
            .get(&get_url(&self.control_api_address, path))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Applies on a media server the provided media [`Element`] identified by
    /// the provided `path`.
    pub async fn apply(
        &self,
        path: &str,
        element: Element,
    ) -> Result<CreateResponse> {
        Ok(self
            .inner
            .put(&get_url(&self.control_api_address, path))
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
            .inner
            .get(&format!("{}/callbacks", self.control_api_address))
            .send()
            .await?
            .json()
            .await?)
    }
}

/// Returns URL of a media [`Element`] identified by the provided `path`.
#[must_use]
fn get_url(control_api_address: &str, path: &str) -> String {
    format!("{}/control-api/{}", control_api_address, path)
}
