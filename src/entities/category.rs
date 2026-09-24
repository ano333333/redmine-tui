use crate::vos::{CategoryId, ProjectId};

#[derive(Clone)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub project_id: ProjectId,
}
