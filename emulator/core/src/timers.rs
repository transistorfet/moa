
use std::fmt;
use std::time::Instant;

#[derive(Clone)]
pub struct AverageTimer {
    pub high: u32,
    pub average: f32,
    pub low: u32,
    pub events: u32,
    pub start: Option<Instant>,
}

impl Default for AverageTimer {
    fn default() -> AverageTimer {
        AverageTimer {
            high: 0,
            average: 0.0,
            low: u32::MAX,
            events: 0,
            start: None,
        }
    }
}

impl AverageTimer {
    pub fn start(&mut self) {
        //self.start = Some(Instant::now())
    }

    pub fn end(&mut self) {
        //let time = self.start.unwrap().elapsed().as_nanos() as u32;

        //self.events += 1;
        //if time > self.high {
        //    self.high = time;
        //}
        //if time < self.low {
        //    self.low = time;
        //}
        //self.average = (self.average + time as f32) / 2.0;
    }
}

impl fmt::Display for AverageTimer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "H: {:8} A: {:4} L: {:8} over {} events", self.high, self.average as u32, self.low, self.events)
    }
}


#[derive(Clone, Default)]
pub struct CpuTimer {
    pub decode: AverageTimer,
    pub execute: AverageTimer,
    pub cycle: AverageTimer,
}

impl fmt::Display for CpuTimer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Decode:  {}", self.decode)?;
        writeln!(f, "Execute: {}", self.execute)?;
        writeln!(f, "Cycle:   {}", self.cycle)?;
        Ok(())
    }
}

