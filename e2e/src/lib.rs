//! Tools for testing [Medea] medea client through a [WebDriver] protocol.
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [WebDriver]: https://w3.org/TR/webdriver

#![allow(clippy::module_name_repetitions)]
#![forbid(non_ascii_idents, unsafe_code)]

pub mod browser;
pub mod object;

pub use browser::{WebDriverClient, WebDriverClientBuilder};
