
use std::rc::Rc;
use std::cell::{Cell, RefCell, RefMut};

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
pub struct Latch<T>(Rc<RefCell<T>>);

impl<T> Latch<T> {
    pub fn new(init: T) -> Latch<T> {
        Latch(Rc::new(RefCell::new(init)))
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }
}

