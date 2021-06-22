#![forbid(non_ascii_idents, unsafe_code)]

mod browser;
mod conf;
mod control;
mod object;
mod steps;
mod world;

use cucumber_rust::WorldInit as _;

use self::world::World;

#[tokio::main]
async fn main() {
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}
