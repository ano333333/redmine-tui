use crate::stores::LocalJournalEntry;

use super::journals_list_item::widget::local_state_marker;
use super::journals_list_item::{JournalItemWidget, JournalItemWidgetState, LocalJournalItemView};

/// Local Journalの表示内容と本文の描画cacheを保持する、focusを持たないcomponent。
pub struct LocalJournalItemComponent {
    notes: String,
    state_marker: &'static str,
    comment_line_count: u16,
    widget_state: JournalItemWidgetState,
}

impl LocalJournalItemComponent {
    pub fn new() -> Self {
        Self {
            notes: String::new(),
            state_marker: "(local)",
            comment_line_count: 0,
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn update(&mut self, entry: &LocalJournalEntry, width: u16) {
        self.notes.clone_from(&entry.journal.notes);
        self.state_marker = local_state_marker(&entry.state);
        self.widget_state
            .update(width, "", &chrono::Local::now(), &self.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
    }

    pub fn create_widget(&self) -> JournalItemWidget<'_> {
        JournalItemWidget::new(
            LocalJournalItemView {
                notes: &self.notes,
                state_marker: self.state_marker,
            },
            vec![],
            &self.widget_state,
            // Local Journal専用のfocusと編集経路が接続されるまでは表示だけを担う。
            false,
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + 1 + self.comment_line_count + 1
    }
}
