//! 取得済みIssueの詳細画面。
//!
//! `IssueComponent`はIssueが取得済みのときだけこの画面を作るため、配下のComponentは
//! `create_widget`の時点で対象のIssueがStoreにあることを前提に`get_issue`を使う。

pub mod attachment_image;
pub mod attachments_list;
pub mod body;
pub mod children_list;
pub mod component;
pub mod header;
pub mod journals_list;
pub mod markdown_view;
pub mod property;
pub mod widget;

pub use component::{EventProcessResult, IssueDetailComponent};
pub use widget::{IssueDetailWidget, IssueDetailWidgetState};
