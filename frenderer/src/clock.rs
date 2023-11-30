use std::time::Instant;

const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];

/// A data structure storing a time accumulator and some parameters
/// for controlling the progression of game time.  This implementation
/// yields a fixed-timestep simulation loop following
/// [https://www.gafferongames.com/post/fix_your_timestep/](Fix Your
/// Timestep!) with optional improvements to smoothness and avoidance
/// of death spirals based on
/// [https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75](Tyler
/// Glaiel's blog post).
pub struct Clock {
    acc: f32,
    dt: f32,
    fudge_amount: f32,
    max_frames_per_tick: usize,
    last_t: Instant,
}

impl Clock {
    /// Creates a clock with the given target simulation frame rate.
    ///
    /// `fudge_amount`, if non-zero, will "fudge" time intervals to nearby standard frame rates (e.g., 15hz, 30hz, 60hz, 120hz, 144hz) to smooth out small differences under vertical sync.
    ///
    /// `max_frames_per_tick` limits the largest number of steps to simulate at once in order to avoid "death spirals" (where one slow frame causes multiple simulation steps, which delays the next frame, which then needs to make up for even more steps).
    pub fn new(dt: f32, fudge_amount: f32, max_frames_per_tick: usize) -> Self {
        Self {
            acc: 0.0,
            dt,
            fudge_amount,
            max_frames_per_tick,
            last_t: Instant::now(),
        }
    }
    /// Re-initialize the last-ticked time to the given instant and
    /// clear the accumulator.  This might be useful when a new game
    /// level is loaded or at some other interval to limit drift
    /// between the fudged frame times and the actual wall-clock time.
    pub fn set_now(&mut self, instant: Instant) {
        self.last_t = instant;
        self.acc = 0.0;
    }
    /// Tick the clock forward based on the time since it was last
    /// ticked.  Returns how many timesteps to simulate based on the
    /// elapsed time.
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
