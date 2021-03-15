//! Implementation of the `WebSocket` object mock.

use crate::browser::{Statement, Window};

/// Mock for the `WebSocket` WebAPI object.
pub struct WebSocket<'a>(pub(super) &'a Window);

impl<'a> WebSocket<'a> {
    /// Instantiates `WebSocket` mock in the provided [`Window`].
    pub(super) async fn instantiate(window: &Window) {
        window
            .execute(Statement::new(
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
                vec![],
            ))
            .await
            .unwrap();
    }

    /// Fires `CloseEvent` with a provided code on the all created `WebSocket`
    /// instances.
    ///
    /// Will fire this `CloseEvent` on every `WebSocket`'s constructor call,
    /// until [`WebSocket::disable_connection_loss`] will be called.
    pub async fn enable_connection_loss(&self, code: u64) {
        self.0
            .execute(Statement::new(
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
                vec![code.into()],
            ))
            .await
            .unwrap();
    }

    /// Disables [`WebSocket::enable_connection_loss`] effects.
    ///
    /// After this method call, `WebSocket`'s constructor will work the same way
    /// as without mock.
    pub async fn disable_connection_loss(&self) {
        self.0
            .execute(Statement::new(
                r#"
            async () => {
                window.wsMock.isClosed = false;
                window.wsMock.closeCode = 0;
            }
        "#,
                vec![],
            ))
            .await
            .unwrap();
    }
}
