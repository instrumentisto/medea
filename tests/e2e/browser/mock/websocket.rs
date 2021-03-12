use crate::browser::{Statement, Window};

pub struct WebSocket<'a>(pub(super) &'a Window);

impl<'a> WebSocket<'a> {
    pub(super) async fn instantiate(window: &Window) {
        window.execute(Statement::new(
            r#"
                async () => {
                    let ws = {
                        originalSend: WebSocket.prototype.send,
                        originalConstructor: WebSocket.prototype.constructor,
                        isClosed: false,
                        closeCode: 0,
                        allSockets: []
                    };
                    window.wsMock = ws;

                    WebSocket.prototype.constructor = (url) => {
                        let createdWs = ws.originalConstructor(url);
                        ws.allSockets.push(createdWs);
                        if (ws.isClosed) {
                            createdWs.dispatchEvent(new CloseEvent("close", { code: ws.code }));
                        }

                        return createdWs;
                    };
                }
            "#,
            vec![]
        )).await.unwrap();
    }

    pub async fn enable_message_loss(&self) {
        self.0
            .execute(Statement::new(
                r#"
                async () => {
                    WebSocket.prototype.send = () => {};
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();
    }

    pub async fn disable_message_loss(&self) {
        self.0
            .execute(Statement::new(
                r#"
                async () => {
                    WebSocket.prototype.send = window.wsMock.originalSend;
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();
    }

    pub async fn enable_connection_loss(&self, code: u64) {
        self.0.execute(Statement::new(r#"
            async () => {
                const [code] = args;
                for (socket of window.wsMock.allSockets) {
                    window.wsMock.isClosed = true;
                    window.wsMock.closeCode = code;
                    socket.dispatchEvent(new CloseEvent("close", { code: code }));
                }
            }
        "#, vec![code.into()])).await.unwrap();
    }

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
