use crate::vos::{CategoryId, ProjectId};

pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub project_id: ProjectId,
}
