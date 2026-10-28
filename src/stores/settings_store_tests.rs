use ratatui_image::picker::Picker;

use super::{ImageDisplay, SettingsAction, Store, TerminalImageEncoder};

#[test]
fn image_display_is_disabled_initially() {
    let store = Store::new();

    assert!(matches!(store.get_image_display(), ImageDisplay::Disabled));
}

#[test]
fn set_image_display_replaces_image_display() {
    let mut store = Store::new();
    let encoder = TerminalImageEncoder::new(Picker::halfblocks());

    store.consume_action(SettingsAction::SetImageDisplay(ImageDisplay::Enabled { encoder }).into());

    assert!(matches!(
        store.get_image_display(),
        ImageDisplay::Enabled { .. }
    ));
}
