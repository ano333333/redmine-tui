use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{HeaderWidget, TitleDecorater};
use ratatui::layout::Position;

use crate::platform::input::InputEvent;
use crate::stores::{IssueState, Store};
use crate::vos::IssueId;

pub struct HeaderComponent {
    id: IssueId,
    focus_state: FocusState,
}

impl HeaderComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
        }
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, store: &Store) {
        let widget = self.create_widget(store);
        self.focus_state
            .update(Position::new(widget.title_start_x(), 0));
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        let (issue, issue_state) = store.get_issue(self.id);
        let widget = HeaderWidget::new(
            self.id,
            &issue.issue.subject,
            self.focus_state.is_focused(),
            Self::title_decorator(&issue_state),
        );
        widget.line_count(width) as u16
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> HeaderWidget<'a> {
        let (issue, issue_status) = store.get_issue(self.id);
        HeaderWidget::new(
            self.id,
            &issue.issue.subject,
            self.focus_state.is_focused(),
            Self::title_decorator(&issue_status),
        )
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }

    fn title_decorator(issue_state: &IssueState) -> Option<TitleDecorater> {
        match issue_state {
            IssueState::Synced => None,
            IssueState::Edited => Some(TitleDecorater::Edited),
            IssueState::Uploading => Some(TitleDecorater::Uploading),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{IssueAction, Store};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn uploading_issue_renders_uploading_decorator() {
        let mut store = Store::new();
        store.consume_action(IssueAction::Load { id: 1.into() }.into());
        store.consume_action(
            IssueAction::UpdateDescription {
                id: 1.into(),
                body: "edited body".to_string(),
            }
            .into(),
        );
        store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

        let component = HeaderComponent::new(1);
        let widget = component.create_widget(&store);

        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        terminal
            .draw(|frame| widget.render(frame.area(), frame.buffer_mut()))
            .unwrap();
        // コンパクトヘッダーはID・マーカー・タイトルを先頭行にまとめる
        let title_line = (0..40)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();

        // IDバッジ・送信中マーカー・タイトルが先頭行にこの順で並ぶ
        // (全角文字はセル単位で見ると後ろに空セルが続くため、順序だけを検証する)
        let id_at = title_line.find("#1").expect("id badge");
        let marker_at = title_line.find('↑').expect("uploading marker");
        let title_at = title_line.find("issue1").expect("title");
        assert!(
            id_at < marker_at && marker_at < title_at,
            "got: {title_line}"
        );
    }

    #[test]
    fn cursor_position_points_to_synced_issue_title() {
        let mut store = Store::new();
        store.consume_action(IssueAction::Load { id: 1.into() }.into());

        let mut component = HeaderComponent::new(1);
        component.update(&store);

        assert_eq!(component.get_cursor_position(), Position::new(4, 0));
    }

    #[test]
    fn cursor_position_accounts_for_uploading_decorator() {
        let mut store = Store::new();
        store.consume_action(IssueAction::Load { id: 1.into() }.into());
        store.consume_action(
            IssueAction::UpdateDescription {
                id: 1.into(),
                body: "edited body".to_string(),
            }
            .into(),
        );
        store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

        let mut component = HeaderComponent::new(1);
        component.update(&store);

        assert_eq!(component.get_cursor_position(), Position::new(12, 0));
    }
}
