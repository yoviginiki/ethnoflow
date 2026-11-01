//! ethnoflow-nyquist — runs Audacity Nyquist plug-ins (`.ny`, LISP syntax) on a selection.
//!
//! What is supported (A9, 2026-11-01): the plug-in header (`;control` lines become UI),
//! the XLISP core (defun / let / setq / setf / if / cond / case / do / dolist / dotimes /
//! loop / return, lists, strings, `format`), property lists (`*track*`, `*selection*`,
//! `*project*` as Audacity 3.x exposes them) and the common sound primitives on fixed arrays.
//! Not supported: lazy streaming (`snd-fetch` loops), SAL syntax, `*scratch*` persistence,
//! files and GUI. A plug-in that needs them stops with a clear message.

pub mod eval;
pub mod plugin;
pub mod reader;
pub mod sound;
pub mod value;

use std::cell::RefCell;
use std::rc::Rc;

pub use eval::{Interp, NyError};
pub use plugin::{parse_header, Control, ControlKind, PluginHeader};
use value::{Sound, Value};

/// What a plug-in produced.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginOutput {
    /// New audio for the selection (one Vec per channel, same rate as the input).
    Audio(Vec<Vec<f32>>),
    /// A message (analyze plug-ins, or an error string returned by the plug-in).
    Message(String),
    /// Labels from an analyze plug-in (times relative to the selection start).
    Labels(Vec<Label>),
    /// NIL / no output.
    Nothing,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Label {
    pub start: f64,
    pub end: Option<f64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub output: PluginOutput,
    /// Everything the plug-in printed (`print`, `format t`).
    pub printed: String,
}

/// A control value as the UI sends it: numbers for int/float/choice/time, strings otherwise.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlValue {
    Number(f64),
    Text(String),
}

/// Run a plug-in's code on `channels` at `sr`, with the controls bound to the given values
/// (missing controls take their header default). `selection_start` is the selection's
/// start time in the track in seconds (for `*selection*` / `*track*` properties).
pub fn run_plugin(
    header: &PluginHeader,
    values: &[(String, ControlValue)],
    channels: &[Vec<f32>],
    sr: f64,
    selection_start: f64,
) -> Result<RunResult, String> {
    if header.codetype.starts_with("sal") {
        return Err("SAL plug-ins are not supported yet (only LISP syntax)".into());
    }
    // Deeply recursive plug-ins (list builders) need more stack than a default thread has.
    let header = header.clone();
    let values = values.to_vec();
    let channels = channels.to_vec();
    std::thread::Builder::new()
        .name("nyquist".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || run_plugin_here(&header, &values, &channels, sr, selection_start))
        .map_err(|e| format!("cannot start the plug-in thread: {e}"))?
        .join()
        .map_err(|_| "the plug-in crashed".to_string())?
}

