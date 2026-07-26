use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect, Size};
use ratatui::widgets::Widget;

use crate::widgets::{Hr, VerticalScrollWidget, VerticalScrollWidgetState};

use super::body::widget::BodyWidget;
use super::children_list::widget::ChildrenListWidget;
use super::header::widget::HeaderWidget;
use super::journals_list::JournalsListWidget;
use super::property::widget::PropertyWidget;

pub struct IssueDetailWidgetState {
    /// HeaderWidget以外の全てのWidgetのスクロール状態
    vertical_scroll_state: VerticalScrollWidgetState,
}

impl IssueDetailWidgetState {
    pub fn new() -> Self {
        Self {
            vertical_scroll_state: VerticalScrollWidgetState::new(),
        }
    }

    #[cfg(test)]
    fn offset_y(&self) -> u16 {
        self.vertical_scroll_state.offset_y()
    }

    /// # Arguments
    ///
    /// * `cursor_global_position` - **HeaderWidgetを含む**全てのWidgetの仮想バッファから見たカーソル位置
    /// * `height` - IssueDetailWidgetの表示行数
    /// * `header_height` - HeaderWidgetの高さ・行数
    pub fn update(&mut self, cursor_global_position: Position, height: u16, header_height: u16) {
        if cursor_global_position.y < header_height {
            self.vertical_scroll_state.update(
                Position {
                    x: cursor_global_position.x,
                    y: 0,
                },
                1,
            );
            return;
        }

        let content_height = height.saturating_sub(header_height);
        let content_cursor_position = Position {
            x: cursor_global_position.x,
            y: cursor_global_position.y - header_height,
        };

        self.vertical_scroll_state
            .update(content_cursor_position, content_height);
    }

    /// **HeaderWidgetを含む**全てのWidgetの仮想バッファから見たカーソル位置を、
    /// クライアント座標に変換する。
    ///
    /// # Arguments
    ///
    /// * `cursor_global_position` - **HeaderWidgetを含む**全てのWidgetの仮想バッファから見たカーソル位置
    /// * `area` - このWidgetを描画するクライアント領域
    /// * `header_height` - HeaderWidgetの高さ・行数
    pub fn calc_cursor_area_position(
        &self,
        cursor_global_position: Position,
        area: Rect,
        header_height: u16,
    ) -> Position {
        if cursor_global_position.y < header_height {
            return Position {
                x: area.x + cursor_global_position.x,
                y: area.y + cursor_global_position.y,
            };
        }

        let content_area = Rect::new(
            area.x,
            area.y + header_height,
            area.width,
            area.height.saturating_sub(header_height),
        );
        let content_cursor_position = Position {
            x: cursor_global_position.x,
            y: cursor_global_position.y - header_height,
        };

        self.vertical_scroll_state
            .calc_cursor_area_position(content_cursor_position, content_area)
    }
}

pub struct IssueDetailWidget<'a> {
    header: HeaderWidget<'a>,
    property: PropertyWidget<'a>,
    body: BodyWidget<'a>,
    children_list: ChildrenListWidget<'a>,
    journals_list: JournalsListWidget<'a>,
    state: &'a IssueDetailWidgetState,
}

impl<'a> Widget for IssueDetailWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width;
        let header_height = self.header.line_count(width) as u16;

        let header_area = Rect::new(area.x, area.y, area.width, area.height.min(header_height));
        self.header.render(header_area, buf);

        let scroll_area = Rect::new(
            area.x,
            area.y + area.height.min(header_height),
            area.width,
            area.height.saturating_sub(header_height),
        );
        let mut scroll_widget = VerticalScrollWidget::new(
            &self.state.vertical_scroll_state,
            Size::new(width, scroll_area.height),
        );

        let property_line_count = self.property.line_count(width) as u16;
        let body_line_count = self.body.line_count(width) as u16;
        let children_line_count = self.children_list.line_count();
        let journals_line_count = self.journals_list.line_count(width);

        scroll_widget.render_widget(self.property, property_line_count);
        scroll_widget.render_widget(Hr::default(), 1);
        scroll_widget.render_widget(self.body, body_line_count);
        scroll_widget.render_widget(Hr::default(), 1);
        scroll_widget.render_widget(self.children_list, children_line_count);
        scroll_widget.render_widget(Hr::default(), 1);
        scroll_widget.render_widget(self.journals_list, journals_line_count);

        scroll_widget.render(scroll_area, buf);
    }
}

