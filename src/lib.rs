//! Browser-based JSON and YAML formatter, compiled to WebAssembly with Rust.
//!
//! The formatting rules are deliberately conservative: input is validated with a
//! real parser, then re-indented with four spaces per level. Everything else about
//! the document — key order, comments, anchors, quoting, numeric precision — is
//! preserved exactly as the author wrote it.
//!
//! * [`format::format`] is the mode-aware entry point used by the WebAssembly
//!   bridge and by the tests.
//! * [`json`] validates and indents JSON.
//! * [`yaml`] validates YAML and normalises indentation without re-serialising.

pub mod format;
pub mod json;
pub mod yaml;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// One indentation level used for both JSON and YAML output.
pub(crate) const INDENT: &str = "    ";

pub use format::{format, Format, Formatted};
