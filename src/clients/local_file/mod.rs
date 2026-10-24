//! 添付ファイルとして送るローカルファイルの読み込みを、platform固有の実装から分離する。

use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(any(feature = "web-demo", test))]
pub mod demo;
#[cfg(feature = "native")]
pub mod native;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFileError {
    pub path: PathBuf,
    pub reason: String,
}

impl fmt::Display for LocalFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to read {}: {}", self.path.display(), self.reason)
    }
}

/// 各メソッドのFutureはbackground taskとして実行するため`Send`を契約とする。
pub trait LocalFileClient {
    /// ファイルが`max_len`バイトを超える場合は、全体をメモリへ読み込まずに`LocalFileError`を返す。
    fn read_file(
        &self,
        path: &Path,
        max_len: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, LocalFileError>> + Send;
}

pub(crate) fn too_large(path: &Path, max_len: u64) -> LocalFileError {
    LocalFileError {
        path: path.to_path_buf(),
        reason: format!("the file exceeds the upload limit of {max_len} bytes"),
    }
}
