//! Implementation of the object for interacting with a Medea Control API.

use derive_more::From;
use medea_control_api_mock::{
    api::{Response, SingleGetResponse},
    proto::{CreateResponse, Element},
};
use reqwest::Client;

use crate::conf;

fn get_url(path: &str) -> String {
    format!("{}/{}", *conf::CONTROL_API_ADDR, path)
}

#[derive(Debug, From)]
pub enum Error {
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub struct ControlApi(Client);

impl ControlApi {
    pub fn new() -> Self {
        Self(Client::new())
    }

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

    pub async fn delete(&self, path: &str) -> Result<Response> {
        Ok(self.0.delete(&get_url(path)).send().await?.json().await?)
    }

    pub async fn get(&self, path: &str) -> Result<SingleGetResponse> {
        Ok(self.0.get(&get_url(path)).send().await?.json().await?)
    }
}