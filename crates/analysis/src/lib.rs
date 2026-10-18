//! Music DNA analyzer (Program P2, Phase B foundation). Everything here is offline and
//! pure: it takes mono samples and returns time-stamped curves, tempo candidates with the
//! half/double ambiguity made explicit, a beat grid, novelty-based boundary proposals and
//! *hypotheses* about section roles with confidence and evidence. Labels are never
//! derived from a section's position alone (spec §3.3), and nothing here claims to be
//! ground truth: the user corrects everything in the map.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

pub const ANALYZER_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Analysis sample rate the caller resampled to (22 050 recommended).
    pub sample_rate: u32,
    pub window: usize,
    pub hop: usize,
    /// Tempo search range.
    pub bpm_lo: f64,
    pub bpm_hi: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sample_rate: 22_050,
            window: 2048,
            hop: 512,
            bpm_lo: 60.0,
            bpm_hi: 200.0,
        }
    }
}

/// Per-frame feature curves, all the same length; time of frame i = t0_s + i × hop_s
/// (t0_s = half the analysis window, so a frame is stamped at its centre).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureCurves {
    pub hop_s: f64,
    #[serde(default)]
    pub t0_s: f64,
    pub rms: Vec<f32>,
    pub centroid_hz: Vec<f32>,
    /// Half-wave rectified spectral flux (onset strength).
    pub flux: Vec<f32>,
    pub low_band: Vec<f32>,
    /// 12 pitch classes per frame, flattened (frame-major).
    pub chroma: Vec<f32>,
    /// 12 timbre coefficients per frame (DCT of 24 log bands), flattened.
    pub timbre: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoCandidate {
    pub bpm: f64,
    /// 0..1 relative strength of the autocorrelation peak.
    pub confidence: f64,
    /// "peak" = found directly; "half" / "double" = listed because of the ambiguity.
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    pub seconds: f64,
    /// 0..1 novelty prominence.
    pub confidence: f64,
    /// "coarse" | "medium" | "fine" — the kernel that found it (coarse ⊂ medium ⊂ fine).
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentHypothesis {
    pub start_s: f64,
    pub end_s: f64,
    /// Mean energy 0..1 relative to the loudest segment.
    pub energy: f64,
    /// Onset density 0..1 relative to the densest segment.
    pub density: f64,
    /// Low-band share 0..1 (bass / kick presence).
    pub low: f64,
    /// Proposed role or "unknown", with a confidence and the evidence behind it.
    pub role: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    /// Index of the most similar other segment (repetition), if any.
    pub similar_to: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceAnalysis {
    pub analyzer_version: u32,
    pub sample_rate: u32,
    pub duration_s: f64,
    pub curves: FeatureCurves,
    pub tempo: Vec<TempoCandidate>,
    pub beats_s: Vec<f64>,
    pub boundaries: Vec<Boundary>,
    pub segments: Vec<SegmentHypothesis>,
}

// ---------- STFT and features ----------

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect()
}

/// Magnitude spectra (frames × bins) for `samples`.
pub fn stft(samples: &[f32], window: usize, hop: usize) -> Vec<Vec<f32>> {
    if samples.len() < window {
        return Vec::new();
    }
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(window);
    let win = hann(window);
    let mut buf = vec![Complex::new(0.0f32, 0.0); window];
    let mut scratch = vec![Complex::new(0.0f32, 0.0); fft.get_inplace_scratch_len()];
    let frames = (samples.len() - window) / hop + 1;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = f * hop;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = Complex::new(samples[start + i] * win[i], 0.0);
        }
        fft.process_with_scratch(&mut buf, &mut scratch);
        out.push(buf[..window / 2 + 1].iter().map(|c| c.norm()).collect());
    }
    out
}

