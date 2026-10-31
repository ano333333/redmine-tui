use super::terminal_image::TerminalImageEncoder;

/// 画像表示の可否。利用者の設定とterminalの対応状況の両方を満たすときだけ`Enabled`になる。
#[derive(Clone, Debug)]
pub enum ImageDisplay {
    Disabled,
    Enabled { encoder: TerminalImageEncoder },
}

pub enum SettingsAction {
    SetImageDisplay(ImageDisplay),
}

pub(super) struct SettingsStore {
    image_display: ImageDisplay,
}

impl SettingsStore {
    pub(super) fn new() -> Self {
        Self {
            image_display: ImageDisplay::Disabled,
        }
    }

    pub(super) fn consume_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::SetImageDisplay(image_display) => self.image_display = image_display,
        }
    }

    pub(super) fn image_display(&self) -> &ImageDisplay {
        &self.image_display
    }
}
