pub mod grpc_control_api;
mod signaling_with_grpc_control_api;
pub mod signalling;

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
