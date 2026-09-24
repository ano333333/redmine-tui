use crate::vos::TrackerId;

#[derive(Clone)]
pub struct Tracker {
    pub id: TrackerId,
    pub name: String,
}
