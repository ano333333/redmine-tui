use std::path::Path;

use tokio::io::AsyncReadExt;

use super::{LocalFileClient, LocalFileError, too_large};

pub struct NativeLocalFileClient;

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
}
