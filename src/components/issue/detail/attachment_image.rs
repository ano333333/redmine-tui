//! 本文やnotesの画像の参照先を、Issueの添付ファイルと読み込んだ画像に解決する。

use crate::entities::Attachment;
use crate::stores::{ImageState, PreparedImage, Store};

/// 参照先の画像の表示状態。
#[derive(Clone, Debug)]
pub enum AttachmentImageView {
    Ready(PreparedImage),
    /// 画像を取得または前処理している。
    Loading,
    Failed,
    /// 対応する添付ファイルがないか、画像を読み込んでいない。
    Unavailable,
}

/// `dest`をIssueの添付ファイルのファイル名と照合し、その添付ファイルの表示に使う画像の状態を返す。
///
/// Redmineと同じく、ファイル名は大文字小文字を区別せずに照合し、同名なら後に添付したものを使う。
/// パスやURLを含む参照先は添付ファイルとして扱わない。
pub fn resolve_attachment_image(
    store: &Store,
    attachments: &[Attachment],
    dest: &str,
) -> AttachmentImageView {
    if dest.contains('/') {
        return AttachmentImageView::Unavailable;
    }
    let Some(attachment) = attachments
        .iter()
        .filter(|attachment| attachment.filename.eq_ignore_ascii_case(dest))
        .max_by_key(|attachment| (attachment.created_on, attachment.id))
    else {
        return AttachmentImageView::Unavailable;
    };
    let state = store
        .try_get_attachment_image_key(attachment.id)
        .and_then(|key| store.try_get_image(key));
    match state {
        Some(ImageState::Ready(image)) => AttachmentImageView::Ready(image.clone()),
        Some(ImageState::Loading | ImageState::Preparing { .. }) => AttachmentImageView::Loading,
        Some(ImageState::Failed { .. }) => AttachmentImageView::Failed,
        None => AttachmentImageView::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::stores::{AttachmentImageAction, ImageAction, ImageKey};
    use crate::test_support::{sample_attachment, sample_prepared_image};
    use crate::vos::AttachmentId;

    fn original(id: u16) -> ImageKey {
        ImageKey::AttachmentOriginal {
            id: AttachmentId::new(id),
        }
    }

    fn start(store: &mut Store, id: u16) {
        store.consume_action(
            AttachmentImageAction::Start {
                attachment_id: AttachmentId::new(id),
                key: original(id),
            }
            .into(),
        );
    }

    fn load(store: &mut Store, id: u16) {
        store.consume_action(
            ImageAction::Loaded {
                key: original(id),
                path: PathBuf::from("/tmp/image"),
            }
            .into(),
        );
    }

    fn named(id: u16, filename: &str) -> Attachment {
        let mut attachment = sample_attachment(id, Some("image/png"), false);
        attachment.filename = filename.to_string();
        attachment
    }

    #[test]
    fn ready_image_of_the_named_attachment_is_returned() {
        let mut store = Store::new();
        start(&mut store, 1);
        load(&mut store, 1);
        store.consume_action(
            ImageAction::Prepared {
                key: original(1),
                image: sample_prepared_image(10, 20),
            }
            .into(),
        );

        let view = resolve_attachment_image(&store, &[named(1, "Sample.PNG")], "sample.png");

        assert!(
            matches!(view, AttachmentImageView::Ready(image) if image.pixel_size() == (10, 20))
        );
    }

    #[test]
    fn loading_and_preparing_images_are_loading() {
        let mut store = Store::new();
        start(&mut store, 1);
        assert!(matches!(
            resolve_attachment_image(&store, &[named(1, "a.png")], "a.png"),
            AttachmentImageView::Loading
        ));

        load(&mut store, 1);
        assert!(matches!(
            resolve_attachment_image(&store, &[named(1, "a.png")], "a.png"),
            AttachmentImageView::Loading
        ));
    }

    #[test]
    fn failed_image_is_failed() {
        let mut store = Store::new();
        start(&mut store, 1);
        store.consume_action(
            ImageAction::Fail {
                key: original(1),
                reason: "not found".to_string(),
            }
            .into(),
        );

        assert!(matches!(
            resolve_attachment_image(&store, &[named(1, "a.png")], "a.png"),
            AttachmentImageView::Failed
        ));
    }

    #[test]
    fn the_later_attachment_of_the_same_name_is_used() {
        let mut store = Store::new();
        start(&mut store, 2);
        let mut older = named(1, "a.png");
        older.created_on = crate::test_support::local_datetime("2026-01-01T00:00:00+09:00");

        assert!(matches!(
            resolve_attachment_image(&store, &[named(2, "a.png"), older], "a.png"),
            AttachmentImageView::Loading
        ));
    }

    #[test]
    fn unknown_names_paths_and_unloaded_images_are_unavailable() {
        let mut store = Store::new();
        start(&mut store, 1);
        let attachments = [named(1, "a.png"), named(2, "b.png")];

        for dest in ["c.png", "attachments/download/1/a.png", "b.png"] {
            assert!(
                matches!(
                    resolve_attachment_image(&store, &attachments, dest),
                    AttachmentImageView::Unavailable
                ),
                "{dest}"
            );
        }
    }
}
