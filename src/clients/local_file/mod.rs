//! 添付ファイルとして送るローカルファイルの読み込みと、取得した添付ファイルの一時保存を、
//! platform固有の実装から分離する。

use std::fmt;
use std::path::{Component, Path, PathBuf};

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
    /// 一時ディレクトリに`file_name`で`content`を書き込み、書き込んだファイルのパスを返す。
    ///
    /// 同じ名前のファイルがあれば置き換える。`file_name`がパス区切りや`..`を含む場合は、
    /// 一時ディレクトリの外に書き込まないよう`LocalFileError`を返す。
    fn write_temp_file(
        &self,
        file_name: &str,
        content: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<PathBuf, LocalFileError>> + Send;
}

pub(crate) fn too_large(path: &Path, max_len: u64) -> LocalFileError {
    LocalFileError {
        path: path.to_path_buf(),
        reason: format!("the file exceeds the upload limit of {max_len} bytes"),
    }
}

/// `file_name`が、ディレクトリを含まない1つのファイル名かを返す。
pub(crate) fn is_plain_file_name(file_name: &str) -> bool {
    let mut components = Path::new(file_name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

pub(crate) fn invalid_file_name(file_name: &str) -> LocalFileError {
    LocalFileError {
        path: PathBuf::from(file_name),
        reason: "the temporary file name must not contain a directory".to_string(),
    }
}
