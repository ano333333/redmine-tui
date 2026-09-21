use uuid::Uuid;

pub(super) const NOTICE_VISIBLE_FOR: chrono::Duration = chrono::Duration::seconds(5);

/// 一時通知を個別に識別するID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoticeId(Uuid);

impl NoticeId {
    /// 新しいIDを生成する。
    // Journal保存などの失敗発生源が通知へ接続されるとproduction codeから使われる。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for NoticeId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

/// 表示中の一時通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// 通知の重複判定と個別破棄に使うID。
    pub id: NoticeId,
    /// ユーザーへ表示するメッセージ。
    pub message: String,
    pub(super) elapsed: chrono::Duration,
}

impl Notice {
    /// 表示期限内かどうかを返す。累積経過時間が期限ちょうどの場合は表示終了とみなす。
    pub(super) fn is_visible(&self) -> bool {
        self.elapsed < NOTICE_VISIBLE_FOR
    }
}

/// 一時通知の追加と破棄を表すAction。
// Journal保存などの失敗発生源と手動破棄操作が接続されると各variantがproduction codeから使われる。
pub enum NoticeAction {
    /// 指定したIDが未登録の場合だけ通知を追加する。
    Push { id: NoticeId, message: String },
    /// 指定したIDの通知を破棄する。未登録のIDは正常なno-opとして扱う。
    Dismiss { id: NoticeId },
}

pub(super) struct NoticeStore {
    notices: Vec<Notice>,
}

impl NoticeStore {
    pub(super) fn new() -> Self {
        Self {
            notices: Vec::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: NoticeAction) {
        match action {
            NoticeAction::Push { id, message } => {
                // 非同期処理から同じ完了通知が複数回届いても表示を重複させない。
                if self.notices.iter().any(|notice| notice.id == id) {
                    return;
                }
                self.notices.push(Notice {
                    id,
                    message,
                    elapsed: chrono::Duration::zero(),
                });
            }
            NoticeAction::Dismiss { id } => {
                // 手動破棄と期限切れ処理は競合し得るため、すでに消えたIDも受理する。
                self.notices.retain(|notice| notice.id != id);
            }
        }
    }

    pub(super) fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub(super) fn update(&mut self, tick: chrono::Duration) {
        self.notices.iter_mut().for_each(|notice| {
            notice.elapsed += tick;
        });
        self.notices.retain(Notice::is_visible);
    }
}
