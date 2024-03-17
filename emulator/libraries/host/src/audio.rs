#[derive(Copy, Clone, Default)]
pub struct Sample(pub f32, pub f32);

impl Sample {
    pub fn new(left: f32, right: f32) -> Self {
        Self(left, right)
    }
}

#[derive(Clone, Default)]
pub struct AudioFrame {
    pub sample_rate: usize,
    pub data: Vec<Sample>,
}

impl AudioFrame {
    pub fn new(sample_rate: usize, data: Vec<Sample>) -> Self {
        AudioFrame {
            sample_rate,
            data,
        }
    }
}
