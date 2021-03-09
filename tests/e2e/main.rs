#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod object;
mod steps;
mod world;

use cucumber_rust::WorldInit as _;
use tokio_1 as tokio;

use self::world::World;

use crate::object::{room::ParsingFailedError, MediaKind, MediaSourceKind};

#[tokio::main]
async fn main() {
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

/// Parses [`MediaKind`] and [`MediaSourceKind`] from the provided [`str`].
fn parse_media_kinds(
    s: &str,
) -> Result<(MediaKind, MediaSourceKind), ParsingFailedError> {
    let media_kind = s.parse()?;
    let source_kind = match media_kind {
        MediaKind::Audio => MediaSourceKind::Device,
        MediaKind::Video => s.parse()?,
    };
    Ok((media_kind, source_kind))
}
