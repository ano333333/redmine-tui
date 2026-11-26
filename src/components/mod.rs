//! TUIの画面部品。
//!
//! FocusStateとComponentの`process_event`は`Option<EventProcessResult>`を返す。`None`は
//! 入力を解釈しなかったこと、`Some(Handled)`は入力を使い親への要求がないことを表し、それ以外は
//! 親への要求である。端で動けなかった場合も、解釈したキーなら`Handled`を返す。子の
//! `CursorLeavedFrom*`を受けた親は、隣の子へfocusを移せたら`Handled`を返し、移せなければ外へ返す。
//! ただし、画面の最上位のComponent（`IssueDetailComponent`）は外へ返さず`Handled`にする。

pub mod app;
pub mod attachment_input_popup;
pub mod date_picker_popup;
pub mod issue;
#[allow(dead_code)]
pub mod issue_property_conflict_popup;
pub mod issue_select_popup;
pub mod new_issue;
pub mod number_input_popup;
pub mod remote_journal_conflict_popup;
pub mod request_sink;
pub mod select_box_popup;
pub mod spent_time_input_popup;
pub mod toast_list;

pub use app::AppComponent;
pub use request_sink::RequestSink;

#[cfg(test)]
mod module_path_tests {
    use super::issue::IssueDetailComponent;

    #[test]
    fn issue_detail_component_is_exposed_from_issue_detail_module() {
        fn accepts_issue_detail_component(_: Option<IssueDetailComponent>) {}

        accepts_issue_detail_component(None);
    }
}
