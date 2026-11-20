use uuid::Uuid;

/// 新規作成用Componentごとに割り振り、作成の要求とその結果のイベントを対応付ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueCreationId(Uuid);

impl IssueCreationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
