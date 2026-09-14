use crossterm::event::Event;
use ratatui::layout::{Position, Rect, Size};

use crate::vos::JournalNotesDiff;
use crate::widgets::{VerticalScrollWidget, VerticalScrollWidgetState};

use super::focus_state::{EventProcessResult as RawEventProcessResult, FocusState, FocusTarget};
use super::widget::{RemoteJournalConflictChoice, RemoteJournalConflictWidget};

/// Remote Journal競合popupのキー操作結果。
pub enum EventProcessResult {
    /// 競合解決を中止する。
    Canceled,
    /// 選択した本文でuploadを再試行する。
    Continued { resolved_notes: String },
}

/// Remote Journal本文の競合解決popupを管理するComponent。
pub struct RemoteJournalConflictComponent {
    diff: JournalNotesDiff,
    server_notes: String,
    selected_choice: RemoteJournalConflictChoice,
    focus_state: FocusState,
    vertical_scroll_state: VerticalScrollWidgetState,
}

impl RemoteJournalConflictComponent {
    /// 編集前、ローカル編集後、サーバー現在値からComponentを作成する。
    ///
    /// 初期状態ではローカル編集後の本文を採用値として選択する。
    pub fn new(diff: JournalNotesDiff, server_notes: String) -> Self {
        Self {
            diff,
            server_notes,
            selected_choice: RemoteJournalConflictChoice::Local,
            focus_state: FocusState::new(3, 1),
            vertical_scroll_state: VerticalScrollWidgetState::new(),
        }
    }

    /// 描画領域に合わせて、現在のフォーカスが見えるようスクロール状態を更新する。
    pub fn update(&mut self, area: Rect) {
        let cursor = self.cursor_global_position(area.width);
        self.vertical_scroll_state.update(cursor, area.height);
    }

