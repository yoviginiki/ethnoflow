//! Small mono reverb (Schroeder: 4 damped combs + 2 allpasses). Buffers are allocated
//! once at construction for the sample rate; `process` never allocates.

pub struct Reverb {
    combs: [Comb; 4],
    allpasses: [Allpass; 2],
    size: f32,
    damp: f32,
}

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    filter: f32,
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

// Freeverb-derived tunings at 44.1 kHz, scaled to the stream rate.
const COMB_MS: [f32; 4] = [25.3, 26.9, 28.9, 30.7];
const ALLPASS_MS: [f32; 2] = [12.6, 10.0];

impl Reverb {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let len = |ms: f32| ((ms * 0.001 * sr) as usize).max(2);
        Self {
            combs: COMB_MS.map(|ms| Comb {
                buf: vec![0.0; len(ms)],
                pos: 0,
                filter: 0.0,
            }),
            allpasses: ALLPASS_MS.map(|ms| Allpass {
                buf: vec![0.0; len(ms)],
                pos: 0,
            }),
            size: 0.5,
            damp: 0.4,
        }
    }

    /// `size` 0..1 (tail length), `damp` 0..1 (high-frequency loss).
    pub fn set(&mut self, size: f32, damp: f32) {
        self.size = size.clamp(0.0, 1.0);
        self.damp = damp.clamp(0.0, 0.99);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let feedback = 0.70 + 0.28 * self.size;
        let mut acc = 0.0f32;
        for c in self.combs.iter_mut() {
            let out = c.buf[c.pos];
            c.filter =
                super::super::params::undenormal(out * (1.0 - self.damp) + c.filter * self.damp);
            c.buf[c.pos] = x + c.filter * feedback;
            c.pos += 1;
            if c.pos >= c.buf.len() {
                c.pos = 0;
            }
            acc += out;
        }
        let mut y = acc * 0.25;
        for a in self.allpasses.iter_mut() {
            let b = a.buf[a.pos];
            let out = b - y;
            a.buf[a.pos] = super::super::params::undenormal(y + b * 0.5);
            a.pos += 1;
            if a.pos >= a.buf.len() {
                a.pos = 0;
            }
            y = out;
        }
        y
    }

    pub fn clear(&mut self) {
        for c in self.combs.iter_mut() {
            c.buf.fill(0.0);
            c.filter = 0.0;
        }
        for a in self.allpasses.iter_mut() {
            a.buf.fill(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impulse_decays_and_longer_size_rings_longer() {
        let tail = |size: f32| {
            let mut r = Reverb::new(48_000);
            r.set(size, 0.3);
            let mut energy_late = 0.0f32;
            for i in 0..48_000 {
                let y = r.process(if i == 0 { 1.0 } else { 0.0 });
                assert!(y.is_finite() && y.abs() < 4.0);
                if i > 24_000 {
                    energy_late += y * y;
                }
            }
            energy_late
        };
        let short = tail(0.0);
        let long = tail(1.0);
        assert!(long > short * 2.0, "short {short} long {long}");
        assert!(short < 1.0, "it decays: {short}");
    }
}
