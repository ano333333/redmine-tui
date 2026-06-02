use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use crate::app::Store;
use crate::entities::Journal;

use super::comment_journal::focus_state::FocusState as CommentFocusState;
use super::comment_journal::widget::{CommentJournalWidget, CommentJournalWidgetState};
use super::focus_state::{
    EventProcessResult, FocusEvent, FocusState, ItemFocusState, ItemMetrics, ItemUpdate,
};
use super::property_journal::focus_state::FocusState as PropertyFocusState;
use super::property_journal::widget::PropertyJournalWidget;
use super::widget::{JournalItemWidget, JournalsListWidget};

pub struct JournalsListComponent {
    id: u16,
    focus_state: FocusState,
    comment_widget_states: HashMap<u16, CommentJournalWidgetState>,
}

impl JournalsListComponent {
    pub fn new(issue_id: u16) -> Self {
        Self {
            id: issue_id,
            focus_state: FocusState::new(),
            comment_widget_states: HashMap::new(),
        }
    }

    pub fn process_event(
        &mut self,
        event: &crossterm::event::Event,
        _: &Store,
        _: u16,
    ) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, _: &Store, event: FocusEvent, _: u16) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, store: &Store, width: u16) -> Option<EventProcessResult> {
        let Some(issue) = store.get_issue(self.id) else {
            return None;
        };

        let mut active_comment_ids = Vec::new();
        let mut items = Vec::with_capacity(issue.journal_ids.len());
        for id in &issue.journal_ids {
            let Some(journal) = store.get_journal(*id) else {
                continue;
            };
            match journal {
                Journal::Comment {
                    creator,
                    updated_at,
                    body,
                    ..
                } => {
                    let widget_state = self
                        .comment_widget_states
                        .entry(*id)
                        .or_insert_with(CommentJournalWidgetState::new);
                    widget_state.update(width, creator, updated_at, body);
                    let body_line_count = widget_state.body_line_count(width);
                    let line_count = widget_state.line_count(width);
                    let mut focus_state = CommentFocusState::new();
                    focus_state.update(width, body_line_count);
                    items.push(ItemUpdate {
                        id: *id,
                        line_count,
                        metrics: ItemMetrics::Comment {
                            width,
                            body_line_count,
                        },
                        focus_state: ItemFocusState::Comment(focus_state),
                    });
                    active_comment_ids.push(*id);
                }
                Journal::Property {
                    creator,
                    target,
                    old,
                    new,
                    updated_at,
                    ..
                } => {
                    let widget =
                        PropertyJournalWidget::new(creator, target, old, new, updated_at, false);
                    let line_count = widget.line_count(width);
                    let mut focus_state = PropertyFocusState::new();
                    focus_state.update(line_count);
                    items.push(ItemUpdate {
                        id: *id,
                        line_count,
                        metrics: ItemMetrics::Property { line_count },
                        focus_state: ItemFocusState::Property(focus_state),
                    });
                }
            }
        }

        self.comment_widget_states
            .retain(|id, _| active_comment_ids.contains(id));
        self.focus_state.update(items);
        None
    }

    pub fn render(&self, store: &Store, area: Rect, buf: &mut Buffer) {
        let widgets = self.create_widgets(store);
        JournalsListWidget::new(&widgets).render(area, buf);
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        let widgets = self.create_widgets(store);
        JournalsListWidget::new(&widgets).line_count(width)
    }

    pub fn get_cursor_position(&self, _: &Store, _: u16) -> Position {
        self.focus_state.get_cursor_position()
    }

    fn create_widgets<'a>(&'a self, store: &'a Store) -> Vec<JournalItemWidget<'a>> {
        let Some(issue) = store.get_issue(self.id) else {
            return vec![];
        };

        issue
            .journal_ids
            .iter()
            .filter_map(|id| {
                let journal = store.get_journal(*id)?;
                match journal {
                Journal::Comment {
                    ..
                } => self
                    .comment_widget_states
                    .get(id)
                    .map(|state| {
                        JournalItemWidget::Comment(CommentJournalWidget::new(
                            state,
                            self.focus_state.is_item_focused(*id),
                        ))
                    }),
                Journal::Property {
                    creator,
                    target,
                        old,
                        new,
                        updated_at,
                        ..
                    } => Some(JournalItemWidget::Property(PropertyJournalWidget::new(
                        creator,
                        target,
                        old,
                        new,
                        updated_at,
                        self.focus_state.is_item_focused(*id),
                    ))),
                }
            })
            .collect()
    }
}
