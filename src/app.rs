use std::collections::{HashMap, VecDeque};

pub struct Dispatcher {
    store: Store,
    actions: VecDeque<Action>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher {
            store: Store::new(),
            actions: VecDeque::new(),
        }
    }
    pub fn store(&self) -> &Store {
        &self.store
    }
    pub fn dispatch(&mut self, action: Action) {
        self.actions.push_back(action);
    }
    pub fn consume_actions(&mut self) {
        while let Some(action) = self.actions.pop_front() {
            self.store.consume_action(action);
        }
    }
}

pub struct Store {
    counter: u16,
    counter_observers: HashMap<u16, Box<dyn Fn(u16)>>,
    counter_observers_key: u16,
}

impl Store {
    pub fn new() -> Self {
        Self {
            counter: 0,
            counter_observers: HashMap::new(),
            counter_observers_key: 0,
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Increment => {
                self.counter += 1;
            }
        }
    }

    pub fn get_counter(&self) -> u16 {
        self.counter
    }

    pub fn append_counter_observer(&mut self, observer: Box<dyn Fn(u16)>) {
        self.counter_observers
            .insert(self.counter_observers_key, observer);
        self.counter_observers_key += 1;
    }

    pub fn remove_counter_observer(&mut self, key: u16) {
        self.counter_observers.remove(&key);
    }

    pub fn consume_action(&mut self, action: Action) {
        match action {
            Action::Increment => {
                self.counter += 1;
                for (_, observer) in &self.counter_observers {
                    observer(self.counter);
                }
            }
        }
    }
}

pub enum Action {
    Increment,
}
