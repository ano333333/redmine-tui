pub mod base;
#[cfg(feature = "native")]
pub mod default;
#[cfg(any(feature = "web-demo", test))]
pub mod demo;
#[cfg(test)]
pub mod fake;

pub use base::{IssueUpdate, RedmineClient, RedmineClientError, RedmineHttpError};
#[cfg(feature = "native")]
pub use default::DefaultRedmineClient;
