
use std::rc::Rc;
use std::cell::Cell;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Signal<T: Copy>(Rc<Cell<T>>);

impl<T: Copy> Signal<T> {
    pub fn new(init: T) -> Signal<T> {
        Signal(Rc::new(Cell::new(init)))
    }

    pub fn set(&mut self, value: T) {
        self.0.set(value);
    }

    pub fn get(&mut self) -> T {
        self.0.get()
    }
}


#[derive(Clone, Debug)]
pub struct SyncSignal<T: Copy>(Arc<Mutex<T>>);

impl<T: Copy> SyncSignal<T> {
    pub fn new(init: T) -> SyncSignal<T> {
        SyncSignal(Arc::new(Mutex::new(init)))
    }

    pub fn set(&mut self, value: T) {
        *(self.0.lock().unwrap()) = value;
    }

    pub fn get(&mut self) -> T {
        *(self.0.lock().unwrap())
    }
}


