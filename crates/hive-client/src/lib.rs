//! Shared HTTP client for hive CLI and panel.

mod client;
mod encode;
mod endpoints;
mod error;
mod http;

pub use client::Client;
pub use endpoints::Endpoints;
pub use error::Error;
