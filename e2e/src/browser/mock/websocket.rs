//! `WebSocket` object mock.

use crate::browser::{Statement, Window};

/// Mock for a `WebSocket` WebAPI object.
pub struct WebSocket<'a>(pub(super) &'a Window);

impl<'a> WebSocket<'a> {
    /// Instantiates a new `WebSocket` mock in the provided [`Window`].
    pub(super) async fn instantiate(window: &Window) {
        window
            .execute(Statement::new(
                // language=JavaScript
                r#"
                    async () => {
                        let ws = {
                            originalSend: WebSocket.prototype.send,
                            isClosed: false,
                            closeCode: 0,
                            allSockets: []
                        };
                        window.wsMock = ws;

                        window.wsConstructor = (url) => {
                            let createdWs = new window.originalWs(url);
                            ws.allSockets.push(createdWs);
                            if (ws.isClosed) {
                                createdWs.dispatchEvent(
                                    new CloseEvent("close", { code: ws.code })
                                );
                            }
                            return createdWs;
                        };
                    }
                "#,
                [],
            ))
            .await
            .unwrap();
    }

    /// Fires a `CloseEvent` with the provided code on all the created
    /// `WebSocket` instances.
    ///
    /// Will fire this `CloseEvent` on every `WebSocket`'s constructor call,
    /// until [`WebSocket::disable_connection_loss()`] will be called.
    ///
    /// # Panics
    ///
    /// If failed to execute JS statement.
    pub async fn enable_connection_loss(&self, code: u64) {
        self.0
            .execute(Statement::new(
                // language=JavaScript
                r#"
                    async () => {
                        const [code] = args;
                        for (socket of window.wsMock.allSockets) {
                            window.wsMock.isClosed = true;
                            window.wsMock.closeCode = code;
                            socket.dispatchEvent(
                                new CloseEvent("close", { code: code })
                            );
                        }
                    }
                "#,
                [code.into()],
            ))
            .await
            .unwrap();
    }

    /// Disables [`WebSocket::enable_connection_loss()`] effects.
    ///
    /// After this method call, `WebSocket`'s constructor will work the same way
    /// as without mock.
    ///
    /// # Panics
    ///
    /// If failed to execute JS statement.
    pub async fn disable_connection_loss(&self) {
        self.0
            .execute(Statement::new(
                // language=JavaScript
                r#"
                    async () => {
                        window.wsMock.isClosed = false;
                        window.wsMock.closeCode = 0;
                    }
                "#,
                [],
            ))
            .await
            .unwrap();
    }
}
