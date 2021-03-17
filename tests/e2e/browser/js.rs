//! JS code executable in a browser.

use std::{iter, mem};

use serde_json::Value as Json;

use crate::object::ObjectPtr;

/// Representation of a JS code executable in a browser.
///
/// Example of a JS expression:
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
    /// Actual JS code to be executed.
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
    /// Returns a new [`Statement`] with the provided JS code and arguments.
    ///
    /// Example of a JS expression:
    /// ```js
    /// async (lastResult) => {
    ///     const [room] = objs;
    ///     const [id] = args;
    ///     // ...
    ///
    ///     return "foobar";
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn new<A: Into<Vec<Json>>>(expression: &str, args: A) -> Self {
        Self {
            expression: expression.to_owned(),
            args: args.into(),
            objs: Vec::new(),
            and_then: None,
        }
    }

    /// Returns a new [`Statement`] with the provided JS code, arguments and
    /// objects.
    #[inline]
    #[must_use]
    pub fn with_objs<A: Into<Vec<Json>>, O: Into<Vec<ObjectPtr>>>(
        expression: &str,
        args: A,
        objs: O,
    ) -> Self {
        Self {
            expression: expression.to_owned(),
            args: args.into(),
            objs: objs.into(),
            and_then: None,
        }
    }

    /// Executes the `another` [`Statement`] after this one being executed
    /// successfully.
    ///
    /// The success value is passed to the next [`Statement`] as a JS lambda
    /// argument.
    #[allow(clippy::option_if_let_else)] // due to moving `another` value
    #[inline]
    #[must_use]
    pub fn and_then(mut self, another: Self) -> Self {
        self.and_then = Some(Box::new(if let Some(e) = self.and_then {
            e.and_then(another)
        } else {
            another
        }));
        self
    }

    /// Returns a JS code which should be executed in a browser and [`Json`]
    /// arguments for this code.
    #[must_use]
    pub(super) fn prepare(self) -> (String, Vec<Json>) {
        // language=JavaScript
        let mut final_js = r#"
            let lastResult;
            let objs;
            let args;
        "#
        .to_owned();

        let mut statement = Some(Box::new(self));
        let (mut i, mut args) = (0, Vec::new());
        while let Some(mut e) = statement.take() {
            final_js.push_str(&e.step_js(i));
            i += 1;
            args.push(mem::take(&mut e.args).into());
            statement = e.and_then;
        }

        (final_js, args)
    }

    /// Returns a JS code which obtains [`Statement::objs`] JS objects.
    ///
    /// Should be injected to the [`Statement::step_js`] code.
    fn objects_injection_js(&self) -> String {
        // language=JavaScript
        iter::once("objs = [];\n".to_owned())
            .chain(self.objs.iter().map(|id| {
                format!("objs.push(window.registry.get('{}'));\n", id)
            }))
            .collect()
    }

    /// Returns a JS code for this [`Statement`].
    ///
    /// Doesn't generates code for the [`Statement::and_then`].
    #[must_use]
    fn step_js(&self, i: usize) -> String {
        // language=JavaScript
        format!(
            r#"
                args = arguments[{i}];
                {objs_js}
                lastResult = await ({expr})(lastResult);
            "#,
            i = i,
            objs_js = self.objects_injection_js(),
            expr = self.expression,
        )
    }
}