impl<'a> IssueDetailWidget<'a> {
    pub fn new(
        header: HeaderWidget<'a>,
        property: PropertyWidget<'a>,
        body: BodyWidget<'a>,
        children_list: ChildrenListWidget<'a>,
        journals_list: JournalsListWidget<'a>,
        state: &'a IssueDetailWidgetState,
    ) -> Self {
        Self {
            header,
            property,
            body,
            children_list,
            journals_list,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::{Position, Rect};

    use super::*;
    use crate::components::issue::body::widget::BodyWidgetState;
    use crate::components::issue::children_list::widget::ChildIssueRow;
    use crate::components::issue::journals_list::journals_list_item::{
        JournalItemWidget, JournalItemWidgetState,
    };
    use crate::entities::{Issue, IssueStatus, Journal};
    use crate::test_support::{local_datetime, render_snapshot, sample_issue};
    use crate::vos::{JournalDetail, JournalDetailAttr, JournalId};

    const WIDTH: u16 = 40;
    const HEIGHT: u16 = 10;
    const TALL_HEIGHT: u16 = 42;

    struct IssueDetailWidgetFixture {
        title: String,
        status: String,
        tracker: String,
        priority: String,
        project: String,
        person_in_charge: Option<String>,
        target_version: Option<String>,
        resolve_way: Option<String>,
        component: String,
        body_state: BodyWidgetState,
        child_issue: Issue,
        child_status: IssueStatus,
        journal: Journal,
        journal_state: JournalItemWidgetState,
    }

    impl IssueDetailWidgetFixture {
        fn new(width: u16) -> Self {
            let body =
                "Body first line\n\nBody paragraph with enough words to wrap in narrow layouts."
                    .to_string();
            let mut body_state = BodyWidgetState::new();
            body_state.update(width, &body);

            let journal_user = "alice".to_string();
            let journal_updated_on = local_datetime("2026-01-15T00:00:00+09:00");
            let journal_notes =
                "Journal note first paragraph\n\nJournal note second paragraph".to_string();
            let mut journal_state = JournalItemWidgetState::new();
            journal_state.update(width, &journal_user, &journal_updated_on, &journal_notes);
            let journal = Journal {
                id: JournalId::new(1),
                user: journal_user,
                updated_on: journal_updated_on,
                details: vec![JournalDetail::Attr(JournalDetailAttr::AssignedTo {
                    old: None,
                    new: Some("bob".to_string()),
                })],
                notes: journal_notes,
            };

            Self {
                title: "Issue detail widget snapshot".to_string(),
                status: "In Progress".to_string(),
                tracker: "Bug".to_string(),
                priority: "critical".to_string(),
                project: "Sample Project".to_string(),
                person_in_charge: Some("alice".to_string()),
                target_version: Some("2026 Spring".to_string()),
                resolve_way: Some("Patch".to_string()),
                component: "Admin UI".to_string(),
                body_state,
                child_issue: sample_issue(
                    7,
                    "Open child",
                    5.into(),
                    Some(1),
                    Some("2026-01-10T00:00:00+09:00"),
                    Some("2026-01-20T00:00:00+09:00"),
                    35,
                ),
                child_status: IssueStatus {
                    id: 5.into(),
                    name: "進行中".to_string(),
                    is_closed: false,
                },
                journal,
                journal_state,
            }
        }

        fn widget<'a>(&'a self, state: &'a IssueDetailWidgetState) -> IssueDetailWidget<'a> {
            IssueDetailWidget::new(
                self.header_widget(),
                self.property_widget(),
                self.body_widget(),
                self.children_list_widget(),
                self.journals_list_widget(),
                state,
            )
        }

        fn header_widget(&self) -> HeaderWidget<'_> {
            HeaderWidget::new(42, &self.title, false, true)
        }

        fn property_widget(&self) -> PropertyWidget<'_> {
            PropertyWidget::new(
                42,
                "author",
                local_datetime("2026-01-10T00:00:00+09:00"),
                local_datetime("2026-01-15T00:00:00+09:00"),
                self.status.as_str(),
                self.tracker.as_str(),
                self.priority.as_str(),
                self.project.as_str(),
                self.person_in_charge.as_deref(),
                &self.target_version,
                Some(local_datetime("2026-01-10T00:00:00+09:00")),
                Some(local_datetime("2026-01-20T00:00:00+09:00")),
                65,
                Some(13),
                Some(8.5),
                &self.resolve_way,
                &self.component,
                Some(3),
            )
        }

        fn body_widget(&self) -> BodyWidget<'_> {
            BodyWidget::new(&self.body_state, false)
        }

