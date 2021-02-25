//! Implementation and definition of the object which represents some JS code
//! which can be executed in the browser.

use std::iter;

use serde_json::Value as Json;

use crate::object::ObjectPtr;

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
pub struct Statement {
    /// Actual JS code to execute.
    expression: String,

    /// Arguments for the [`Statement::expression`] which will be provided
    /// as `args` array.
    args: Vec<Json>,

    /// [`ObjectPtr`] to the JS objects needed by [`Statement::expression`]
    /// which will be provided as `objs` array.
    objs: Vec<ObjectPtr>,

    /// [`Statement`] which should be executed after this [`Statement`].
    ///
    /// Result returned from this [`Statement`] will be provided to the
    /// [`Statement::and_then`].
    and_then: Option<Box<Statement>>,
}

impl Statement {
    /// Returns new [`Statement`] with a provided JS code and arguments.
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

    /// Returns new [`Statement`] with a provided JS code, arguments and
    /// objects.
    #[allow(dead_code)]
    pub fn with_objs(
        expression: &str,
        args: Vec<Json>,
        objs: Vec<ObjectPtr>,
    ) -> Self {
        Self {
            expression: expression.to_string(),
            args,
            objs,
            and_then: None,
        }
    }

    /// Executes another [`Statement`] after this one executed successfully.
    ///
    /// The success value is passed to a next [`Statement`] as JS lambda
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
    pub(super) fn prepare(self) -> (String, Vec<Json>) {
        // language=JavaScript
        let mut final_js = r#"
            let lastResult;
            let objs;
            let args;
        "#
        .to_string();
        let mut args = Vec::new();

        let mut statement = Some(Box::new(self));
        let mut i = 0;
        while let Some(mut e) = statement.take() {
            final_js.push_str(&e.step_js(i));
            i += 1;
            args.push(std::mem::take(&mut e.args).into());
            statement = e.and_then;
        }

        (final_js, args)
    }

    /// Returns JS code which obtains [`Statement::objs`] JS objects.
    ///
    /// Should be injected to the [`Statement::step_js`] code.
    fn objects_injection_js(&self) -> String {
        iter::once("objs = [];\n".to_string())
            .chain(self.objs.iter().map(|id| {
                format!("objs.push(window.registry.get('{}'));\n", id)
            }))
            .collect()
    }

    /// Returns JS code for this [`Statement`].
    ///
    /// Doesn't generates code for the [`Statement::and_then`].
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
