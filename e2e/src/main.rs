#![allow(dead_code)]

mod browser;
mod conf;
mod control;
mod entity;
mod file_server;
mod world;

use cucumber_rust::WorldInit as _;

use self::{file_server::FileServer, world::BrowserWorld};

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = BrowserWorld::init(&["./features"]);
    runner.run_and_exit().await;
}
