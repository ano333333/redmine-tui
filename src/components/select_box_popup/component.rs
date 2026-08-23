use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode};

use crate::stores::Dispatcher;

use super::SelectBoxPopupWidget;

pub enum EventProcessResult {
    Entered,
    Quited,
}

const NONE_CHOICE_LABEL: &str = "選択なし(None)";

pub struct SelectBoxPopupComponent<'a> {
    items: Vec<(Option<u16>, String)>,
    focused_index: usize,
    observer: Box<dyn FnMut(Option<u16>) + 'a>,
}

impl<'a> SelectBoxPopupComponent<'a> {
    pub fn new(
        items: &[(u16, String)],
        focused_index: usize,
        include_none: bool,
        observer: Box<dyn FnMut(Option<u16>) + 'a>,
    ) -> Self {
        let mut popup_items = Vec::with_capacity(items.len() + usize::from(include_none));
        if include_none {
            popup_items.push((None, NONE_CHOICE_LABEL.to_string()));
        }
        popup_items.extend(items.iter().map(|(id, name)| (Some(*id), name.clone())));

        Self {
            items: popup_items,
            focused_index: focused_index + usize::from(include_none),
            observer,
        }
    }

    pub fn process_event(
        &mut self,
        event: Event,
        _dispatcher: Rc<RefCell<Dispatcher>>,
    ) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => {
                if self.focused_index + 1 < self.items.len() {
                    self.focused_index += 1;
                }
            }
            KeyCode::Char('k') => {
                if self.focused_index > 0 {
                    self.focused_index -= 1;
                }
            }
            KeyCode::Enter => {
                let (id, _) = self.items[self.focused_index];
                (self.observer)(id);
                return Some(EventProcessResult::Entered);
            }
            KeyCode::Char('q') => {
                return Some(EventProcessResult::Quited);
            }
            _ => {}
        }
        None
    }

    pub fn create_widget<'b>(&'b self) -> SelectBoxPopupWidget<'b> {
        SelectBoxPopupWidget::new(&self.items, self.focused_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(Dispatcher::new()))
    }

    fn items() -> Vec<(u16, String)> {
        vec![
            (1, "New".to_string()),
            (2, "Doing".to_string()),
            (3, "Done".to_string()),
        ]
    }

    fn component_with_observer(
        focused_index: usize,
        include_none: bool,
        selected_id: Rc<RefCell<Option<Option<u16>>>>,
        selected_dispatcher: Rc<RefCell<Option<Rc<RefCell<Dispatcher>>>>>,
    ) -> SelectBoxPopupComponent<'static> {
        SelectBoxPopupComponent::new(
            &items(),
            focused_index,
            include_none,
            Box::new(move |id| {
                *selected_id.borrow_mut() = Some(id);
                *selected_dispatcher.borrow_mut() = None;
            }),
        )
    }

    #[test]
    fn new_sets_widget_properties() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let component = component_with_observer(1, false, selected_id, selected_dispatcher);

        let widget = component.create_widget();
        assert_eq!(
            widget.items,
            [
                (Some(1), "New".to_string()),
                (Some(2), "Doing".to_string()),
                (Some(3), "Done".to_string())
            ]
        );
        assert_eq!(widget.focused_index, 1);
    }

    #[test]
    fn new_can_include_none_choice_before_items() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let component = component_with_observer(1, true, selected_id, selected_dispatcher);

        let widget = component.create_widget();
        assert_eq!(
            widget.items,
            [
                (None, "選択なし(None)".to_string()),
                (Some(1), "New".to_string()),
                (Some(2), "Doing".to_string()),
                (Some(3), "Done".to_string())
            ]
        );
        assert_eq!(widget.focused_index, 2);
    }

    #[test]
    fn process_event_j_moves_focus_forward() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(0, false, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('j')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_j_stops_at_last_item() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(2, false, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('j')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 2);
    }

    #[test]
    fn process_event_k_moves_focus_backward() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(2, false, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('k')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_k_stops_at_first_item() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(0, false, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('k')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 0);
    }

    #[test]
    fn process_event_enter_notifies_observer_with_focused_id() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component =
            component_with_observer(1, false, selected_id.clone(), selected_dispatcher.clone());
        let dispatcher = dispatcher();

        let result = component.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*selected_id.borrow(), Some(Some(2)));
        assert!(selected_dispatcher.borrow().is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_enter_notifies_observer_with_none() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component =
            component_with_observer(0, true, selected_id.clone(), selected_dispatcher.clone());
        let dispatcher = dispatcher();

        let move_result =
            component.process_event(key_event(KeyCode::Char('k')), dispatcher.clone());
        let result = component.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(move_result.is_none());
        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*selected_id.borrow(), Some(None));
        assert!(selected_dispatcher.borrow().is_none());
        assert_eq!(component.create_widget().focused_index, 0);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(1, false, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('q')), dispatcher());

        assert!(matches!(result, Some(EventProcessResult::Quited)));
        assert_eq!(component.create_widget().focused_index, 1);
    }
}
