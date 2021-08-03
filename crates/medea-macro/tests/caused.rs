#![forbid(non_ascii_idents, unsafe_code)]

use medea_jason::utils::Caused;

struct JsError {}

#[test]
fn derives_for_structure() {
    #[derive(Caused)]
    #[cause(error = "JsError")]
    struct TestError;

    let err = TestError;
    assert!(err.cause().is_none());
}

#[test]
fn derives_for_enum_with_js_error() {
    #[derive(Caused)]
    #[cause(error = "JsError")]
    enum TestError {
        Foo,
        Bar(JsError),
    }

    let err = TestError::Foo;
    assert!(err.cause().is_none());

    let err = TestError::Bar(JsError {});
    assert!(err.cause().is_some());
}

#[test]
fn derives_for_enum_with_nested_js_error() {
    #[derive(Caused)]
    #[cause(error = "JsError")]
    enum CausedError {
        Baz(JsError),
    }

    #[derive(Caused)]
    #[cause(error = "JsError")]
    enum TestError {
        Foo,
        Bar(#[cause] CausedError),
    }

    let cause = CausedError::Baz(JsError {});

    let err = TestError::Foo;
    assert!(err.cause().is_none());

    let err = TestError::Bar(cause);
    assert!(err.cause().is_some());
}

#[test]
fn derives_for_non_default_name_js_error() {
    struct SomeError;

    #[derive(Caused)]
    #[cause(error = "SomeError")]
    enum CausedError {
        Baz(SomeError),
    }

    #[derive(Caused)]
    #[cause(error = "SomeError")]
    enum TestError {
        Foo,
        Bar(#[cause] CausedError),
    }

    let cause = CausedError::Baz(SomeError {});

    let err = TestError::Foo;
    assert!(err.cause().is_none());

    let err = TestError::Bar(cause);
    assert!(err.cause().is_some());
}
