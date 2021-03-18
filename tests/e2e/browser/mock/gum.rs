//! Implementation of the `MediaDevices.getUserMedia` function mock.

use crate::browser::{Statement, Window};

/// Mock for the `WebSocket` WebAPI object.
pub struct Gum<'a>(pub(super) &'a Window);

impl<'a> Gum<'a> {
    /// Instantiates `MediaDevices.getUserMedia` mock in the provided
    /// [`Window`].
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

    /// Brokes `MediaDevice.getUserMedia` requests for the provided media types.
    ///
    /// If some media type is broken, then `NotFoundError` will be thrown on
    /// each gUM request.
    pub async fn broke_gum(&self, video: bool, audio: bool) {
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
