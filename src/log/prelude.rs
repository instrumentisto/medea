//! Re-exports common definitions for logging.
//!
//! Use this module as following:
//! ```rust
//! use crate::log::prelude::*;
//! ```

pub use slog::{slog_debug, slog_error, slog_info, slog_trace, slog_warn};
pub use slog_scope::{debug, error, info, trace, warn};