pub fn features(samples: &[f32], opts: &Options) -> FeatureCurves {
    let sr = opts.sample_rate as f32;
    let spec = stft(samples, opts.window, opts.hop);
    let bins = opts.window / 2 + 1;
    let bin_hz = sr / opts.window as f32;
    let mut c = FeatureCurves {
        hop_s: opts.hop as f64 / sr as f64,
        t0_s: opts.window as f64 / 2.0 / sr as f64,
        ..Default::default()
    };
    // 24 log-spaced bands 60 Hz .. sr/2 for the timbre coefficients.
    let nb = 24usize;
    let f_lo = 60.0f32;
    let f_hi = sr / 2.0;
    let band_edges: Vec<f32> = (0..=nb)
        .map(|k| f_lo * (f_hi / f_lo).powf(k as f32 / nb as f32))
        .collect();
    let mut prev: Vec<f32> = vec![0.0; bins];
    for (f, mag) in spec.iter().enumerate() {
        let start = f * opts.hop;
        let frame = &samples[start..start + opts.window];
        let rms = (frame.iter().map(|x| x * x).sum::<f32>() / opts.window as f32).sqrt();
        c.rms.push(rms);
        let total: f32 = mag.iter().sum::<f32>().max(1e-9);
        let centroid = mag
            .iter()
            .enumerate()
            .map(|(i, m)| i as f32 * bin_hz * m)
            .sum::<f32>()
            / total;
        c.centroid_hz.push(centroid);
        let flux: f32 = mag
            .iter()
            .zip(prev.iter())
            .map(|(m, p)| (m - p).max(0.0))
            .sum::<f32>()
            / (bins as f32).sqrt();
        c.flux.push(flux);
        prev.clone_from(mag);
        let low_bins = ((150.0 / bin_hz) as usize).max(1);
        let low: f32 = mag[..low_bins.min(bins)].iter().sum::<f32>() / total;
        c.low_band.push(low);
        // Chroma: sum magnitudes by pitch class from 60 Hz to 5 kHz.
        let mut chroma = [0.0f32; 12];
        for (i, m) in mag.iter().enumerate().skip(1) {
            let hz = i as f32 * bin_hz;
            if !(60.0..5000.0).contains(&hz) {
                continue;
            }
            let midi = 69.0 + 12.0 * (hz / 440.0).log2();
            let pc = ((midi.round() as i32).rem_euclid(12)) as usize;
            chroma[pc] += m;
        }
        let cs: f32 = chroma.iter().sum::<f32>().max(1e-9);
        c.chroma.extend(chroma.iter().map(|v| v / cs));
        // Timbre: log energy in 24 bands → 12 DCT-II coefficients (skipping the DC).
        let mut bands = vec![0.0f32; nb];
        for (i, m) in mag.iter().enumerate() {
            let hz = i as f32 * bin_hz;
            if hz < f_lo {
                continue;
            }
            let k = band_edges
                .windows(2)
                .position(|w| hz >= w[0] && hz < w[1])
                .unwrap_or(nb - 1);
            bands[k] += m * m;
        }
        let logb: Vec<f32> = bands.iter().map(|e| (e + 1e-9).ln()).collect();
        for k in 1..=12 {
            let v: f32 = logb
                .iter()
                .enumerate()
                .map(|(n, x)| {
                    x * ((std::f32::consts::PI / nb as f32) * (n as f32 + 0.5) * k as f32).cos()
                })
                .sum();
            c.timbre.push(v / nb as f32);
        }
    }
    c
}

// ---------- tempo and beats ----------

