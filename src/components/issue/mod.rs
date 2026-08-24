mod component;
mod detail;
mod widget;

pub use component::{EventProcessResult, IssueComponent};
pub(crate) use detail::{
    EventProcessResult as IssueDetailEventProcessResult, IssueDetailComponent,
};
pub(crate) use widget::IssueWidget;
