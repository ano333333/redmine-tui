use crate::vos::IssueStatusId;

#[derive(Clone)]
pub struct IssueStatus {
    pub id: IssueStatusId,
    pub name: String,
    pub is_closed: bool,
}

pub trait IssueStatusExt {
    /// 未知statusを完了と誤認させないため、既知の完了状態だけを`true`として扱う。
    fn is_closed_status(&self) -> bool;
}

impl IssueStatusExt for Option<&IssueStatus> {
    fn is_closed_status(&self) -> bool {
        self.is_some_and(|issue_status| issue_status.is_closed)
    }
}
