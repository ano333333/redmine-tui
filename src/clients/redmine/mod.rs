pub mod base;
#[cfg(feature = "native")]
pub mod default;
#[cfg(any(feature = "web-demo", test))]
pub mod demo;

pub use base::{IssueUpdate, RedmineClient, RedmineClientError, RedmineHttpError};
#[cfg(feature = "native")]
pub use default::DefaultRedmineClient;