        fn children_list_widget(&self) -> ChildrenListWidget<'_> {
            ChildrenListWidget::new(
                1,
                0,
                1,
                vec![ChildIssueRow {
                    issue: &self.child_issue,
                    issue_status: &self.child_status,
                    assigned_to_name: Some("alice"),
                }],
                Some(0),
            )
        }

        fn journals_list_widget(&self) -> JournalsListWidget<'_> {
            JournalsListWidget::new(vec![JournalItemWidget::new(
                &self.journal,
                &self.journal_state,
                false,
            )])
        }

        fn header_height(&self, width: u16) -> u16 {
            self.header_widget().line_count(width) as u16
        }

        fn journals_start_y(&self, width: u16) -> u16 {
            self.header_height(width)
                + self.property_widget().line_count(width) as u16
                + 1
                + self.body_widget().line_count(width) as u16
                + 1
                + self.children_list_widget().line_count()
                + 1
        }
    }

    fn assert_cursor_position_after_update(
        state: &mut IssueDetailWidgetState,
        cursor_global_position: Position,
        height: u16,
        header_height: u16,
        expected_offset_y: u16,
        expected_cursor_area_position: Position,
    ) {
        state.update(cursor_global_position, height, header_height);
        assert_eq!(state.offset_y(), expected_offset_y);
        assert_eq!(
            state.calc_cursor_area_position(
                cursor_global_position,
                Rect::new(0, 0, WIDTH, height),
                header_height,
            ),
            expected_cursor_area_position
        );
    }

    #[test]
    fn snapshot_issue_detail_cursor_in_header_keeps_content_at_top() {
        let fixture = IssueDetailWidgetFixture::new(WIDTH);
        let mut state = IssueDetailWidgetState::new();
        let header_height = fixture.header_height(WIDTH);
        let cursor = Position { x: 2, y: 2 };

        assert_cursor_position_after_update(
            &mut state,
            cursor,
            HEIGHT,
            header_height,
            0,
            Position { x: 2, y: 2 },
        );

        render_snapshot(
            "issue_detail_cursor_in_header",
            WIDTH,
            HEIGHT,
            fixture.widget(&state),
        );
    }

    #[test]
    fn snapshot_issue_detail_cursor_in_visible_content_keeps_content_at_top() {
        let fixture = IssueDetailWidgetFixture::new(WIDTH);
        let mut state = IssueDetailWidgetState::new();
        let header_height = fixture.header_height(WIDTH);
        let cursor = Position {
            x: 0,
            y: header_height + 3,
        };

        assert_cursor_position_after_update(
            &mut state,
            cursor,
            HEIGHT,
            header_height,
            0,
            Position {
                x: 0,
                y: header_height + 3,
            },
        );

        render_snapshot(
            "issue_detail_cursor_in_visible_content",
            WIDTH,
            HEIGHT,
            fixture.widget(&state),
        );
    }

    #[test]
    fn snapshot_issue_detail_tall_area_shows_children_and_journals_without_scrolling() {
        let fixture = IssueDetailWidgetFixture::new(WIDTH);
        let mut state = IssueDetailWidgetState::new();
        let header_height = fixture.header_height(WIDTH);
        let cursor = Position {
            x: 0,
            y: fixture.journals_start_y(WIDTH) + 1,
        };

        assert_cursor_position_after_update(
            &mut state,
            cursor,
            TALL_HEIGHT,
            header_height,
            0,
            cursor,
        );

        render_snapshot(
            "issue_detail_tall_area_children_and_journals",
            WIDTH,
            TALL_HEIGHT,
            fixture.widget(&state),
        );
    }

    #[test]
    fn snapshot_issue_detail_height_shorter_than_header_does_not_panic() {
        let fixture = IssueDetailWidgetFixture::new(WIDTH);
        let mut state = IssueDetailWidgetState::new();
        let height = 3;
        let header_height = fixture.header_height(WIDTH);
        let cursor = Position {
            x: 0,
            y: header_height + 1,
        };

        state.update(cursor, height, header_height);
        assert_eq!(
            state.calc_cursor_area_position(cursor, Rect::new(0, 0, WIDTH, height), header_height),
            Position {
                x: 0,
                y: header_height
            }
        );

        render_snapshot(
            "issue_detail_height_shorter_than_header",
            WIDTH,
            height,
            fixture.widget(&state),
        );
    }
}