    /// キーイベントを処理し、popupの終了やupload再試行が必要な場合は結果を返す。
    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .and_then(|result| match result {
                RawEventProcessResult::Selected(index) => {
                    self.selected_choice = self.choice_at(index);
                    None
                }
                RawEventProcessResult::Canceled => Some(EventProcessResult::Canceled),
                RawEventProcessResult::Continued => Some(EventProcessResult::Continued {
                    resolved_notes: self.selected_notes(),
                }),
            })
    }

    /// 現在の採用値、フォーカス、スクロール状態を反映したWidgetを作成する。
    pub fn create_widget(&self, area: Rect) -> VerticalScrollWidget<'_> {
        let widget = RemoteJournalConflictWidget::new(
            self.diff.before.clone(),
            self.diff.after.clone(),
            self.server_notes.clone(),
            self.selected_choice,
        )
        .with_focused_button(self.focus_state.focused_button());
        let line_count = widget.line_count(area.width) as u16;
        let mut scroll_widget = VerticalScrollWidget::new(
            &self.vertical_scroll_state,
            Size::new(area.width, area.height),
        );
        scroll_widget.render_widget(widget, line_count);
        scroll_widget
    }

    /// スクロール適用後の画面上カーソル位置を返す。
    ///
    /// カーソルが描画領域外にある場合は `None` を返す。
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let cursor = self.cursor_global_position(area.width);
        let position = self
            .vertical_scroll_state
            .calc_cursor_area_position(cursor, area);
        if position.y >= area.y && position.y < area.y.saturating_add(area.height) {
            Some(position)
        } else {
            None
        }
    }

    fn cursor_global_position(&self, width: u16) -> Position {
        let widget = RemoteJournalConflictWidget::new(
            self.diff.before.clone(),
            self.diff.after.clone(),
            self.server_notes.clone(),
            self.selected_choice,
        );
        match self.focus_state.target() {
            FocusTarget::Choice(choice_index) => {
                widget.cursor_position_for_choice(choice_index, width)
            }
            FocusTarget::Button(button) => widget.cursor_position_for_button(button, width),
        }
    }

    fn choice_at(&self, choice_index: usize) -> RemoteJournalConflictChoice {
        match choice_index {
            0 => RemoteJournalConflictChoice::Before,
            1 => RemoteJournalConflictChoice::Local,
            2 => RemoteJournalConflictChoice::Server,
            _ => panic!("choice index must be 0..=2"),
        }
    }

    fn selected_notes(&self) -> String {
        match self.selected_choice {
            RemoteJournalConflictChoice::Before => self.diff.before.clone(),
            RemoteJournalConflictChoice::Local => self.diff.after.clone(),
            RemoteJournalConflictChoice::Server => self.server_notes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::{Position, Rect};

    use crate::components::remote_journal_conflict_popup::widget::RemoteJournalConflictButton;
    use crate::test_support::render_snapshot;
    use crate::vos::JournalNotesDiff;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn component() -> RemoteJournalConflictComponent {
        RemoteJournalConflictComponent::new(
            JournalNotesDiff {
                before: "before notes".to_string(),
                after: "local notes".to_string(),
            },
            "server notes".to_string(),
        )
    }

    fn widget_position(target: FocusTarget, width: u16) -> Position {
        let widget = RemoteJournalConflictWidget::new(
            "before notes",
            "local notes",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );
        match target {
            FocusTarget::Choice(index) => widget.cursor_position_for_choice(index, width),
            FocusTarget::Button(button) => widget.cursor_position_for_button(button, width),
        }
    }

    #[test]
    fn new_focuses_local_choice() {
        let mut component = component();
        component.update(AREA);

        assert_eq!(
            component.cursor_position(AREA),
            Some(widget_position(FocusTarget::Choice(1), AREA.width))
        );
    }

    #[test]
    fn j_and_k_move_cursor_between_choices() {
        let mut component = component();
        component.update(AREA);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(widget_position(FocusTarget::Choice(2), AREA.width))
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(widget_position(FocusTarget::Choice(1), AREA.width))
        );
    }

    fn assert_button_cursor(cursor: Option<Position>, button: RemoteJournalConflictButton) {
        let expected_x = widget_position(FocusTarget::Button(button), AREA.width).x;
        let cursor = cursor.expect("button cursor should be visible");
        assert_eq!(cursor.x, expected_x);
    }

    #[test]
    fn h_and_l_move_cursor_between_cancel_and_continue_buttons() {
        let mut component = component();
        component.update(AREA);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        component.update(AREA);
        assert_button_cursor(
            component.cursor_position(AREA),
            RemoteJournalConflictButton::Cancel,
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );
        component.update(AREA);
        assert_button_cursor(
            component.cursor_position(AREA),
            RemoteJournalConflictButton::Continue,
        );
    }

    #[test]
    fn j_from_last_choice_focuses_continue_button() {
        let mut component = component();
        component.update(AREA);

        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.update(AREA);

        assert_button_cursor(
            component.cursor_position(AREA),
            RemoteJournalConflictButton::Continue,
        );
    }

    #[test]
    fn enter_on_choice_changes_selected_notes() {
        let mut component = component();

        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Enter));
        let result = component.process_event(key_event(KeyCode::Char('l')));
        assert!(result.is_none());

        let Some(EventProcessResult::Continued { resolved_notes }) =
            component.process_event(key_event(KeyCode::Enter))
        else {
            panic!("continue should return selected notes");
        };
        assert_eq!(resolved_notes, "server notes");
    }

    #[test]
    fn q_and_esc_request_popup_close() {
        let mut component = component();

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('q'))),
            Some(EventProcessResult::Canceled)
        ));
        assert!(matches!(
            component.process_event(key_event(KeyCode::Esc)),
            Some(EventProcessResult::Canceled)
        ));
    }

    #[test]
    fn enter_on_cancel_requests_popup_close() {
        let mut component = component();

        component.process_event(key_event(KeyCode::Char('h')));
        assert!(matches!(
            component.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::Canceled)
        ));
    }

    #[test]
    fn enter_on_continue_returns_local_notes_by_default() {
        let mut component = component();

        component.process_event(key_event(KeyCode::Char('l')));
        let result = component.process_event(key_event(KeyCode::Enter));

        let Some(EventProcessResult::Continued { resolved_notes }) = result else {
            panic!("continue should return selected notes");
        };
        assert_eq!(resolved_notes, "local notes");
    }

    #[test]
    fn update_keeps_cursor_visible_when_notes_wrap() {
        let mut component = RemoteJournalConflictComponent::new(
            JournalNotesDiff {
                before: "before notes".to_string(),
                after: "local notes that are long enough to wrap over several lines".to_string(),
            },
            "server notes".to_string(),
        );
        component.update(AREA);
        for _ in 0..4 {
            component.process_event(key_event(KeyCode::Char('j')));
            component.update(AREA);
        }

        let cursor = component
            .cursor_position(AREA)
            .expect("cursor should be visible after scroll update");
        assert!(cursor.y >= AREA.y);
        assert!(cursor.y < AREA.y + AREA.height);
    }

    #[test]
    fn snapshot_create_widget_renders_vertical_scroll_widget_with_button_focus() {
        let mut component = component();
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('l')));
        component.update(AREA);

        render_snapshot(
            "remote_journal_conflict_component_vertical_scroll_widget_continue_focus",
            AREA.width,
            AREA.height,
            component.create_widget(AREA),
        );
    }
}
