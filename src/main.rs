#[cfg(all(feature = "native", feature = "web-demo"))]
compile_error!("features `native` and `web-demo` cannot be enabled together");

#[cfg(all(feature = "web-demo", not(target_arch = "wasm32")))]
compile_error!("feature `web-demo` requires a wasm32 target");

#[cfg(all(feature = "native", target_arch = "wasm32"))]
compile_error!("feature `native` cannot be enabled for a wasm32 target");

#[cfg(all(not(feature = "native"), not(feature = "web-demo")))]
compile_error!("either feature `native` or `web-demo` must be enabled");

mod clients;
mod components;
mod entities;
mod entry;
mod libs;
mod logging;
mod platform;
mod runner;
mod stores;
#[cfg(test)]
mod test_support;
mod usecases;
mod vos;
mod widgets;

#[cfg(feature = "native")]
fn main() -> std::process::ExitCode {
    entry::native::run()
}

#[cfg(not(feature = "native"))]
// Web entryを追加するまでは、native依存なしのbinaryを検査するための空entryとする。
fn main() {}
