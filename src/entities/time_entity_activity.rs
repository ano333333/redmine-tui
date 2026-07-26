use crate::vos::TimeEntityActivityId;

pub struct TimeEntityActivity {
    pub id: TimeEntityActivityId,
    pub name: String,
    pub is_default: bool,
}

impl TimeEntityActivity {
    pub fn new(id: TimeEntityActivityId, name: impl Into<String>, is_default: bool) -> Self {
        Self {
            id,
            name: name.into(),
            is_default,
        }
    }
}
