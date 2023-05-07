
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;


pub fn event_queue<T>() -> (EventSender<T>, EventReceiver<T>) {
    let sender = EventSender {
        queue: Arc::new(Mutex::new(VecDeque::new())),
    };

    let receiver = EventReceiver {
        queue: sender.queue.clone(),
    };

    (sender, receiver)
}

pub struct EventSender<T> {
    queue: Arc<Mutex<VecDeque<T>>>,
}

impl<T> EventSender<T> {
    pub fn send(&self, event: T) {
        self.queue.lock().unwrap().push_back(event);
    }

    //pub fn send_at_instant(&self, instant: Instant, event: T) {
    //    self.queue.lock().unwrap().push_back((instant, event));
    //}
}

pub struct EventReceiver<T> {
    queue: Arc<Mutex<VecDeque<T>>>,
}

impl<T> EventReceiver<T> {
    pub fn receive(&self) -> Option<T> {
        self.queue.lock().unwrap().pop_front()
    }
}

