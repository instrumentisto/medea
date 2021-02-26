use crate::browser::{Statement, Window};

pub struct Gum<'a>(pub(super) &'a Window);

impl<'a> Gum<'a> {
    pub(super) async fn instantiate(window: &Window) {
        window
            .execute(Statement::new(
                r#"
                async () => {
                    window.gumMock = {
                        originalGum: navigator.mediaDevices.getUserMedia
                    };
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();
    }
}
