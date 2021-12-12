
use std::f32::consts::PI;


#[derive(Clone)]
pub struct SineWave {
    pub frequency: f32,
    pub sample_rate: usize,
    pub position: usize,
}

impl SineWave {
    pub fn new(frequency: f32, sample_rate: usize) -> Self {
        Self {
            frequency,
            sample_rate,
            position: 0,
        }
    }
}

impl Iterator for SineWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.position += 1;
        let result = (2.0 * PI * self.frequency * self.position as f32 / (self.sample_rate as f32)).sin();
        Some(result)
    }
}

#[derive(Clone)]
pub struct SquareWave {
    pub frequency: f32,
    pub sample_rate: usize,
    pub position: usize,
}

impl SquareWave {
    pub fn new(frequency: f32, sample_rate: usize) -> Self {
        Self {
            frequency,
            sample_rate,
            position: 0,
        }
    }
}

impl Iterator for SquareWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.position += 1;
        let samples_per_hz = self.sample_rate as f32 / self.frequency;
        let result = if (self.position as f32 % samples_per_hz) < (samples_per_hz / 2.0) { 1.0 } else { -1.0 };
        Some(result)
    }
}