/// Tempo candidates from the autocorrelation of the onset strength, with the half/double
/// relations listed explicitly so the UI can show "174 or 87" instead of guessing.
pub fn tempo_candidates(
    curves: &FeatureCurves,
    bpm_lo: f64,
    bpm_hi: f64,
    max: usize,
) -> Vec<TempoCandidate> {
    let n = curves.flux.len();
    if n < 8 || curves.hop_s <= 0.0 {
        return Vec::new();
    }
    // Detrended, non-negative onset strength.
    let mean = curves.flux.iter().sum::<f32>() / n as f32;
    let env: Vec<f32> = curves.flux.iter().map(|v| (v - mean).max(0.0)).collect();
    let lag_min = ((60.0 / bpm_hi) / curves.hop_s).floor().max(1.0) as usize;
    let lag_max = ((60.0 / bpm_lo) / curves.hop_s).ceil() as usize;
    if lag_max >= n || lag_min >= lag_max {
        return Vec::new();
    }
    let energy: f32 = env.iter().map(|v| v * v).sum::<f32>().max(1e-9);
    let ac: Vec<f32> = (lag_min..=lag_max)
        .map(|lag| {
            let s: f32 = env[lag..].iter().zip(env.iter()).map(|(a, b)| a * b).sum();
            s / energy
        })
        .collect();
    // Local peaks, refined by parabolic interpolation; scored by height.
    let mut peaks: Vec<(f64, f64)> = Vec::new();
    for i in 1..ac.len().saturating_sub(1) {
        if ac[i] > ac[i - 1] && ac[i] >= ac[i + 1] && ac[i] > 0.05 {
            let (a, b, c) = (ac[i - 1] as f64, ac[i] as f64, ac[i + 1] as f64);
            let denom = a - 2.0 * b + c;
            let delta = if denom.abs() > 1e-9 {
                0.5 * (a - c) / denom
            } else {
                0.0
            };
            let lag = (lag_min + i) as f64 + delta;
            peaks.push((60.0 / (lag * curves.hop_s), b));
        }
    }
    peaks.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
    let top = peaks.first().map(|p| p.1).unwrap_or(1.0).max(1e-9);
    let mut out: Vec<TempoCandidate> = Vec::new();
    for (bpm, h) in peaks.into_iter().take(max) {
        if out.iter().any(|c| (c.bpm - bpm).abs() < 1.5) {
            continue;
        }
        out.push(TempoCandidate {
            bpm: (bpm * 10.0).round() / 10.0,
            confidence: (h / top).clamp(0.0, 1.0),
            relation: "peak".into(),
        });
    }
    // The ambiguity, stated: every peak's half and double inside the range.
    let found: Vec<(f64, f64)> = out.iter().map(|c| (c.bpm, c.confidence)).collect();
    for (bpm, conf) in found {
        for (factor, rel) in [(0.5, "half"), (2.0, "double")] {
            let b = bpm * factor;
            if b >= bpm_lo && b <= bpm_hi && !out.iter().any(|c| (c.bpm - b).abs() < 1.5) {
                out.push(TempoCandidate {
                    bpm: (b * 10.0).round() / 10.0,
                    confidence: (conf * 0.6).clamp(0.0, 1.0),
                    relation: rel.into(),
                });
            }
        }
    }
    out.sort_by(|x, y| y.confidence.partial_cmp(&x.confidence).unwrap());
    out
}

/// Beat times for a tempo: dynamic programming over the onset strength (Ellis 2007), so
/// beats fall on strong onsets while keeping the period.
pub fn beats(curves: &FeatureCurves, bpm: f64) -> Vec<f64> {
    let n = curves.flux.len();
    if n == 0 || bpm <= 0.0 {
        return Vec::new();
    }
    let period = (60.0 / bpm / curves.hop_s).max(1.0);
    let tightness = 100.0f64;
    let max_f = curves.flux.iter().cloned().fold(0.0f32, f32::max).max(1e-9) as f64;
    let onset: Vec<f64> = curves.flux.iter().map(|v| *v as f64 / max_f).collect();
    let mut score = vec![0.0f64; n];
    let mut back = vec![usize::MAX; n];
    let lo = (period * 0.5).floor() as usize;
    let hi = (period * 2.0).ceil() as usize;
    for i in 0..n {
        let mut best = 0.0;
        let mut bi = usize::MAX;
        if i >= lo.max(1) {
            let from = i.saturating_sub(hi);
            let to = i - lo.max(1);
            for (j, sj) in score.iter().enumerate().take(to + 1).skip(from) {
                let d = (i - j) as f64;
                let pen = -tightness * ((d / period).ln()).powi(2);
                let v = sj + pen;
                if bi == usize::MAX || v > best {
                    best = v;
                    bi = j;
                }
            }
        }
        score[i] = onset[i] + if bi == usize::MAX { 0.0 } else { best };
        back[i] = bi;
    }
    // Backtrack from the best frame in the last period.
    let start = n.saturating_sub(period.ceil() as usize);
    let mut i = (start..n)
        .max_by(|a, b| score[*a].partial_cmp(&score[*b]).unwrap())
        .unwrap_or(n - 1);
    let mut out = Vec::new();
    while i != usize::MAX {
        out.push(curves.t0_s + i as f64 * curves.hop_s);
        i = back[i];
    }
    out.reverse();
    out
}

