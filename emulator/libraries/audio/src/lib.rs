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

    pub fn set_frequency(&mut self, frequency: f32) {
        let ratio = self.frequency / frequency;
        self.frequency = frequency;
        self.position = (self.position as f32 * ratio) as usize;
    }

    pub fn reset(&mut self) {
        self.position = 0;
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

    pub fn set_frequency(&mut self, frequency: f32) {
        let ratio = self.frequency / frequency;
        self.frequency = frequency;
        self.position = (self.position as f32 * ratio) as usize;
    }

    pub fn reset(&mut self) {
        self.position = 0;
    }
}

impl Iterator for SquareWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.position += 1;
        let samples_per_hz = self.sample_rate as f32 / self.frequency;
        let result = if (self.position as f32 % samples_per_hz) < (samples_per_hz / 2.0) {
            1.0
        } else {
            -1.0
        };
        Some(result)
    }
}



#[derive(Copy, Clone)]
pub enum SkewedSquareWaveStage {
    Rising,
    Positive,
    Falling,
    Negative,
}

#[derive(Clone)]
pub struct SkewedSquareWave {
    pub stage: SkewedSquareWaveStage,
    pub frequency: f32,
    pub skew: f32,
    pub sample_rate: usize,
    pub position: usize,
    pub sample: f32,
}

impl SkewedSquareWave {
    pub fn new(frequency: f32, sample_rate: usize) -> Self {
        Self {
            stage: SkewedSquareWaveStage::Rising,
            frequency,
            skew: 0.1,
            sample_rate,
            position: 0,
            sample: 0.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        let ratio = self.frequency / frequency;
        self.frequency = frequency;
        self.position = (self.position as f32 * ratio) as usize;
    }

    pub fn reset(&mut self) {
        self.stage = SkewedSquareWaveStage::Rising;
        self.position = 0;
    }
}

impl Iterator for SkewedSquareWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let samples_per_hz = self.sample_rate as f32 / self.frequency;
        self.position += 1;

        match self.stage {
            SkewedSquareWaveStage::Rising => {
                self.sample += self.skew;
                if self.sample >= 1.0 {
                    self.sample = 1.0;
                    self.stage = SkewedSquareWaveStage::Positive;
                }
            },
            SkewedSquareWaveStage::Positive => {
                if (self.position as f32 % samples_per_hz) >= (samples_per_hz / 2.0) {
                    self.stage = SkewedSquareWaveStage::Falling;
                }
            },
            SkewedSquareWaveStage::Falling => {
                self.sample -= self.skew;
                if self.sample <= -1.0 {
                    self.sample = -1.0;
                    self.stage = SkewedSquareWaveStage::Negative;
                }
            },
            SkewedSquareWaveStage::Negative => {
                if (self.position as f32 % samples_per_hz) < (samples_per_hz / 2.0) {
                    self.stage = SkewedSquareWaveStage::Rising;
                }
            },
        }
        Some(self.sample)
    }
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    (10.0_f32).powf(db / 20.0)
}
