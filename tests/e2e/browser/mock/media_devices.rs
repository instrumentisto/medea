//! Implementation of the [MediaDevices][1] interface mock.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediadevices
use crate::browser::{Statement, Window};

/// Mock for the [MediaDevices][1] interface.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediadevices
pub struct MediaDevices<'a>(pub(super) &'a Window);

impl<'a> MediaDevices<'a> {
    /// Instantiates [MediaDevices][1] interface mock in the provided
    /// [`Window`].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediadevices
    pub(super) async fn instantiate(window: &Window) {
        window
            .execute(Statement::new(
                // language=JavaScript
                r#"
                    async () => {
                        window.gumMock = {
                            original: navigator.mediaDevices.getUserMedia
                        };
                    }
                "#,
                [],
            ))
            .await
            .unwrap();
    }

    /// Makes [getUserMedia()][1] requests return error for the provided media
    /// types.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    pub async fn mock_gum(&self, video: bool, audio: bool) {
        self.0
            .execute(Statement::new(
                // language=JavaScript
                r#"
                    async () => {
                        const [isVideoBroken, isAudioBroken] = args;
                        navigator.mediaDevices.getUserMedia = async (cons) => {
                            if (isAudioBroken && cons.audio != null) {
                                throw new NotFoundError();
                            }
                            if (isVideoBroken && cons.video != null) {
                                throw new NotFoundError();
                            }
                            return await window.gumMock.original(cons);
                        }
                    }
                "#,
                [video.into(), audio.into()],
            ))
            .await
            .unwrap();
    }
}
