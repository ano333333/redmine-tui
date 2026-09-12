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
    /// 通知の表示期限を計算する基準時刻。
    pub created_at: chrono::DateTime<chrono::Local>,
}

/// 一時通知の追加と破棄を表すAction。
// Journal保存などの失敗発生源と手動破棄操作が接続されると各variantがproduction codeから使われる。
pub enum NoticeAction {
    /// 指定したIDが未登録の場合だけ通知を追加する。
    Push {
        id: NoticeId,
        message: String,
        created_at: chrono::DateTime<chrono::Local>,
    },
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
            NoticeAction::Push {
                id,
                message,
                created_at,
            } => {
                // 非同期処理から同じ完了通知が複数回届いても表示を重複させない。
                if self.notices.iter().any(|notice| notice.id == id) {
                    return;
                }
                self.notices.push(Notice {
                    id,
                    message,
                    created_at,
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

    pub(super) fn update(&mut self, now: chrono::DateTime<chrono::Local>) {
        // Store内で現在時刻を取得せず、呼び出し側の1時点を全Storeで共有できるようにする。
        self.notices
            .retain(|notice| notice.created_at + NOTICE_VISIBLE_FOR > now);
    }
}
