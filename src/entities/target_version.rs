use crate::vos::{ProjectId, TargetVersionId};

#[derive(Clone)]
pub struct TargetVersion {
    pub id: TargetVersionId,
    pub name: String,
    pub project_id: ProjectId,
}
