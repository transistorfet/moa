
use std::fmt;
use std::time::Instant;

pub struct AverageTimer {
    pub high: u32,
    pub average: f32,
    pub low: u32,
    pub events: u32,
}

impl AverageTimer {
    pub fn new() -> AverageTimer {
        AverageTimer {
            high: 0,
            average: 0.0,
            low: u32::MAX,
            events: 0,
        }
    }

    pub fn start(&self) -> Instant {
        Instant::now()
    }

    pub fn end(&mut self, timer: Instant) {
        let time = timer.elapsed().as_nanos() as u32;

        self.events += 1;
        if time > self.high {
            self.high = time;
        }
        if time < self.low {
            self.low = time;
        }
        self.average = (self.average + time as f32) / 2.0;
    }
}

impl fmt::Display for AverageTimer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "H: {:8} A: {:4} L: {:8} over {} events", self.high, self.average as u32, self.low, self.events)
    }
}


pub struct CpuTimer {
    pub decode: AverageTimer,
    pub execute: AverageTimer,
    pub cycle: AverageTimer,
}

impl CpuTimer {
    pub fn new() -> CpuTimer {
        CpuTimer {
            decode: AverageTimer::new(),
            execute: AverageTimer::new(),
            cycle: AverageTimer::new(),
        }
    }
}

impl fmt::Display for CpuTimer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decode:  {}\n", self.decode)?;
        write!(f, "Execute: {}\n", self.execute)?;
        write!(f, "Cycle:   {}\n", self.cycle)?;
        Ok(())
    }
}


