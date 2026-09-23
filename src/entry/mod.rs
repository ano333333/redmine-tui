//! native版とWeb版の起動処理を分けるentry point。

#[cfg(feature = "native")]
pub(crate) mod native;

#[cfg(feature = "web-demo")]
pub(crate) mod web;
