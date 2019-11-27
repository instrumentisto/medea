macro_rules! debug {
    ($($arg:expr),+) => {
        web_sys::console::debug_1(
            &format!(
                "[{}:{}] {}", module_path!(), line!(), format!($($arg),+)
            ).into()
        );
    };
}
