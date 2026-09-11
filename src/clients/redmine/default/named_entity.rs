//! Redmineの参照データエンドポイントに共通する`{ id, name }`形式をデシリアライズする。

use serde::Deserialize;

use crate::entities::{Category, Priority, Project, TargetVersion, Tracker};
use crate::vos::{CategoryId, PriorityId, ProjectId, TargetVersionId, TrackerId};

/// レスポンスラッパーが選択したドメイン型へ変換するための中間DTO。
#[derive(Deserialize)]
pub(super) struct NamedRedmineEntity {
    id: u16,
    name: String,
}

impl From<NamedRedmineEntity> for Category {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: CategoryId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Priority {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: PriorityId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Project {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: ProjectId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for TargetVersion {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: TargetVersionId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Tracker {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: TrackerId::new(value.id),
            name: value.name,
        }
    }
}
