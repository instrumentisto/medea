//! Implementation and definition of the object which represents some JS code
//! which can be executed in the browser.

use std::iter;

use serde_json::Value as Json;

use crate::entity::EntityPtr;

/// Representation of the JS code which can be executed in the browser.
///
/// Example of JS expression:
///
/// ```js
/// async (lastResult) => {
///     const [room] = objs;
///     const [id] = args;
///     // ...
///
///     return "foobar";
/// }
/// ```
pub struct JsExecutable {
    /// Actual JS code to execute.
    expression: String,

    /// Arguments for the [`JsExecutable::expression`] which will be provided
    /// as `args` array.
    args: Vec<Json>,

    /// [`EntityPtr`] to the JS objects needed by [`JsExecutable::expression`]
    /// which will be provided as `objs` array.
    objs: Vec<EntityPtr>,

    /// [`JsExecutable`] which should be executed after this [`JsExecutable`].
    ///
    /// Result returned from this [`JsExecutable`] will be provided to the
    /// [`JsExecutable::and_then`].
    and_then: Option<Box<JsExecutable>>,
}

impl JsExecutable {
    /// Returns new [`JsExecutable`] with a provided JS code and arguments.
    ///
    /// Example of JS expression:
    ///
    /// ```js
    /// async (lastResult) => {
    ///     const [room] = objs;
    ///     const [id] = args;
    ///     // ...
    ///
    ///     return "foobar";
    /// }
    /// ```
    pub fn new(expression: &str, args: Vec<Json>) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs: Vec::new(),
            and_then: None,
        }
    }

    /// Returns new [`JsExecutable`] with a provided JS code, arguments and
    /// objects.
    #[allow(dead_code)]
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
        }
    }

    /// Executes another [`JsExecutable`] after this one executed successfully.
    ///
    /// The success value is passed to a next [`JsExecutable`] as JS lambda
    /// argument.
    #[allow(clippy::option_if_let_else)]
    pub fn and_then(mut self, another: Self) -> Self {
        if let Some(e) = self.and_then {
            self.and_then = Some(Box::new(e.and_then(another)));
            self
        } else {
            self.and_then = Some(Box::new(another));
            self
        }
    }

    /// Returns JS code which should be executed in the browser and [`Json`]
    /// arguments for this code.
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

    /// Returns JS code which obtains [`JsExecutable::objs`] JS objects.
    ///
    /// Should be injected to the [`JsExecutable::step_js`] code.
    fn objects_injection_js(&self) -> String {
        iter::once("objs = [];\n".to_string())
            .chain(self.objs.iter().map(|id| {
                format!("objs.push(window.holders.get('{}'));\n", id)
            }))
            .collect()
    }

    /// Returns JS code for this [`JsExecutable`].
    ///
    /// Doesn't generates code for the [`JsExecutable::and_then`].
    fn step_js(&self, i: usize) -> String {
        format!(
            r#"
            args = arguments[{i}];
            {objs_js}
            lastResult = await ({expr})(lastResult);
        "#,
            i = i,
            objs_js = self.objects_injection_js(),
            expr = self.expression
        )
    }
}
