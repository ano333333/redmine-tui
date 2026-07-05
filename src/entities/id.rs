use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub trait EntityIdValue: Copy + Eq {
    fn get(self) -> u16;

    fn same_as(self, other: Self) -> bool {
        self == other
    }
}

#[derive(Debug)]
pub struct EntityId<Tag> {
    value: u16,
    _tag: PhantomData<Tag>,
}

impl<Tag> EntityId<Tag> {
    pub const fn new(value: u16) -> Self {
        Self {
            value,
            _tag: PhantomData,
        }
    }
}

impl<Tag> EntityIdValue for EntityId<Tag> {
    fn get(self) -> u16 {
        self.value
    }
}

impl<Tag> Clone for EntityId<Tag> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Tag> Copy for EntityId<Tag> {}

impl<Tag> PartialEq for EntityId<Tag> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<Tag> Eq for EntityId<Tag> {}

impl<Tag> PartialOrd for EntityId<Tag> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tag> Ord for EntityId<Tag> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<Tag> Hash for EntityId<Tag> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<Tag> From<u16> for EntityId<Tag> {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}

impl<Tag> From<EntityId<Tag>> for u16 {
    fn from(value: EntityId<Tag>) -> Self {
        value.value
    }
}

impl<Tag> PartialEq<u16> for EntityId<Tag> {
    fn eq(&self, other: &u16) -> bool {
        self.value == *other
    }
}

impl<Tag> fmt::Display for EntityId<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IssueTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IssueStatusTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JournalTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PriorityTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrackerTag {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UserTag {}

pub type IssueId = EntityId<IssueTag>;
pub type IssueStatusId = EntityId<IssueStatusTag>;
pub type JournalId = EntityId<JournalTag>;
pub type PriorityId = EntityId<PriorityTag>;
pub type ProjectId = EntityId<ProjectTag>;
pub type TrackerId = EntityId<TrackerTag>;
pub type UserId = EntityId<UserTag>;
