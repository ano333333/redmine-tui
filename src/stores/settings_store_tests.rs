use super::{ImageDisplay, SettingsAction, Store};
use crate::test_support::test_image_encoder;

#[test]
fn image_display_is_disabled_initially() {
    let store = Store::new();

    assert!(matches!(store.get_image_display(), ImageDisplay::Disabled));
}

#[test]
fn set_image_display_replaces_image_display() {
    let mut store = Store::new();
    let encoder = test_image_encoder();

    store.consume_action(SettingsAction::SetImageDisplay(ImageDisplay::Enabled { encoder }).into());

    assert!(matches!(
        store.get_image_display(),
        ImageDisplay::Enabled { .. }
    ));
}
