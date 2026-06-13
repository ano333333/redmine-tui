use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode};

use crate::app::Dispatcher;

use super::SelectBoxPopupWidget;

pub enum EventProcessResult {
    Entered,
    Quited,
}

pub struct SelectBoxPopupComponent {
    items: Vec<(u16, String)>,
    focused_index: usize,
    observer: Box<dyn FnMut(u16)>,
}

impl SelectBoxPopupComponent {
    pub fn new(items: &[(u16, String)], focused_index: usize, observer: Box<dyn FnMut(u16)>) -> Self {
        Self {
            items: Vec::from(items),
            focused_index,
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

    pub fn create_widget<'a>(&'a self) -> SelectBoxPopupWidget<'a> {
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
        selected_id: Rc<RefCell<Option<u16>>>,
        selected_dispatcher: Rc<RefCell<Option<Rc<RefCell<Dispatcher>>>>>,
    ) -> SelectBoxPopupComponent {
        SelectBoxPopupComponent::new(
            &items(),
            focused_index,
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
        let component = component_with_observer(1, selected_id, selected_dispatcher);

        let widget = component.create_widget();
        assert_eq!(widget.items, [(1, "New".to_string()), (2, "Doing".to_string()), (3, "Done".to_string())]);
        assert_eq!(widget.focused_index, 1);
    }

    #[test]
    fn process_event_j_moves_focus_forward() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(0, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('j')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_j_stops_at_last_item() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(2, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('j')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 2);
    }

    #[test]
    fn process_event_k_moves_focus_backward() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(2, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('k')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_k_stops_at_first_item() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(0, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('k')), dispatcher());

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_index, 0);
    }

    #[test]
    fn process_event_enter_notifies_observer_with_focused_id() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component =
            component_with_observer(1, selected_id.clone(), selected_dispatcher.clone());
        let dispatcher = dispatcher();

        let result = component.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*selected_id.borrow(), Some(2));
        assert!(selected_dispatcher.borrow().is_none());
        assert_eq!(component.create_widget().focused_index, 1);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let selected_id = Rc::new(RefCell::new(None));
        let selected_dispatcher = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(1, selected_id, selected_dispatcher);

        let result = component.process_event(key_event(KeyCode::Char('q')), dispatcher());

        assert!(matches!(result, Some(EventProcessResult::Quited)));
        assert_eq!(component.create_widget().focused_index, 1);
    }
}
