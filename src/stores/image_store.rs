//! 表示する画像を、取得元を問わずキーごとに読み込み、表示用に前処理する進行状態。

use std::collections::HashMap;
use std::path::PathBuf;

use super::terminal_image::PreparedImage;
use crate::vos::AttachmentId;

/// 画像の取得元。同じ添付ファイルでも、サムネイルと原画像は別の画像として扱う。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageKey {
    AttachmentThumbnail { id: AttachmentId, size: u32 },
    AttachmentOriginal { id: AttachmentId },
}

/// 状態がない画像と`Failed`の画像だけが、読み込みを始められる。
///
/// ```text
///  None / Failed ── Start ──▶ Loading ── Loaded ──▶ Preparing ── Prepared ──▶ Ready
///                               │                      │
///                               └──────── Fail ────────┴──▶ Failed
/// ```
#[derive(Clone, Debug)]
pub enum ImageState {
    /// 画像のファイルを用意し、一時ファイルへ書き込んでいる。
    Loading,
    /// 一時ファイルを読み、表示用に前処理している。
    Preparing {
        path: PathBuf,
    },
    Ready(PreparedImage),
    Failed {
        reason: String,
    },
}

pub enum ImageAction {
    /// 状態があり`Failed`でない場合はpanicする。
    Start { key: ImageKey },
    /// Loading以外の場合はpanicする。
    Loaded { key: ImageKey, path: PathBuf },
    /// Preparing以外の場合はpanicする。
    Prepared { key: ImageKey, image: PreparedImage },
    /// LoadingとPreparing以外の場合はpanicする。
    Fail { key: ImageKey, reason: String },
}

pub(super) struct ImageStore {
    states: HashMap<ImageKey, ImageState>,
}

impl ImageStore {
    pub(super) fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: ImageAction) {
        match action {
            ImageAction::Start { key } => {
                assert!(
                    self.can_start(key),
                    "cannot start loading image {key:?} at {:?}",
                    self.states.get(&key)
                );
                self.states.insert(key, ImageState::Loading);
            }
            ImageAction::Loaded { key, path } => {
                let state = self.state_mut(key, "complete loading");
                assert!(
                    matches!(state, ImageState::Loading),
                    "cannot complete loading image {key:?} at {state:?}"
                );
                *state = ImageState::Preparing { path };
            }
            ImageAction::Prepared { key, image } => {
                let state = self.state_mut(key, "complete preparing");
                assert!(
                    matches!(state, ImageState::Preparing { .. }),
                    "cannot complete preparing image {key:?} at {state:?}"
                );
                *state = ImageState::Ready(image);
            }
            ImageAction::Fail { key, reason } => {
                let state = self.state_mut(key, "fail");
                assert!(
                    matches!(state, ImageState::Loading | ImageState::Preparing { .. }),
                    "cannot fail image {key:?} at {state:?}"
                );
                *state = ImageState::Failed { reason };
            }
        }
    }

    pub(super) fn try_get(&self, key: ImageKey) -> Option<&ImageState> {
        self.states.get(&key)
    }

    pub(super) fn can_start(&self, key: ImageKey) -> bool {
        matches!(
            self.states.get(&key),
            None | Some(ImageState::Failed { .. })
        )
    }

    fn state_mut(&mut self, key: ImageKey, operation: &str) -> &mut ImageState {
        self.states
            .get_mut(&key)
            .unwrap_or_else(|| panic!("cannot {operation} image {key:?} before starting it"))
    }
}
