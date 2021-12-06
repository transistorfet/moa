
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

#[derive(Clone)]
//pub struct Register<T>(Rc<RefCell<T>>);
pub struct Register<T>(Rc<RefCell<(T, Option<Box<dyn Fn(&T)>>)>>);

impl<T> Register<T> {
    pub fn new(init: T) -> Register<T> {
        Register(Rc::new(RefCell::new((init, None))))
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        RefMut::map(self.0.borrow_mut(), |v| &mut v.0)
    }

    pub fn set_observer<F>(&self, f: F) where F: Fn(&T) + 'static {
        self.0.borrow_mut().1 = Some(Box::new(f));
    }

    pub fn notify(&self) {
        let data = self.0.borrow();
        if let Some(closure) = &data.1 {
            closure(&data.0);
        }
    }
}

