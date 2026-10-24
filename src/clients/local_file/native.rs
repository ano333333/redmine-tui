use std::path::Path;

use super::{LocalFileClient, LocalFileError};

pub struct NativeLocalFileClient;

impl LocalFileClient for NativeLocalFileClient {
    fn read_file(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, LocalFileError>> + Send {
        let path = path.to_path_buf();
        async move {
            tokio::fs::read(&path)
                .await
                .map_err(|error| LocalFileError {
                    path,
                    reason: error.to_string(),
                })
        }
    }
}
