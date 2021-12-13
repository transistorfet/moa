
use std::rc::Rc;
use std::cell::{Cell, RefCell, RefMut};

pub trait Observable<T> {
    fn set_observer<F>(&self, f: F) where F: Fn(&T) + 'static;
    fn notify(&self);
}

// TODO these could be used to imply how it should be used, or they could even be shorthands for T=bool, except TriState which would have 3
// TODO or maybe even tristate is T=Option<S>
#[allow(dead_code)]
type Output<T> = Signal<T>;
#[allow(dead_code)]
type Input<T> = Signal<T>;
#[allow(dead_code)]
type TriState<T> = Signal<T>;


#[derive(Clone, Debug)]
pub struct Signal<T: Copy>(Rc<Cell<T>>);

impl<T: Copy> Signal<T> {
    pub fn new(init: T) -> Signal<T> {
        Signal(Rc::new(Cell::new(init)))
    }

    pub fn set(&mut self, value: T) {
        self.0.set(value);
    }

    pub fn get(&self) -> T {
        self.0.get()
    }
}


#[derive(Clone, Debug)]
pub struct EdgeSignal(Signal<bool>);

impl EdgeSignal {
    pub fn new() -> Self {
        EdgeSignal(Signal::new(false))
    }

    pub fn signal(&mut self) {
        self.0.set(true);
    }

    pub fn get(&mut self) -> bool {
        let value = self.0.get();
        self.0.set(false);
        value
    }
}


#[derive(Clone)]
pub struct ObservableSignal<T>(Rc<RefCell<(T, Option<Box<dyn Fn(&T)>>)>>);

impl<T> ObservableSignal<T> {
    pub fn new(init: T) -> ObservableSignal<T> {
        ObservableSignal(Rc::new(RefCell::new((init, None))))
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        RefMut::map(self.0.borrow_mut(), |v| &mut v.0)
    }
}

impl<T> Observable<T> for ObservableSignal<T> {
    fn set_observer<F>(&self, f: F) where F: Fn(&T) + 'static {
        self.0.borrow_mut().1 = Some(Box::new(f));
    }

    fn notify(&self) {
        let data = self.0.borrow();
        if let Some(closure) = &data.1 {
            closure(&data.0);
        }
    }
}


pub struct ObservableEdgeSignal(ObservableSignal<bool>);

impl ObservableEdgeSignal {
    pub fn new() -> Self {
        ObservableEdgeSignal(ObservableSignal::new(false))
    }

    pub fn set(&mut self) {
        *self.0.borrow_mut() = true;
        self.0.notify();
    }

    pub fn get(&mut self) -> bool {
        let mut addr = self.0.borrow_mut();
        let value = *addr;
        *addr = false;
        value
    }
}

impl Observable<bool> for ObservableEdgeSignal {
    fn set_observer<F>(&self, f: F) where F: Fn(&bool) + 'static {
        self.0.set_observer(f)
    }

    fn notify(&self) {
        self.0.notify()
    }
}


