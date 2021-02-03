use reqwest::{Client};
use medea_control_api_mock::proto::Element;
use medea_control_api_mock::proto::CreateResponse;
use medea_control_api_mock::api::{Response, SingleGetResponse};

fn get_url(path: &str) -> String {
    format!("http://127.0.0.1:8000/control-api/{}", path)
}

pub struct ControlApi(Client);

impl ControlApi {
    pub fn new() -> Self {
        Self(Client::new())
    }

    pub async fn create(&self, path: &str, element: Element) -> CreateResponse {
        self.0.post(&get_url(path))
            .json(&element)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn delete(&self, path: &str) -> Response {
        self.0.delete(&get_url(path))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn get(&self, path: &str) -> SingleGetResponse {
        self.0.get(&get_url(path))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}