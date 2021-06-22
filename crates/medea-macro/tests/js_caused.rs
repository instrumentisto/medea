#![forbid(non_ascii_idents, unsafe_code)]

use medea_jason::utils::JsCaused;

struct JsError {}

#[test]
fn derives_for_structure() {
    #[derive(JsCaused)]
    struct TestError;

    let err = TestError;
    assert_eq!(err.name(), "TestError");
    assert!(err.js_cause().is_none());
}

#[test]
fn derives_for_enum_with_js_error() {
    #[derive(JsCaused)]
    enum TestError {
        Foo,
        Bar(JsError),
    }

    let err = TestError::Foo;
    assert_eq!(err.name(), "Foo");
    assert!(err.js_cause().is_none());

    let err = TestError::Bar(JsError {});
    assert_eq!(err.name(), "Bar");
    assert!(err.js_cause().is_some());
}

#[test]
fn derives_for_enum_with_nested_js_error() {
    #[derive(JsCaused)]
    enum CausedError {
        Baz(JsError),
    }

    #[derive(JsCaused)]
    enum TestError {
        Foo,
        Bar(#[js(cause)] CausedError),
    }

    let cause = CausedError::Baz(JsError {});

    let err = TestError::Foo;
    assert_eq!(err.name(), "Foo");
    assert!(err.js_cause().is_none());

    let err = TestError::Bar(cause);
    assert_eq!(err.name(), "Bar");
    assert!(err.js_cause().is_some());
}

#[test]
fn derives_for_non_default_name_js_error() {
    struct SomeError;

    #[derive(JsCaused)]
    #[js(error = "SomeError")]
    enum CausedError {
        Baz(SomeError),
    }

    #[derive(JsCaused)]
    #[js(error = "SomeError")]
    enum TestError {
        Foo,
        Bar(#[js(cause)] CausedError),
    }

    let cause = CausedError::Baz(SomeError {});

    let err = TestError::Foo;
    assert_eq!(err.name(), "Foo");
    assert!(err.js_cause().is_none());

    let err = TestError::Bar(cause);
    assert_eq!(err.name(), "Bar");
    assert!(err.js_cause().is_some());
}
