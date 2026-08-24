use std::{cell::RefCell, rc::Rc};

use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Widget,
};

use crate::{
    stores::{Dispatcher, IssueState, Store},
    vos::IssueId,
};

use super::{
    IssueWidget,
    detail::{self, IssueDetailComponent},
};

#[derive(Debug, PartialEq, Eq)]
pub enum EventProcessResult {
    FetchRequested { id: IssueId },
    OpenIssueSelectPopup,
    Detail(detail::EventProcessResult),
}

pub struct IssueComponent {
    issue_id: IssueId,
    detail: Option<IssueDetailComponent>,
}

fn detail_first<T>(detail_result: Option<T>, fallback: impl FnOnce() -> Option<T>) -> Option<T> {
    detail_result.or_else(fallback)
}

impl IssueComponent {
    pub fn new(store: &Store, issue_id: impl Into<IssueId>) -> (Self, Option<EventProcessResult>) {
        let issue_id = issue_id.into();
        let (detail, result) = Self::build_for_state(store, issue_id);
        (Self { issue_id, detail }, result)
    }

    pub fn issue_id(&self) -> IssueId {
        self.issue_id
    }

    fn build_for_state(
        store: &Store,
        id: IssueId,
    ) -> (Option<IssueDetailComponent>, Option<EventProcessResult>) {
        match store.get_issue_state(id) {
            Some(IssueState::Synced | IssueState::Edited | IssueState::Uploading) => {
                (Some(IssueDetailComponent::new(id)), None)
            }
            Some(IssueState::Fetching) => (None, None),
            None | Some(IssueState::FetchFailed { .. }) => {
                (None, Some(EventProcessResult::FetchRequested { id }))
            }
        }
    }

    pub fn process_event(
        &mut self,
        event: Event,
        dispatcher: Rc<RefCell<Dispatcher>>,
    ) -> Option<EventProcessResult> {
        // Detail gets the first opportunity so future overlapping shortcuts keep
        // the behavior of the currently focused, more specific component.
        let detail_result = self
            .detail
            .as_mut()
            .and_then(|detail| detail.process_event(event.clone(), dispatcher.clone()))
            .map(EventProcessResult::Detail);

        detail_first(detail_result, || {
            self.process_own_event(&event, &dispatcher)
        })
    }

    fn process_own_event(
        &self,
        event: &Event,
        dispatcher: &Rc<RefCell<Dispatcher>>,
    ) -> Option<EventProcessResult> {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Char('y') {
                return Some(EventProcessResult::OpenIssueSelectPopup);
            }
            if key.code == KeyCode::Char('r') {
                let failed = matches!(
                    dispatcher.borrow().store().get_issue_state(self.issue_id),
                    Some(IssueState::FetchFailed { .. })
                );
                return failed.then_some(EventProcessResult::FetchRequested { id: self.issue_id });
            }
        }