fn run_plugin_here(
    header: &PluginHeader,
    values: &[(String, ControlValue)],
    channels: &[Vec<f32>],
    sr: f64,
    selection_start: f64,
) -> Result<RunResult, String> {
    let mut it = Interp::new(sr);
    for c in &header.controls {
        let v = values
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&c.var))
            .map(|(_, v)| v.clone());
        let bound = match (&c.kind, v) {
            (
                ControlKind::Int | ControlKind::IntText | ControlKind::Choice,
                Some(ControlValue::Number(n)),
            ) => Value::Int(n.round() as i64),
            (ControlKind::Int | ControlKind::IntText | ControlKind::Choice, None) => {
                Value::Int(c.initial.round() as i64)
            }
            (
                ControlKind::Float | ControlKind::FloatText | ControlKind::Time,
                Some(ControlValue::Number(n)),
            ) => Value::Float(n),
            (ControlKind::Float | ControlKind::FloatText | ControlKind::Time, None) => {
                Value::Float(c.initial)
            }
            (_, Some(ControlValue::Text(s))) => Value::Str(s),
            (_, Some(ControlValue::Number(n))) => Value::Float(n),
            (_, None) => Value::Str(c.default_text.clone()),
        };
        it.env.define(&c.var, bound);
    }
    let len = channels.first().map(|c| c.len()).unwrap_or(0);
    let dur = len as f64 / sr;
    let track = match channels.len() {
        0 => Value::Nil,
        1 => Value::Sound(Rc::new(Sound::new(sr, channels[0].clone()))),
        _ => Value::Array(Rc::new(RefCell::new(
            channels
                .iter()
                .map(|c| Value::Sound(Rc::new(Sound::new(sr, c.clone()))))
                .collect(),
        ))),
    };
    it.env.define("*track*", track.clone());
    it.env.define("s", track);
    it.env.define("len", Value::Int(len as i64));
    it.env.define("*sound-srate*", Value::Float(sr));
    it.put_prop(
        "*track*",
        "channels",
        Value::Int(channels.len().max(1) as i64),
    );
    it.put_prop("*track*", "rate", Value::Float(sr));
    it.put_prop("*track*", "start-time", Value::Float(0.0));
    it.put_prop("*track*", "end-time", Value::Float(dur));
    it.put_prop("*track*", "name", Value::Str("track".into()));
    it.put_prop("*track*", "view", Value::Str("Waveform".into()));
    it.put_prop("*track*", "type", Value::Str("wave".into()));
    it.put_prop("*track*", "gain", Value::Float(0.0));
    it.put_prop("*track*", "pan", Value::Float(0.0));
    it.put_prop("*track*", "index", Value::Int(1));
    // One clip spanning the selection; a stereo track gets an array of per-channel lists.
    let clip_list = Value::list(vec![Value::list(vec![
        Value::Float(0.0),
        Value::Float(dur),
    ])]);
    it.put_prop(
        "*track*",
        "clips",
        if channels.len() > 1 {
            Value::Array(Rc::new(RefCell::new(vec![clip_list; channels.len()])))
        } else {
            clip_list
        },
    );
    it.put_prop("*selection*", "start", Value::Float(selection_start));
    it.put_prop("*selection*", "end", Value::Float(selection_start + dur));
    it.put_prop("*selection*", "tracks", Value::list(vec![Value::Int(1)]));
    it.put_prop("*project*", "preview-duration", Value::Float(6.0));
    it.put_prop(
        "*selection*",
        "channels",
        Value::Int(channels.len().max(1) as i64),
    );
    let peak = channels
        .iter()
        .flat_map(|c| c.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    let rms = {
        let n: usize = channels.iter().map(|c| c.len()).sum();
        if n == 0 {
            0.0
        } else {
            (channels
                .iter()
                .flat_map(|c| c.iter())
                .map(|v| v * v)
                .sum::<f32>()
                / n as f32)
                .sqrt()
        }
    };
    it.put_prop("*selection*", "peak", Value::Float(peak as f64));
    it.put_prop("*selection*", "peak-level", Value::Float(peak as f64));
    it.put_prop("*selection*", "rms", Value::Float(rms as f64));
    it.put_prop("*selection*", "rate", Value::Float(sr));
    it.put_prop("*project*", "rate", Value::Float(sr));
    it.put_prop("*project*", "tracks", Value::Int(1));
    it.put_prop("*project*", "wavetracks", Value::Int(1));
    it.put_prop("*project*", "name", Value::Str("EthnoFlow".into()));
    it.put_prop("*audacity*", "version", Value::Str("3.4.2".into()));
    it.put_prop("*system-dir*", "user-plug-in", Value::Str(String::new()));

    let result = it.run(&header.code).map_err(|e| e.to_string())?;
    let output = match result {
        Value::Nil => PluginOutput::Nothing,
        Value::Str(s) => PluginOutput::Message(s),
        Value::Sound(s) => {
            let mono = s.data.clone();
            PluginOutput::Audio(vec![mono; channels.len().max(1)])
        }
        Value::Array(arr) => {
            let arr = arr.borrow();
            let mut out = Vec::new();
            for v in arr.iter() {
                match v {
                    Value::Sound(s) => out.push(s.data.clone()),
                    o => {
                        return Err(format!(
                            "the plug-in returned an array with a non-sound: {}",
                            o.display()
                        ))
                    }
                }
            }
            PluginOutput::Audio(out)
        }
        Value::List(items) => {
            // Audacity labels: ((time "text") (start end "text") …)
            let mut labels = Vec::new();
            for it in items.iter() {
                let Some(l) = it.as_list() else { break };
                match l {
                    [t, Value::Str(s)] if t.is_number() => labels.push(Label {
                        start: t.as_f64().unwrap_or(0.0),
                        end: None,
                        text: s.clone(),
                    }),
                    [t0, t1, Value::Str(s)] if t0.is_number() && t1.is_number() => {
                        labels.push(Label {
                            start: t0.as_f64().unwrap_or(0.0),
                            end: t1.as_f64(),
                            text: s.clone(),
                        })
                    }
                    _ => break,
                }
            }
            if labels.len() == items.len() && !labels.is_empty() {
                PluginOutput::Labels(labels)
            } else {
                PluginOutput::Message(Value::List(items).display())
            }
        }
        other => PluginOutput::Message(other.display()),
    };
    Ok(RunResult {
        output,
        printed: it.output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(sr: f64, hz: f64, secs: f64) -> Vec<f32> {
        (0..(sr * secs) as usize)
            .map(|i| (i as f64 * hz / sr * std::f64::consts::TAU).sin() as f32 * 0.5)
            .collect()
    }

    #[test]
    fn lisp_core_works() {
        let mut it = Interp::new(44_100.0);
        let v = it
            .run(
                r#"
(defun fact (n) (if (<= n 1) 1 (* n (fact (1- n)))))
(setq lst '(3 1 2))
(let* ((total 0) (names nil))
  (dolist (x lst) (setq total (+ total x)) (push (format nil "n~a" x) names))
  (dotimes (i 3) (setq total (+ total i)))
  (do ((i 0 (1+ i))) ((= i 2)) (setq total (+ total 10)))
  (list (fact 5) total (reverse names) (case 2 (1 "one") ((2 3) "two-three") (t "other"))
        (/ 7 2) (/ 7.0 2) (cond ((> 1 2) 'no) (t 'yes)) (nth 1 lst) (strcat "a" "b")
        (string-upcase "ab") (length lst) (apply #'+ lst) (mapcar #'1+ '(1 2))
        (loop (return 42))))
"#,
            )
            .unwrap();
        assert_eq!(
            v.display(),
            "(120 29 (n3 n1 n2) two-three 3 3.5 YES 1 ab AB 3 6 (2 3) 42)"
        );
    }

    #[test]
    fn gain_plugin_scales_the_selection() {
        let src = ";nyquist plug-in\n;version 4\n;type process\n;name \"Half\"\n;control gain \"Gain\" float \"\" 0.5 0 1\n(mult *track* gain)\n";
        let h = parse_header(src);
        let input = sine(8000.0, 440.0, 0.1);
        let r = run_plugin(&h, &[], std::slice::from_ref(&input), 8000.0, 0.0).unwrap();
        let PluginOutput::Audio(out) = r.output else {
            panic!("expected audio")
        };
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), input.len());
        assert!((out[0][10] - input[10] * 0.5).abs() < 1e-6);
    }

    #[test]
    fn tremolo_plugin_with_lfo_on_stereo() {
        let src = r#";nyquist plug-in
;version 4
;type process
;name "Tremolo"
;control wet "Wet" int "" 40 1 100
;control lfo "Hz" float "" 4 0.001 1000
(setq wet (/ wet 200.0))
(defun tremolo (sig)
  (mult sig (sum (- 1.0 wet) (mult wet (lfo lfo (get-duration 1))))))
(multichan-expand #'tremolo *track*)
"#;
        let h = parse_header(src);
        let l = sine(8000.0, 440.0, 0.25);
        let r = sine(8000.0, 220.0, 0.25);
        let out = run_plugin(
            &h,
            &[("lfo".into(), ControlValue::Number(8.0))],
            &[l.clone(), r.clone()],
            8000.0,
            0.0,
        )
        .unwrap();
        let PluginOutput::Audio(ch) = out.output else {
            panic!("expected audio")
        };
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].len(), l.len());
        // The envelope stays between 0.6 and 1.0 of the input.
        let ratio = ch[1]
            .iter()
            .zip(&r)
            .filter(|(_, x)| x.abs() > 0.3)
            .map(|(y, x)| y / x)
            .fold((2.0f32, 0.0f32), |(lo, hi), q| (lo.min(q), hi.max(q)));
        assert!(ratio.0 > 0.55 && ratio.1 < 1.05, "{ratio:?}");
    }

    #[test]
    fn analyze_plugin_returns_a_message_with_props() {
        let src = ";nyquist plug-in\n;version 4\n;type analyze\n;name \"Peak\"\n(format nil \"Peak ~a dB, ~a channels, ~a s\" (round (linear-to-db (get '*selection* 'peak))) (get '*track* 'channels) (get-duration 1))\n";
        let h = parse_header(src);
        let r = run_plugin(&h, &[], &[sine(8000.0, 100.0, 0.5)], 8000.0, 0.0).unwrap();
        assert_eq!(
            r.output,
            PluginOutput::Message("Peak -6 dB, 1 channels, 1.0 s".into())
        );
    }

    #[test]
    fn generate_plugin_makes_a_tone_and_fade() {
        let src = ";nyquist plug-in\n;version 4\n;type generate\n;name \"Tone\"\n;control dur \"Duration\" float \"\" 0.5 0 10\n(mult (osc 69 dur) (pwl 0.1 1 (- dur 0.1) 1 dur))\n";
        let h = parse_header(src);
        let r = run_plugin(&h, &[], &[], 8000.0, 0.0).unwrap();
        let PluginOutput::Audio(ch) = r.output else {
            panic!("expected audio")
        };
        assert_eq!(ch[0].len(), 4000);
        assert!(ch[0][0].abs() < 1e-3);
        let p = ch[0].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(p > 0.95 && p <= 1.0001, "{p}");
    }

    #[test]
    fn objects_and_fft_round_trip() {
        // A class with an ivar and methods; an STFT iterator (like Vocal Reduction's) feeding
        // snd-ifft with hop = frame length reconstructs the signal.
        let mut it = Interp::new(8000.0);
        let v = it
            .run(
                r#"
(setf counter (send class :new '(n step)))
(send counter :answer :isnew '(s) '((setf n 0) (setf step s)))
(send counter :answer :next '() '((setf n (+ n step)) n))
(setf c (send counter :new 5))
(send c :next)
(send c :next)
"#,
            )
            .unwrap();
        assert_eq!(v, Value::Int(10));
        let src = r#";nyquist plug-in
;type process
(setf fft-class (send class :new '(sound length skip)))
(send fft-class :answer :next '() '((snd-fft sound length skip nil)))
(send fft-class :answer :isnew '(s l k) '((setf sound s) (setf length l) (setf skip k)))
(snd-ifft 0 *sound-srate* (send fft-class :new *track* 256 256) 256 nil)
"#;
        let h = parse_header(src);
        let input = sine(8000.0, 440.0, 0.256);
        let r = run_plugin(&h, &[], std::slice::from_ref(&input), 8000.0, 0.0).unwrap();
        let PluginOutput::Audio(ch) = r.output else {
            panic!("expected audio")
        };
        assert_eq!(ch[0].len(), input.len());
        let err = ch[0]
            .iter()
            .zip(&input)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-4, "round trip error {err}");
    }

    #[test]
    fn filters_and_errors() {
        let src = ";nyquist plug-in\n;type process\n(lowpass2 *track* 100 0.7)\n";
        let h = parse_header(src);
        let hi = sine(8000.0, 2000.0, 0.2);
        let r = run_plugin(&h, &[], &[hi], 8000.0, 0.0).unwrap();
        let PluginOutput::Audio(ch) = r.output else {
            panic!()
        };
        let p = ch[0][1000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(p < 0.02, "{p}");

        let bad = parse_header(";nyquist plug-in\n(undefined-fn *track*)\n");
        let e = run_plugin(&bad, &[], &[vec![0.0; 10]], 8000.0, 0.0).unwrap_err();
        assert!(e.contains("UNDEFINED-FN"), "{e}");
    }
}