// ---------- structure ----------

/// Structure works on a coarser grid than the features (default 0.25 s): each block is
/// the mean of its frames' timbre, chroma, energy and low-band share. That keeps the
/// self-similarity kernels small (a 60 s song is 240 blocks) and is what the ear hears
/// as "a part" anyway.
pub const STRUCTURE_STEP_S: f64 = 0.25;

pub struct StructureGrid {
    pub step_s: f64,
    /// Time of block 0 (the features' t0_s).
    pub t0_s: f64,
    pub vecs: Vec<Vec<f32>>,
}

pub fn structure_grid(curves: &FeatureCurves, step_s: f64) -> StructureGrid {
    let n = curves.rms.len();
    let per = ((step_s / curves.hop_s).round() as usize).max(1);
    let blocks = n.div_ceil(per);
    let mut vecs = Vec::with_capacity(blocks);
    for b in 0..blocks {
        let (a, z) = (b * per, ((b + 1) * per).min(n));
        let mut v = vec![0.0f32; 26];
        for i in a..z {
            for k in 0..12 {
                v[k] += curves.timbre[i * 12 + k];
                v[12 + k] += curves.chroma[i * 12 + k] * 4.0;
            }
            v[24] += curves.rms[i] * 8.0;
            v[25] += curves.low_band[i] * 4.0;
        }
        let cnt = (z - a).max(1) as f32;
        for x in v.iter_mut() {
            *x /= cnt;
        }
        vecs.push(v);
    }
    StructureGrid {
        step_s: per as f64 * curves.hop_s,
        t0_s: curves.t0_s,
        vecs,
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-9 || nb < 1e-9 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Novelty (Foote): a checkerboard kernel of `half` blocks slid along the diagonal of the
/// self-similarity matrix of the grid vectors. The band of the matrix is computed once
/// (block i against i+1 ..= i+2·half); each kernel then sums its two "same" squares and
/// its "cross" square from that band.
pub fn novelty(grid: &[Vec<f32>], half: usize) -> Vec<f32> {
    let n = grid.len();
    let mut out = vec![0.0f32; n];
    if half == 0 || n < 2 * half + 1 {
        return out;
    }
    let width = 2 * half;
    let band: Vec<Vec<f32>> = (0..n)
        .map(|i| {
            (1..=width)
                .map(|d| {
                    if i + d < n {
                        cosine(&grid[i], &grid[i + d])
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    let sim = |i: usize, j: usize| -> f32 {
        if i == j {
            1.0
        } else {
            let (lo, hi) = if i < j { (i, j) } else { (j, i) };
            band[lo][hi - lo - 1]
        }
    };
    for (i, o) in out.iter_mut().enumerate().take(n - half).skip(half) {
        let mut same = 0.0f32;
        let mut cross = 0.0f32;
        for a in 0..half {
            for b in 0..half {
                same += sim(i - half + a, i - half + b) + sim(i + a, i + b);
                cross += 2.0 * sim(i - half + a, i + b);
            }
        }
        *o = ((same - cross) / (2.0 * (half * half) as f32)).max(0.0);
    }
    out
}

fn peaks_with_prominence(x: &[f32], min_gap: usize) -> Vec<(usize, f32)> {
    let n = x.len();
    let mut out = Vec::new();
    let max = x.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    let mut i = 1;
    while i + 1 < n {
        if x[i] > x[i - 1] && x[i] >= x[i + 1] {
            // Prominence: height above the lowest valley within min_gap on either side.
            let left = x[i.saturating_sub(min_gap)..i]
                .iter()
                .cloned()
                .fold(x[i], f32::min);
            let right = x[i + 1..(i + 1 + min_gap).min(n)]
                .iter()
                .cloned()
                .fold(x[i], f32::min);
            let prom = (x[i] - left.max(right)) / max;
            if prom > 0.15 {
                out.push((i, prom.min(1.0)));
                i += min_gap;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Boundary proposals at three resolutions (kernel ≈ 8 / 4 / 2 s), merged with the
/// coarsest resolution that found each.
pub fn boundaries(curves: &FeatureCurves) -> Vec<Boundary> {
    if curves.hop_s <= 0.0 {
        return Vec::new();
    }
    boundaries_on(&structure_grid(curves, STRUCTURE_STEP_S))
}

pub fn boundaries_on(grid: &StructureGrid) -> Vec<Boundary> {
    let mut out: Vec<Boundary> = Vec::new();
    for (name, secs, weight) in [
        ("coarse", 8.0, 1.0),
        ("medium", 4.0, 0.8),
        ("fine", 2.0, 0.5),
    ] {
        let half = ((secs / grid.step_s) as usize).max(2);
        let nov = novelty(&grid.vecs, half);
        for (i, prom) in peaks_with_prominence(&nov, half) {
            let t = grid.t0_s + i as f64 * grid.step_s;
            if out.iter().any(|b| (b.seconds - t).abs() < secs * 0.5) {
                continue;
            }
            out.push(Boundary {
                seconds: t,
                confidence: (prom as f64 * weight).min(1.0),
                resolution: name.into(),
            });
        }
    }
    out.sort_by(|a, b| a.seconds.partial_cmp(&b.seconds).unwrap());
    out
}

struct RawSegment {
    start: f64,
    end: f64,
    energy: f64,
    density: f64,
    low: f64,
    centroid: Vec<f32>,
}

/// Confidence a boundary needs to cut a segment (fine-kernel peaks rarely reach it).
pub const SEGMENT_MIN_CONFIDENCE: f64 = 0.3;

/// Segment statistics and role hypotheses from the features of each segment (never from
/// its index): energy, onset density and low-band share relative to the whole track.
pub fn segments(
    curves: &FeatureCurves,
    bounds: &[Boundary],
    duration_s: f64,
) -> Vec<SegmentHypothesis> {
    let mut edges: Vec<f64> = vec![0.0];
    edges.extend(
        bounds
            .iter()
            .filter(|b| b.confidence >= SEGMENT_MIN_CONFIDENCE && b.resolution != "fine")
            .map(|b| b.seconds),
    );
    edges.push(duration_s);
    edges.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let n = curves.rms.len();
    if n == 0 || edges.len() < 2 {
        return Vec::new();
    }
    let grid = structure_grid(curves, STRUCTURE_STEP_S);
    let frame = |t: f64| (((t - curves.t0_s).max(0.0) / curves.hop_s) as usize).min(n - 1);
    let mean = |v: &[f32], a: usize, b: usize| -> f64 {
        let b = b.max(a + 1).min(n);
        v[a..b].iter().map(|x| *x as f64).sum::<f64>() / (b - a) as f64
    };
    let flux_mean_all = mean(&curves.flux, 0, n).max(1e-9);
    let mut raw: Vec<RawSegment> = Vec::new();
    for w in edges.windows(2) {
        let (a, b) = (frame(w[0]), frame(w[1]));
        let bz = b.max(a + 1).min(n);
        let energy = mean(&curves.rms, a, b);
        let density = curves.flux[a..bz]
            .iter()
            .filter(|v| **v as f64 > flux_mean_all)
            .count() as f64
            / (bz - a) as f64;
        let low = mean(&curves.low_band, a, b);
        let block = |t: f64| ((t - grid.t0_s).max(0.0) / grid.step_s) as usize;
        let ga = block(w[0]).min(grid.vecs.len());
        let gb = block(w[1]).clamp(ga + 1, grid.vecs.len().max(ga + 1));
        let mut centroid = vec![0.0f32; 26];
        for v in grid.vecs.iter().take(gb.min(grid.vecs.len())).skip(ga) {
            for (k, x) in v.iter().enumerate() {
                centroid[k] += x;
            }
        }
        raw.push(RawSegment {
            start: w[0],
            end: w[1],
            energy,
            density,
            low,
            centroid,
        });
    }
    let e_max = raw
        .iter()
        .map(|r| r.energy)
        .fold(0.0f64, f64::max)
        .max(1e-9);
    let d_max = raw
        .iter()
        .map(|r| r.density)
        .fold(0.0f64, f64::max)
        .max(1e-9);
    let mut out: Vec<SegmentHypothesis> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        let energy = r.energy / e_max;
        let density = r.density / d_max;
        let low = r.low;
        let similar_to = raw
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(j, o)| (j, cosine(&r.centroid, &o.centroid)))
            .filter(|(_, c)| *c > 0.97)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(j, _)| j);
        let mut evidence = Vec::new();
        let (role, confidence) = if energy > 0.8 && density > 0.6 && low > 0.25 {
            evidence.push(format!(
                "energy {:.0}%, onsets {:.0}%, low band {:.0}%",
                energy * 100.0,
                density * 100.0,
                low * 100.0
            ));
            ("drop", 0.6)
        } else if energy < 0.35 && density < 0.4 {
            evidence.push(format!(
                "quiet ({:.0}% energy) and sparse ({:.0}% onsets)",
                energy * 100.0,
                density * 100.0
            ));
            ("break", 0.5)
        } else if energy > 0.5 && density > 0.5 && low < 0.15 {
            evidence.push(format!("busy but little low band ({:.0}%)", low * 100.0));
            ("build", 0.4)
        } else {
            evidence.push("no clear profile".into());
            ("unknown", 0.2)
        };
        if let Some(j) = similar_to {
            evidence.push(format!("repeats segment {}", j + 1));
        }
        out.push(SegmentHypothesis {
            start_s: r.start,
            end_s: r.end,
            energy,
            density,
            low,
            role: role.into(),
            confidence,
            evidence,
            similar_to,
        });
    }
    out
}

/// The whole pipeline on mono samples at `opts.sample_rate`.
pub fn analyze(samples: &[f32], opts: &Options) -> ReferenceAnalysis {
    let curves = features(samples, opts);
    let duration_s = samples.len() as f64 / opts.sample_rate as f64;
    let tempo = tempo_candidates(&curves, opts.bpm_lo, opts.bpm_hi, 6);
    let beats_s = tempo
        .first()
        .map(|t| beats(&curves, t.bpm))
        .unwrap_or_default();
    let bounds = boundaries(&curves);
    let segs = segments(&curves, &bounds, duration_s);
    ReferenceAnalysis {
        analyzer_version: ANALYZER_VERSION,
        sample_rate: opts.sample_rate,
        duration_s,
        curves,
        tempo,
        beats_s,
        boundaries: bounds,
        segments: segs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_track(sr: u32, bpm: f64, seconds: f64, seed: u32) -> Vec<f32> {
        let n = (sr as f64 * seconds) as usize;
        let mut out = vec![0.0f32; n];
        let period = (60.0 / bpm * sr as f64) as usize;
        let mut rng = seed;
        let mut i = 0;
        while i < n {
            // A short decaying noise burst (a drum hit), slightly louder every 4th.
            let len = (sr as usize) / 40;
            for k in 0..len.min(n - i) {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                let noise = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let env = (1.0 - k as f32 / len as f32).powi(2);
                out[i + k] += noise * env * 0.6;
            }
            i += period;
        }
        out
    }

    fn opts() -> Options {
        Options {
            sample_rate: 22_050,
            window: 1024,
            hop: 256,
            ..Options::default()
        }
    }

    #[test]
    fn tempo_of_a_click_track_is_found_with_its_half_and_double_listed() {
        let x = click_track(22_050, 174.0, 20.0, 7);
        let a = analyze(&x, &opts());
        let top = &a.tempo[0];
        assert!(
            (top.bpm - 174.0).abs() < 2.0,
            "top {} ({})",
            top.bpm,
            top.relation
        );
        assert!(
            a.tempo.iter().any(|t| (t.bpm - 87.0).abs() < 2.0),
            "87 listed: {:?}",
            a.tempo
        );
        // Beats land on the hits: median distance to the nearest hit under 25 ms.
        let period = 60.0 / 174.0;
        let mut errs: Vec<f64> = a
            .beats_s
            .iter()
            .map(|b| ((b / period).round() * period - b).abs())
            .collect();
        errs.sort_by(|p, q| p.partial_cmp(q).unwrap());
        assert!(
            errs[errs.len() / 2] < 0.025,
            "beat error {}",
            errs[errs.len() / 2]
        );
    }

    #[test]
    fn boundaries_follow_the_music_not_the_index() {
        let sr = 22_050;
        // 20 s quiet low noise, 20 s bright busy hits, 20 s quiet low noise again.
        let mut x = Vec::new();
        let mut rng = 3u32;
        let mut noise = |amp: f32| {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (rng as f32 / u32::MAX as f32 * 2.0 - 1.0) * amp
        };
        let lp = |v: &mut Vec<f32>, amp: f32, n: usize, noise: &mut dyn FnMut(f32) -> f32| {
            let mut y = 0.0f32;
            for _ in 0..n {
                y += (noise(amp) - y) * 0.02; // dark
                v.push(y);
            }
        };
        lp(&mut x, 0.3, 20 * sr as usize, &mut noise);
        x.extend(click_track(sr, 160.0, 20.0, 11));
        lp(&mut x, 0.3, 20 * sr as usize, &mut noise);
        let a = analyze(&x, &opts());
        let near = |t: f64| {
            a.boundaries
                .iter()
                .any(|b| (b.seconds - t).abs() < 1.5 && b.confidence >= 0.3)
        };
        assert!(near(20.0), "boundary near 20 s: {:?}", a.boundaries);
        assert!(near(40.0), "boundary near 40 s: {:?}", a.boundaries);
        // Density follows the music, not the index: dense inside 20..40 s, sparse outside.
        assert!(
            a.segments.len() >= 3 && a.segments.len() <= 6,
            "{:?}",
            a.segments
        );
        for seg in &a.segments {
            let mid = (seg.start_s + seg.end_s) / 2.0;
            if mid > 21.0 && mid < 39.0 {
                assert!(seg.density > 0.8, "{seg:?}");
            } else if !(19.0..=41.0).contains(&mid) {
                assert!(seg.density < 0.4, "{seg:?}");
                assert_ne!(seg.role, "drop");
            }
        }
        // Repetition: the last quiet segment resembles a segment from the first quiet part.
        let last = a.segments.last().unwrap();
        let j = last
            .similar_to
            .expect("last segment repeats an earlier one");
        assert!(
            a.segments[j].end_s <= 21.0,
            "{last:?} ~ {:?}",
            a.segments[j]
        );
    }

    #[test]
    fn empty_and_short_inputs_do_not_panic_and_curves_align() {
        let a = analyze(&[], &opts());
        assert!(a.tempo.is_empty() && a.boundaries.is_empty() && a.segments.is_empty());
        let x = click_track(22_050, 120.0, 3.0, 1);
        let a = analyze(&x, &opts());
        let n = a.curves.rms.len();
        assert_eq!(a.curves.flux.len(), n);
        assert_eq!(a.curves.chroma.len(), n * 12);
        assert_eq!(a.curves.timbre.len(), n * 12);
        assert!(a.curves.rms.iter().all(|v| v.is_finite()));
        let text = serde_json::to_string(&a).unwrap();
        let back: ReferenceAnalysis = serde_json::from_str(&text).unwrap();
        assert_eq!(back.curves.rms.len(), n);
    }
}