        None
    }

    pub fn update(
        &mut self,
        dispatcher: Rc<RefCell<Dispatcher>>,
        store: &Store,
        frame_size: (u16, u16),
    ) {
        match store.get_issue_state(self.issue_id) {
            Some(IssueState::Synced | IssueState::Edited | IssueState::Uploading) => {
                if self.detail.is_none() {
                    self.detail = Some(IssueDetailComponent::new(self.issue_id));
                }
                self.detail
                    .as_mut()
                    .expect("loaded issue must have a detail component during update")
                    .update(dispatcher, store, frame_size);
            }
            None | Some(IssueState::Fetching | IssueState::FetchFailed { .. }) => {
                self.detail = None;
            }
        }
    }

    pub(crate) fn create_widget<'a>(&'a self, store: &'a Store) -> IssueWidget<'a> {
        match store.get_issue_state(self.issue_id) {
            None => IssueWidget::fetching(),
            Some(IssueState::Fetching) => IssueWidget::fetching(),
            Some(IssueState::FetchFailed { message }) => IssueWidget::fetch_failed(message),
            Some(IssueState::Synced | IssueState::Edited | IssueState::Uploading) => {
                let detail = self
                    .detail
                    .as_ref()
                    .expect("loaded issue must have a detail component after update");
                IssueWidget::detail(detail.create_widget(store))
            }
        }
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        self.create_widget(store).render(area, frame.buffer_mut());
    }

    pub fn calc_cursor_position(&self, store: &Store, area: Rect) -> Option<Position> {
        self.detail
            .as_ref()
            .map(|detail| detail.calc_cursor_position(store, area))
    }

    #[cfg(test)]
    pub(super) fn has_detail_component(&self) -> bool {
        self.detail.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{EventProcessResult, IssueComponent, IssueWidget};
    use crate::{
        stores::{Dispatcher, IssueAction},
        test_support::render_snapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(Dispatcher::new()))
    }

    fn consume(d: &Rc<RefCell<Dispatcher>>, action: IssueAction) {
        d.borrow_mut().dispatch(action);
        d.borrow_mut().consume_action();
    }

    fn component(
        d: &Rc<RefCell<Dispatcher>>,
        id: u16,
    ) -> (IssueComponent, Option<EventProcessResult>) {
        let borrow = d.borrow();
        IssueComponent::new(borrow.store(), id)
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn modified_key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn detail_first_does_not_run_fallback_when_detail_handles_event() {
        let fallback_called = Cell::new(false);

        let result = super::detail_first(Some(1), || {
            fallback_called.set(true);
            Some(2)
        });

        assert_eq!(result, Some(1));
        assert!(!fallback_called.get());
    }

    #[test]
    fn detail_first_runs_fallback_when_detail_does_not_handle_event() {
        let fallback_called = Cell::new(false);

        let result = super::detail_first(None, || {
            fallback_called.set(true);
            Some(2)
        });

        assert_eq!(result, Some(2));
        assert!(fallback_called.get());
    }

    #[test]
    fn unknown_issue_requests_fetch_without_detail() {
        let d = dispatcher();
        let (component, result) = component(&d, 42);
        assert_eq!(
            result,
            Some(EventProcessResult::FetchRequested { id: 42.into() })
        );
        assert_eq!(component.issue_id(), 42);
        assert!(!component.has_detail_component());
        let borrow = d.borrow();
        assert!(matches!(
            component.create_widget(borrow.store()),
            IssueWidget::Fetching
        ));
    }

    #[test]
    fn loaded_issue_builds_detail_without_fetch() {
        let d = dispatcher();
        consume(&d, IssueAction::Load { id: 3.into() });
        let (component, result) = component(&d, 3);
        assert_eq!(result, None);
        assert!(component.has_detail_component());
    }

    #[test]
    fn fetching_does_not_request_again() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        let (component, result) = component(&d, 42);
        assert_eq!(result, None);
        assert!(!component.has_detail_component());
    }

    #[test]
    fn failed_issue_requests_fetch_and_r_retries() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        consume(
            &d,
            IssueAction::FetchFailed {
                id: 42.into(),
                message: "offline".into(),
            },
        );
        let (mut component, result) = component(&d, 42);
        assert_eq!(
            result,
            Some(EventProcessResult::FetchRequested { id: 42.into() })
        );
        assert_eq!(
            component.process_event(key(KeyCode::Char('r')), d.clone()),
            result
        );
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        assert_eq!(
            component.process_event(key(KeyCode::Char('r')), d.clone()),
            None
        );
    }

    #[test]
    fn update_builds_detail_after_fetch_success() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 3.into() });
        let (mut component, _) = component(&d, 3);
        consume(
            &d,
            IssueAction::FetchSucceeded {
                id: 3.into(),
                issue: crate::libs::yaml::parse_issue_yaml(3),
            },
        );
        {
            let borrow = d.borrow();
            component.update(d.clone(), borrow.store(), (80, 24));
        }
        assert!(component.has_detail_component());
    }

    #[test]
    fn failed_store_message_is_forwarded_to_widget() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        consume(
            &d,
            IssueAction::FetchFailed {
                id: 42.into(),
                message: "timeout".into(),
            },
        );
        let (component, _) = component(&d, 42);
        let borrow = d.borrow();

        assert!(matches!(
            component.create_widget(borrow.store()),
            IssueWidget::FetchFailed { message: "timeout" }
        ));
    }

    #[test]
    fn another_issue_completion_does_not_change_target() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        let (mut component, _) = component(&d, 42);
        consume(&d, IssueAction::StartFetching { id: 3.into() });
        consume(
            &d,
            IssueAction::FetchSucceeded {
                id: 3.into(),
                issue: crate::libs::yaml::parse_issue_yaml(3),
            },
        );
        {
            let borrow = d.borrow();
            component.update(d.clone(), borrow.store(), (80, 24));
        }
        assert_eq!(component.issue_id(), 42);
        assert!(!component.has_detail_component());
    }

    #[test]
    fn y_opens_selector_even_without_loaded_issue() {
        let d = dispatcher();
        let (mut component, _) = component(&d, 42);
        assert_eq!(
            component.process_event(key(KeyCode::Char('y')), d),
            Some(EventProcessResult::OpenIssueSelectPopup)
        );
    }

    #[test]
    fn y_opens_selector_while_fetching_failed_and_loaded() {
        let d = dispatcher();
        consume(&d, IssueAction::StartFetching { id: 42.into() });
        let (mut fetching, _) = component(&d, 42);
        assert_eq!(
            fetching.process_event(key(KeyCode::Char('y')), d.clone()),
            Some(EventProcessResult::OpenIssueSelectPopup)
        );

        consume(
            &d,
            IssueAction::FetchFailed {
                id: 42.into(),
                message: "offline".into(),
            },
        );
        let (mut failed, _) = component(&d, 42);
        assert_eq!(
            failed.process_event(key(KeyCode::Char('y')), d.clone()),
            Some(EventProcessResult::OpenIssueSelectPopup)
        );

        consume(&d, IssueAction::Load { id: 3.into() });
        let (mut loaded, _) = component(&d, 3);
        assert_eq!(
            loaded.process_event(key(KeyCode::Char('y')), d),
            Some(EventProcessResult::OpenIssueSelectPopup)
        );
    }

    #[test]
    fn loaded_issue_delegates_detail_events() {
        let d = dispatcher();
        consume(&d, IssueAction::Load { id: 3.into() });
        let (mut component, _) = component(&d, 3);

        assert_eq!(
            component.process_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL), d,),
            Some(EventProcessResult::Detail(
                super::detail::EventProcessResult::StartIssueUpload,
            )),
        );
    }

    #[test]
    fn loaded_issue_creates_detail_widget() {
        let d = dispatcher();
        crate::test_support::dispatch_fixture_entity_actions(&mut d.borrow_mut());
        while d.borrow().consume_actinos_len() > 0 {
            d.borrow_mut().consume_action();
        }
        consume(&d, IssueAction::Load { id: 3.into() });
        let (component, _) = component(&d, 3);
        let borrow = d.borrow();

        assert!(matches!(
            component.create_widget(borrow.store()),
            IssueWidget::Detail(_)
        ));
    }

    #[test]
    fn status_snapshots() {
        render_snapshot("issue_fetching", 32, 3, IssueWidget::fetching());
        render_snapshot(
            "issue_fetch_failed",
            40,
            3,
            IssueWidget::fetch_failed("connection refused"),
        );
    }
}
