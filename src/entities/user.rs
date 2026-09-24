use crate::vos::UserId;

#[derive(Clone)]
pub struct User {
    pub id: UserId,
    pub name: String,
}
