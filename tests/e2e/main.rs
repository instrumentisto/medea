mod browser;
mod conf;
mod control;
mod object;
mod steps;
mod world;

use cucumber_rust::WorldInit as _;
use tokio_1 as tokio;

use self::world::World;

use crate::object::{room::FailedParsing, MediaKind, MediaSourceKind};

#[tokio::main]
async fn main() {
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

/// Tries to find `audio`, `video` or `all` in the provided text. If `audio` or
/// `video` found, then [`Some`] [`MediaKind`] will be returned. If `all` found,
/// the [`None`] will be returned. Otherwise this function will panic.
fn parse_media_kind(text: &str) -> Option<MediaKind> {
    if text.contains("audio") {
        Some(MediaKind::Audio)
    } else if text.contains("video") {
        Some(MediaKind::Video)
    } else if text.contains("all") {
        None
    } else {
        unreachable!()
    }
}

/// Parses [`MediaKind`] and [`MediaSourceKind`] from the provided [`str`].
fn parse_media_kinds(
    s: &str,
) -> Result<(MediaKind, MediaSourceKind), FailedParsing> {
    let media_kind = s.parse()?;
    let source_kind = match media_kind {
        MediaKind::Audio => MediaSourceKind::Device,
        MediaKind::Video => s.parse()?,
    };
    Ok((media_kind, source_kind))
}
