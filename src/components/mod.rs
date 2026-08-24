pub mod app;
pub mod date_picker_popup;
pub mod issue;
#[allow(dead_code)]
pub mod issue_property_conflict_popup;
pub mod issue_select_popup;
pub mod select_box_popup;
pub mod spent_time_input_popup;

pub use app::AppComponent;

#[cfg(test)]
mod module_path_tests {
    use super::issue::IssueDetailComponent;

    #[test]
    fn issue_detail_component_is_exposed_from_issue_detail_module() {
        fn accepts_issue_detail_component(_: Option<IssueDetailComponent>) {}

        accepts_issue_detail_component(None);
    }
}
