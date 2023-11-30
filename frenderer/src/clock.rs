use std::time::Instant;

const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];

pub struct Clock {
    acc: f32,
    dt: f32,
    fudge_amount: f32,
    max_frames_per_tick: usize,
    last_t: Instant,
}

impl Clock {
    pub fn new(dt: f32, fudge_amount: f32, max_frames_per_tick: usize) -> Self {
        Self {
            acc: 0.0,
            dt,
            fudge_amount,
            max_frames_per_tick,
            last_t: Instant::now(),
        }
    }
    pub fn set_now(&mut self, instant: Instant) {
        self.last_t = instant;
    }
    pub fn tick(&mut self) -> usize {
        // compute elapsed time since last frame
        let mut elapsed = self.last_t.elapsed().as_secs_f32();
        // println!("{elapsed}");
        // snap time to nearby vsync framerate
        TIME_SNAPS.iter().for_each(|s| {
            if (elapsed - 1.0 / s).abs() < self.fudge_amount {
                elapsed = 1.0 / s;
            }
        });
        // Death spiral prevention
        if elapsed > (self.max_frames_per_tick as f32 * self.dt) {
            self.acc = 0.0;
            elapsed = self.dt;
        }
        self.acc += elapsed;
        self.last_t = std::time::Instant::now();
        // While we have time to spend

        let steps = (self.acc / self.dt) as usize;
        self.acc -= steps as f32 * self.dt;
        steps
    }
}
