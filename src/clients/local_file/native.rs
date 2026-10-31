use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::{LocalFileClient, LocalFileError, invalid_file_name, is_plain_file_name, too_large};

/// プロセスごとの一時ディレクトリ。破棄するときに中身ごと削除する。
pub struct NativeTempDir {
    path: PathBuf,
}

impl NativeTempDir {
    /// `std::env::temp_dir()`の下に、所有者だけが読み書きできるディレクトリを作る。
    pub fn create() -> io::Result<Self> {
        // 他のユーザーが同じ名前を先に作っていても使わないよう、推測できない名前で新規に作る。
        let path = std::env::temp_dir().join(format!(
            "redmine-tui-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for NativeTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub struct NativeLocalFileClient {
    temp_dir: PathBuf,
}

impl NativeLocalFileClient {
    pub fn new(temp_dir: &NativeTempDir) -> Self {
        Self {
            temp_dir: temp_dir.path().to_path_buf(),
        }
    }
}

impl LocalFileClient for NativeLocalFileClient {
    fn read_file(
        &self,
        path: &Path,
        max_len: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, LocalFileError>> + Send {
        let path = path.to_path_buf();
        async move {
            let io_error = |error: std::io::Error| LocalFileError {
                path: path.clone(),
                reason: error.to_string(),
            };
            let file = tokio::fs::File::open(&path).await.map_err(io_error)?;
            if file.metadata().await.map_err(io_error)?.len() > max_len {
                return Err(too_large(&path, max_len));
            }
            // 開いた後にファイルが伸びても上限を超えて確保しないよう、1バイト多く読んで超過を判定する。
            let mut content = Vec::new();
            file.take(max_len + 1)
                .read_to_end(&mut content)
                .await
                .map_err(io_error)?;
            if content.len() as u64 > max_len {
                return Err(too_large(&path, max_len));
            }
            Ok(content)
        }
    }

    fn write_temp_file(
        &self,
        file_name: &str,
        content: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<PathBuf, LocalFileError>> + Send {
        let path = is_plain_file_name(file_name)
            .then(|| self.temp_dir.join(file_name))
            .ok_or_else(|| invalid_file_name(file_name));
        async move {
            let path = path?;
            tokio::fs::write(&path, content)
                .await
                .map_err(|error| LocalFileError {
                    path: path.clone(),
                    reason: error.to_string(),
                })?;
            Ok(path)
        }
    }
}
