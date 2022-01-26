 
#[derive(Clone)]
pub struct CircularBuffer<T> {
    pub inp: usize,
    pub out: usize,
    pub init: T,
    pub buffer: Vec<T>,
}

impl<T: Copy> CircularBuffer<T> {
    pub fn new(size: usize, init: T) -> Self {
        Self {
            inp: 0,
            out: 0,
            init,
            buffer: vec![init; size],
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.inp = 0;
        self.out = 0;
    }

    pub fn resize(&mut self, newlen: usize) {
        if self.buffer.len() != newlen {
            self.buffer = vec![self.init; newlen];
            self.clear();
        }
    }

    pub fn insert(&mut self, item: T) {
        let next = self.next_in();
        if next != self.out {
            self.buffer[self.inp] = item;
            self.inp = next;
        }
    }

    pub fn drop_next(&mut self, mut count: usize) {
        let avail = self.used_space();
        if count > avail {
            count = avail;
        }

        self.out += count;
        if self.out >= self.buffer.len() {
            self.out -= self.buffer.len();
        }
    }

    pub fn is_full(&self) -> bool {
        self.next_in() == self.out
    }

    pub fn used_space(&self) -> usize {
        if self.inp >= self.out {
            self.inp - self.out
        } else {
            self.buffer.len() - self.out + self.inp
        }
    }

    pub fn free_space(&self) -> usize {
    if self.out > self.inp {
        self.out - self.inp - 1
    } else {
        self.buffer.len() - self.inp + self.out - 1
        }
    }

    fn next_in(&self) -> usize {
        if self.inp + 1 < self.buffer.len() {
            self.inp + 1
        } else {
            0
        }
    }
}

impl<T: Copy> Iterator for CircularBuffer<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.out == self.inp {
            None
        } else {
            let value = self.buffer[self.out];
            self.out += 1;
            if self.out >= self.buffer.len() {
                self.out = 0;
            }
            Some(value)
        }
    }
}

