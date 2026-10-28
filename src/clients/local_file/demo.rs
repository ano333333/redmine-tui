use std::path::{Path, PathBuf};

use super::{LocalFileClient, LocalFileError, too_large};

/// Webデモで添付するサンプル画像。wasmを小さくするため、seedの画像を縮小したものを使う。
pub const SAMPLE_IMAGE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/datas/attachment_files/sample_web.png"
));

/// ブラウザからローカルのパスでファイルを読めないため、どのパスにもサンプル画像を返す。
pub struct DemoLocalFileClient;

impl LocalFileClient for DemoLocalFileClient {
    fn read_file(
        &self,
        path: &Path,
        max_len: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, LocalFileError>> + Send {
        let result = if SAMPLE_IMAGE.len() as u64 > max_len {
            Err(too_large(path, max_len))
        } else {
            Ok(SAMPLE_IMAGE.to_vec())
        };
        async move { result }
    }

    // Webデモは画像を表示しないため、取得した添付ファイルを保存しない。
    fn write_temp_file(
        &self,
        file_name: &str,
        _: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<PathBuf, LocalFileError>> + Send {
        let error = LocalFileError {
            path: PathBuf::from(file_name),
            reason: "the web demo does not write temporary files".to_string(),
        };
        async move { Err(error) }
    }
}
