use std::path::PathBuf;

use super::{AttachmentImageAction, ImageAction, ImageKey, ImageState, PreparedImage, Store};
use crate::test_support::sample_prepared_image;
use crate::vos::AttachmentId;

const ATTACHMENT_ID: AttachmentId = AttachmentId::new(1);
const THUMBNAIL: ImageKey = ImageKey::AttachmentThumbnail {
    id: ATTACHMENT_ID,
    size: 800,
};
const ORIGINAL: ImageKey = ImageKey::AttachmentOriginal { id: ATTACHMENT_ID };

fn prepared_image() -> PreparedImage {
    sample_prepared_image(8, 16)
}

fn temp_path() -> PathBuf {
    PathBuf::from("/tmp/redmine-tui/1-thumbnail-800")
}

fn consume(store: &mut Store, action: impl Into<super::Action>) {
    store.consume_action(action.into());
}

fn start(store: &mut Store, key: ImageKey) {
    consume(
        store,
        AttachmentImageAction::Start {
            attachment_id: ATTACHMENT_ID,
            key,
        },
    );
}

fn fail(store: &mut Store, key: ImageKey, reason: &str) {
    consume(
        store,
        ImageAction::Fail {
            key,
            reason: reason.to_string(),
        },
    );
}

#[test]
fn image_has_no_state_before_loading_starts() {
    let store = Store::new();

    assert!(store.try_get_image(THUMBNAIL).is_none());
    assert_eq!(store.try_get_attachment_image_key(ATTACHMENT_ID), None);
}

#[test]
fn image_proceeds_from_loading_to_ready() {
    let mut store = Store::new();
    consume(&mut store, ImageAction::Start { key: THUMBNAIL });
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Loading)
    ));

    consume(
        &mut store,
        ImageAction::Loaded {
            key: THUMBNAIL,
            path: temp_path(),
        },
    );
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Preparing { path }) if *path == temp_path()
    ));

    consume(
        &mut store,
        ImageAction::Prepared {
            key: THUMBNAIL,
            image: prepared_image(),
        },
    );
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Ready(_))
    ));
}

#[test]
fn failure_keeps_the_reason_and_allows_starting_again() {
    let mut store = Store::new();
    consume(&mut store, ImageAction::Start { key: THUMBNAIL });

    fail(&mut store, THUMBNAIL, "write failed");
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Failed { reason }) if reason == "write failed"
    ));

    consume(&mut store, ImageAction::Start { key: THUMBNAIL });
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Loading)
    ));
}

#[test]
fn attachment_image_start_links_the_attachment_to_the_image() {
    let mut store = Store::new();

    start(&mut store, THUMBNAIL);

    assert_eq!(
        store.try_get_attachment_image_key(ATTACHMENT_ID),
        Some(THUMBNAIL)
    );
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Loading)
    ));
}

#[test]
fn fall_back_fails_the_thumbnail_and_loads_the_original() {
    let mut store = Store::new();
    start(&mut store, THUMBNAIL);

    consume(
        &mut store,
        AttachmentImageAction::FallBackToOriginal {
            attachment_id: ATTACHMENT_ID,
            reason: "not found".to_string(),
        },
    );

    assert_eq!(
        store.try_get_attachment_image_key(ATTACHMENT_ID),
        Some(ORIGINAL)
    );
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Failed { reason }) if reason == "not found"
    ));
    assert!(matches!(
        store.try_get_image(ORIGINAL),
        Some(ImageState::Loading)
    ));
}

#[test]
fn attachment_with_a_failed_image_starts_from_the_thumbnail_again() {
    let mut store = Store::new();
    start(&mut store, THUMBNAIL);
    consume(
        &mut store,
        AttachmentImageAction::FallBackToOriginal {
            attachment_id: ATTACHMENT_ID,
            reason: "not found".to_string(),
        },
    );
    fail(&mut store, ORIGINAL, "too large");

    start(&mut store, THUMBNAIL);

    assert_eq!(
        store.try_get_attachment_image_key(ATTACHMENT_ID),
        Some(THUMBNAIL)
    );
    assert!(matches!(
        store.try_get_image(THUMBNAIL),
        Some(ImageState::Loading)
    ));
}

#[test]
#[should_panic(expected = "cannot start loading image AttachmentThumbnail")]
fn starting_an_image_in_progress_panics() {
    let mut store = Store::new();
    consume(&mut store, ImageAction::Start { key: THUMBNAIL });

    consume(&mut store, ImageAction::Start { key: THUMBNAIL });
}

#[test]
#[should_panic(expected = "cannot complete preparing image AttachmentThumbnail")]
fn preparing_before_loading_completes_panics() {
    let mut store = Store::new();
    consume(&mut store, ImageAction::Start { key: THUMBNAIL });

    consume(
        &mut store,
        ImageAction::Prepared {
            key: THUMBNAIL,
            image: prepared_image(),
        },
    );
}

#[test]
#[should_panic(expected = "cannot start loading an image of attachment 1 while")]
fn starting_an_attachment_image_in_progress_panics() {
    let mut store = Store::new();
    start(&mut store, THUMBNAIL);

    start(&mut store, ORIGINAL);
}

#[test]
#[should_panic(expected = "cannot fall back to the original image of attachment 1 while it uses")]
fn falling_back_from_the_original_panics() {
    let mut store = Store::new();
    start(&mut store, ORIGINAL);

    consume(
        &mut store,
        AttachmentImageAction::FallBackToOriginal {
            attachment_id: ATTACHMENT_ID,
            reason: "not found".to_string(),
        },
    );
}
