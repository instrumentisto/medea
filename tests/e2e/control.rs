//! HTTP client interacting with Medea via its Control API.

use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    api::Response,
    proto::{CreateResponse, Element},
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
}

/// Returns URL of a media [`Element`] identified by the provided `path`.
#[must_use]
fn get_url(path: &str) -> String {
    format!("{}/{}", *conf::CONTROL_API_ADDR, path)
}
