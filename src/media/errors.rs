use failure::Fail;

#[derive(Fail, Debug)]
pub enum MediaError {
    #[fail(display = "Unmatched state of track")]
    UnmatchedTrackState,
}
