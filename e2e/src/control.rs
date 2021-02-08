//! Implementation of the object for interacting with a Medea Control API.

use derive_more::{Display, Error, From};
use medea_control_api_mock::{
    api::{Response, SingleGetResponse},
    proto::{CreateResponse, Element},
};
use reqwest::Client;

use crate::conf;

/// Returns URL to the Control API for the provided [`Element`] path.
fn get_url(path: &str) -> String {
    format!("{}/{}", *conf::CONTROL_API_ADDR, path)
}

/// All errors which can happen while working with Control API.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Client for the Control API.
pub struct ControlApi(Client);

impl ControlApi {
    /// Returns new [`ControlApi`] client.
    pub fn new() -> Self {
        Self(Client::new())
    }

    /// Creates provided [`Element`] in the provided `path`.
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

    /// Deletes [`Element`] in the provided `path`.
    #[allow(dead_code)]
    pub async fn delete(&self, path: &str) -> Result<Response> {
        Ok(self.0.delete(&get_url(path)).send().await?.json().await?)
    }

    /// Returns [`Element`] from the provided `path`.
    #[allow(dead_code)]
    pub async fn get(&self, path: &str) -> Result<SingleGetResponse> {
        Ok(self.0.get(&get_url(path)).send().await?.json().await?)
    }
}
