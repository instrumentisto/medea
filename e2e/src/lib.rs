//! Tools for testing [Medea] client ([Jason]) through a [WebDriver] protocol.
//!
//! [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
//! [Medea]: https://github.com/instrumentisto/medea
//! [WebDriver]: https://w3.org/TR/webdriver

#![forbid(non_ascii_idents, unsafe_code)]

pub mod browser;
pub mod object;

pub use browser::{WebDriverClient, WebDriverClientBuilder};
