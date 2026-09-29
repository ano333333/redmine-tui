//! PTY上でnativeバイナリを起動し、`cargo xtask test-e2e`が用意したRedmine containerへ接続して操作する。

mod issue_fetch;
mod issue_select_popup;
mod startup;
mod support;
