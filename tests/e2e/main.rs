mod grpc_control_api;
pub mod signalling;

/// Macro which generates `format_name!` macro which will replaces `{}` with
/// provided to `format_name_macro!` name.
///
/// # Example usage
///
/// ```
/// fn first_test() {
///     format_name_macro!("first-test");
///
///     let addr = format_name!("ws://127.0.0.1:8080/{}/publisher/test");
///     assert_eq!(addr, "ws://127.0.0.1:8080/first-test/publisher/test");
/// }
///
/// fn second_test() {
///     format_name_macro!("second-test");
///
///     let addr = format_name!("local://{}/publisher");
///     assert_eq!(addr, "local://second-test/publisher");
/// }
///
/// # first_test();
/// # second_test();
/// ```
#[macro_export]
macro_rules! format_name_macro {
    ($name:expr) => {
        macro_rules! format_name {
            ($fmt:expr) => {
                format!($fmt, $name)
            };
        }
    };
}
