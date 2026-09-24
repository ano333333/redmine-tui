use crate::vos::PriorityId;

#[derive(Clone)]
pub struct Priority {
    pub id: PriorityId,
    pub name: String,
}
