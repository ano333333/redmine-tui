use super::IssueStatusId;

pub struct IssueStatus {
    pub id: IssueStatusId,
    pub name: String,
    pub is_closed: bool,
}
