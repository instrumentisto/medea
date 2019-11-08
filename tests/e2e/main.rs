pub mod signalling;
mod callbacks;
mod grpc_control_api;

/// Generates `insert_str!` macro, that can be used as `format!` macro with one
/// predefined argument.
///
/// # Example usage
///
/// ```
/// fn first_test() {
///     gen_insert_str_macro!("first-test");
///
///     let addr = insert_str!("ws://127.0.0.1:8080/{}/publisher/test");
///     assert_eq!(addr, "ws://127.0.0.1:8080/first-test/publisher/test");
/// }
///
/// fn second_test() {
///     gen_insert_str_macro!("second-test");
///
///     let addr = insert_str!("local://{}/publisher");
///     assert_eq!(addr, "local://second-test/publisher");
/// }
///
/// # first_test();
/// # second_test();
/// ```
#[macro_export]
macro_rules! gen_insert_str_macro {
    ($name:expr) => {
        macro_rules! insert_str {
            ($fmt:expr) => {
                format!($fmt, $name)
            };
        }
    };
}
