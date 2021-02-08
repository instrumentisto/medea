use std::iter;

use serde_json::Value as Json;

use crate::entity::EntityPtr;

pub struct JsExecutable {
    expression: String,
    args: Vec<Json>,
    objs: Vec<EntityPtr>,
    and_then: Option<Box<JsExecutable>>,
    depth: u32,
}

impl JsExecutable {
    pub fn new(expression: &str, args: Vec<Json>) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs: Vec::new(),
            and_then: None,
            depth: 0,
        }
    }

    pub fn with_objs(
        expression: &str,
        args: Vec<Json>,
        objs: Vec<EntityPtr>,
    ) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs,
            and_then: None,
            depth: 0,
        }
    }

    #[allow(clippy::option_if_let_else)]
    pub fn and_then(mut self, mut another: Self) -> Self {
        if let Some(e) = self.and_then {
            self.and_then = Some(Box::new(e.and_then(another)));
            self
        } else {
            another.depth = self.depth + 1;
            self.and_then = Some(Box::new(another));
            self
        }
    }

    pub(super) fn finalize(self) -> (String, Vec<Json>) {
        let mut final_js = r#"
            let lastResult;
            let objs;
            let args;
        "#
        .to_string();
        let mut args = Vec::new();

        let mut executable = Some(Box::new(self));
        let mut i = 0;
        while let Some(mut e) = executable.take() {
            final_js.push_str(&e.step_js(i));
            i += 1;
            args.push(std::mem::take(&mut e.args).into());
            executable = e.and_then;
        }

        (final_js, args)
    }

    fn objects_injection_js(&self) -> String {
        iter::once("objs = [];\n".to_string())
            .chain(self.objs.iter().map(|id| {
                format!("objs.push(window.holders.get('{}'));\n", id)
            }))
            .collect()
    }

    fn step_js(&self, i: usize) -> String {
        format!(
            r#"
            args = arguments[{depth}];
            {objs_js}
            lastResult = await ({expr})(lastResult);
        "#,
            depth = i,
            objs_js = self.objects_injection_js(),
            expr = self.expression
        )
    }
}
