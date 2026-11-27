use std::path::Path;

use crate::clients::local_file::{LocalFileClient, LocalFileError};
use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::usecases::UsecaseClients;
use crate::vos::UploadToken;

/// Redmineの添付ファイルサイズ上限の設定`attachment_max_size`の既定値（5120 KB）。
///
/// RedmineのREST APIは設定値を返さないため、既定値に合わせる。サーバーが上限を下げている場合は
/// Redmineが送信を422で拒否し、上げている場合はこの値を超えるファイルを送れない。
const ATTACHMENT_MAX_BYTES: u64 = 5120 * 1024;

pub(super) enum UploadLocalFileError {
    Read(LocalFileError),
    Upload(RedmineClientError),
}

pub(super) async fn upload_local_file<C, F>(
    clients: &UsecaseClients<C, F>,
    filename: &str,
    path: &Path,
) -> Result<UploadToken, UploadLocalFileError>
where
    C: RedmineClient,
    F: LocalFileClient,
{
    let content = clients
        .local_file
        .read_file(path, ATTACHMENT_MAX_BYTES)
        .await
        .map_err(UploadLocalFileError::Read)?;
    clients
        .redmine
        .upload_file(filename, content)
        .await
        .map_err(UploadLocalFileError::Upload)
}
