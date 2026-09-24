use crate::vos::ProjectId;

#[derive(Clone)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
}
