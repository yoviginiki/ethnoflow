//! Evaluator for the XLISP subset that Audacity's Nyquist plug-ins use, plus the sound
//! primitives (fixed arrays). Stereo sounds are arrays of two sounds and every sound
//! primitive expands over channels the way Audacity's `multichan-expand` does.

use std::collections::HashMap;
use std::rc::Rc;

use crate::reader::{ReadError, Reader};
use crate::sound::{self, BiquadKind};
use crate::value::{Env, Lambda, Sound, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum NyError {
    Read(ReadError),
    Runtime(String),
    /// `(return v)` unwinding to the nearest loop / `prog`.
    Return(Value),
    /// `(return-from name v)` unwinding to the enclosing function.
    ReturnFrom(Value),
    /// `(throw tag v)` unwinding to the matching `catch`.
    Throw(Value, Value),
}

impl From<ReadError> for NyError {
    fn from(e: ReadError) -> Self {
        NyError::Read(e)
    }
}

impl std::fmt::Display for NyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NyError::Read(e) => write!(f, "line {}: {}", e.line, e.message),
            NyError::Runtime(m) => write!(f, "{m}"),
            NyError::Return(v) | NyError::ReturnFrom(v) => {
                write!(f, "return outside a loop: {}", v.display())
            }
            NyError::Throw(tag, v) => {
                write!(f, "uncaught throw {}: {}", tag.display(), v.display())
            }
        }
    }
}

pub type NyResult<T> = Result<T, NyError>;

fn rt<T>(m: impl Into<String>) -> NyResult<T> {
    Err(NyError::Runtime(m.into()))
}

const BUILTINS: &[&str] = &[
    // arithmetic
    "+",
    "-",
    "*",
    "/",
    "rem",
    "mod",
    "min",
    "max",
    "abs",
    "sqrt",
    "expt",
    "power",
    "exp",
    "log",
    "sin",
    "cos",
    "tan",
    "atan",
    "truncate",
    "round",
    "floor",
    "ceiling",
    "float",
    "1+",
    "1-",
    "random",
    "rrandom",
    "logand",
    "logior",
    "ash",
    // comparison / predicates
    "=",
    "/=",
    "<",
    ">",
    "<=",
    ">=",
    "eq",
    "eql",
    "equal",
    "null",
    "not",
    "atom",
    "listp",
    "consp",
    "numberp",
    "integerp",
    "floatp",
    "stringp",
    "symbolp",
    "soundp",
    "arrayp",
    "boundp",
    "fboundp",
    "zerop",
    "plusp",
    "minusp",
    "evenp",
    "oddp",
    "functionp",
    // lists
    "car",
    "cdr",
    "cons",
    "list",
    "append",
    "reverse",
    "length",
    "nth",
    "nthcdr",
    "first",
    "second",
    "third",
    "fourth",
    "rest",
    "last",
    "member",
    "assoc",
    "mapcar",
    "mapc",
    "funcall",
    "apply",
    "cadr",
    "cddr",
    "caar",
    "cdar",
    "caddr",
    "cdddr",
    "cadddr",
    "remove",
    "remove-if",
    "remove-if-not",
    "butlast",
    "copy-list",
    "sort",
    "every",
    "some",
    "reduce",
    "position",
    "find",
    // arrays
    "make-array",
    "aref",
    "vector",
    // strings & output
    "strcat",
    "string",
    "string-upcase",
    "string-downcase",
    "string-trim",
    "string-left-trim",
    "string-right-trim",
    "char",
    "subseq",
    "format",
    "print",
    "princ",
    "terpri",
    "prin1",
    "error",
    "intern",
    "symbol-name",
    "read-from-string",
    "parse-integer",
    "string=",
    "string<",
    "string-equal",
    "nstring-upcase",
    "nstring-downcase",
    "string-search",
    "digit-char-p",
    "char-code",
    "code-char",
    "string-rep",
    "int-char",
    "char-int",
    // properties
    "get",
    "putprop",
    "remprop",
    // sounds
    "mult",
    "prod",
    "sum",
    "diff",
    "scale",
    "scale-db",
    "db-to-linear",
    "linear-to-db",
    "hp",
    "lp",
    "lowpass2",
    "highpass2",
    "bandpass2",
    "notch2",
    "lowpass4",
    "highpass4",
    "lowpass6",
    "highpass6",
    "lowpass8",
    "highpass8",
    "pwl",
    "pwlv",
    "pwl-list",
    "pwlv-list",
    "env",
    "osc",
    "hzosc",
    "lfo",
    "osc-tri",
    "osc-saw",
    "osc-pulse",
    "noise",
    "s-rest",
    "extract",
    "extract-abs",
    "seq",
    "sim",
    "at",
    "at-abs",
    "stretch",
    "stretch-abs",
    "peak",
    "snd-length",
    "snd-srate",
    "snd-t0",
    "snd-from-array",
    "snd-samples",
    "snd-fetch",
    "snd-maxsamp",
    "s-min",
    "s-max",
    "s-abs",
    "s-sqrt",
    "s-exp",
    "s-log",
    "clip",
    "step-to-hz",
    "hz-to-step",
    "get-duration",
    "sound-srate",
    "control-srate",
    "local-to-global",
    "multichan-expand",
    "snd-avg",
    "rms",
    "integrate",
    "slope",
    "abs-env",
    "force-srate",
    "resample",
    "sound-p",
    "const",
    "s-sqrt",
    "snd-copy",
    "snd-xform",
    "cue",
    "sound",
    "snd-offset",
    "snd-scale",
    "snd-add",
    "snd-prod",
    "snd-tapv",
    "feedback-delay",
    "delay",
    "comb",
    "alpass",
    "areson",
    "reson",
    "tone",
    "atone",
    "eq-lowshelf",
    "eq-highshelf",
    "eq-band",
    "shape",
    "normalize",
    "snd-sref",
    "sref",
    "quantize",
    "snd-quantize",
    "gate",
    "s-plot",
    "setf-hack",
    "_",
    "string-append",
    "char=",
    "char-equal",
    "char/=",
    "char<",
    "char>",
    "char-upcase",
    "char-downcase",
    "alphanumericp",
    "display",
    "snd-fft",
    "snd-ifft",
    "pv-time-pitch",
    "jcrev",
    "stkrev",
    "nrev",
    "phasevocoder",
    "fmosc",
    "build-harmonic",
    "maketable",
    "exp-dec",
    "pwev",
    "pwev-list",
    "pwevr",
    "snd-const",
    "snd-pwl",
    "snd-biquad",
    "biquad",
    "biquad-m",
    "snd-follow",
    "follow",
    "snd-pluck",
    "pluck",
    "snd-fetch-array",
    "set-logical-stop",
    "set-logical-stop-abs",
    "snd-set-logical-stop",
    "s-rest-abs",
    "snd-exp",
    "snd-log",
    "snd-abs",
    "sine",
    "snd-oneshot",
    "loud",
    "buzz",
    "snd-buzz",
    "eval",
    "read",
    "make-string-input-stream",
    "get-loud",
    "get-transpose",
    "get-sustain",
    "get-tempo",
];

pub struct Interp {
    pub env: Env,
    /// The selection's sample rate (`*sound-srate*`).
    pub sr: f64,
    props: HashMap<(String, String), Value>,
    pub output: String,
    depth: usize,
    steps: u64,
    max_steps: u64,
    rng: u32,
    /// Nyquist's time warp as a plain factor: `(stretch k …)` multiplies the durations of
    /// the behaviours (osc, pwl, noise, …) evaluated inside it; `abs-env` resets it to 1.
    pub stretch: f64,
    /// `snd-fetch` read positions per sound object (kept alive so the key stays unique).
    streams: HashMap<usize, (Rc<Sound>, usize)>,
}

impl Interp {
    pub fn new(sr: f64) -> Self {
        let env = Env::root();
        env.define("*sound-srate*", Value::Float(sr));
        env.define("*control-srate*", Value::Float(sr / 20.0));
        env.define("*default-sound-srate*", Value::Float(sr));
        env.define("*default-control-srate*", Value::Float(sr / 20.0));
        env.define(
            "*warp*",
            Value::list(vec![Value::Float(0.0), Value::Float(1.0), Value::Nil]),
        );
        env.define("*scratch*", Value::Nil);
        env.define("pi", Value::Float(std::f64::consts::PI));
        env.define("*track*", Value::Nil);
        env.define("*selection*", Value::sym("*selection*"));
        env.define("*project*", Value::sym("*project*"));
        env.define("*system-dir*", Value::sym("*system-dir*"));
        env.define("*locale*", Value::Nil);
        env.define("*sal-call-stack*", Value::Nil);
        env.define("s", Value::Nil);
        env.define("*previewp*", Value::Nil);
        env.define("ny:all", Value::Int(1 << 40));
        env.define("op-average", Value::Int(1));
        env.define("op-peak", Value::Int(2));
        env.define("op-rms", Value::Int(3));
        // Pitch constants c0 … b8 with sharps (s) and flats (f): c4 = 60.
        for oct in 0..=8i64 {
            for (name, semi) in [
                ("c", 0),
                ("cs", 1),
                ("df", 1),
                ("d", 2),
                ("ds", 3),
                ("ef", 3),
                ("e", 4),
                ("f", 5),
                ("fs", 6),
                ("gf", 6),
                ("g", 7),
                ("gs", 8),
                ("af", 8),
                ("a", 9),
                ("as", 10),
                ("bf", 10),
                ("b", 11),
            ] {
                env.define(&format!("{name}{oct}"), Value::Int((oct + 1) * 12 + semi));
            }
        }
        // Wave tables: one period of 2048 samples, as (sound step periodic).
        let n = 2048usize;
        let table = |f: fn(f64) -> f64| -> Value {
            let data: Vec<f32> = (0..n).map(|i| f(i as f64 / n as f64) as f32).collect();
            let period = Sound::new(sr, data);
            let step = 69.0 + 12.0 * ((1.0 / period.duration()) / 440.0).log2();
            Value::list(vec![
                Value::Sound(Rc::new(period)),
                Value::Float(step),
                Value::T,
            ])
        };
        env.define("*sine-table*", table(|p| (p * std::f64::consts::TAU).sin()));
        env.define(
            "*tri-table*",
            table(|p| 4.0 * (p - (p + 0.5).floor()).abs() - 1.0),
        );
        env.define("*saw-table*", table(|p| 2.0 * p - 1.0));
        env.define("*table*", table(|p| (p * std::f64::consts::TAU).sin()));
        Self {
            env,
            sr,
            props: HashMap::new(),
            output: String::new(),
            depth: 0,
            steps: 0,
            max_steps: 50_000_000,
            rng: 0x9E37_79B9,
            stretch: 1.0,
            streams: HashMap::new(),
        }
    }

    pub fn put_prop(&mut self, sym: &str, prop: &str, v: Value) {
        self.props
            .insert((sym.to_ascii_lowercase(), prop.to_ascii_lowercase()), v);
    }

    /// Evaluate every top-level form; the value of the last one is the result.
    pub fn run(&mut self, src: &str) -> NyResult<Value> {
        let forms = Reader::new(src).read_all()?;
        let env = self.env.clone();
        let mut last = Value::Nil;
        for f in &forms {
            last = self.eval(f, &env)?;
        }
        Ok(last)
    }

    fn tick(&mut self) -> NyResult<()> {
        self.steps += 1;
        if self.steps > self.max_steps {
            return rt("the plug-in ran too long (step limit)");
        }
        Ok(())
    }

    pub fn eval(&mut self, expr: &Value, env: &Env) -> NyResult<Value> {
        self.tick()?;
        match expr {
            Value::Sym(s) => {
                if let Some(v) = env.get(s) {
                    return Ok(v.clone());
                }
                if s.starts_with(':') {
                    return Ok(expr.clone());
                }
                if BUILTINS.contains(&s.as_str()) {
                    return Ok(Value::Builtin(
                        BUILTINS
                            .iter()
                            .find(|b| **b == s.as_str())
                            .copied()
                            .unwrap(),
                    ));
                }
                rt(format!("unbound variable: {}", s.to_ascii_uppercase()))
            }
            Value::List(items) => {
                let head = &items[0];
                if let Value::Sym(name) = head {
                    if let Some(v) = self.special(name, &items[1..], env)? {
                        return Ok(v);
                    }
                }
                let f = match head {
                    Value::Sym(name) => match env.get(name) {
                        Some(Value::Lambda(l)) if l.is_macro => {
                            let expansion = self.call_lambda(&l, items[1..].to_vec())?;
                            return self.eval(&expansion, env);
                        }
                        Some(v @ Value::Lambda(_)) => v,
                        _ => {
                            if let Some(b) = BUILTINS.iter().find(|b| **b == name.as_str()) {
                                Value::Builtin(b)
                            } else if let Some(v) = env.get(name) {
                                v
                            } else {
                                return rt(format!(
                                    "unbound function: {}",
                                    name.to_ascii_uppercase()
                                ));
                            }
                        }
                    },
                    other => self.eval(other, env)?,
                };
                let mut args = Vec::with_capacity(items.len() - 1);
                for a in &items[1..] {
                    args.push(self.eval(a, env)?);
                }
                self.apply(&f, args)
            }
            other => Ok(other.clone()),
        }
    }

    fn eval_body(&mut self, body: &[Value], env: &Env) -> NyResult<Value> {
        let mut last = Value::Nil;
        for b in body {
            last = self.eval(b, env)?;
        }
        Ok(last)
    }

    pub fn apply(&mut self, f: &Value, args: Vec<Value>) -> NyResult<Value> {
        match f {
            Value::Builtin(name) => self.builtin(name, args),
            Value::Lambda(l) => {
                self.depth += 1;
                if self.depth > 600 {
                    self.depth -= 1;
                    return rt("recursion too deep");
                }
                let r = self.call_lambda(l, args);
                self.depth -= 1;
                r
            }
            Value::Sym(name) => {
                let g = match self.env.get(name) {
                    Some(v) => v,
                    None if BUILTINS.contains(&name.as_str()) => Value::Builtin(
                        BUILTINS
                            .iter()
                            .find(|b| **b == name.as_str())
                            .copied()
                            .unwrap(),
                    ),
                    None => return rt(format!("unbound function: {}", name.to_ascii_uppercase())),
                };
                self.apply(&g, args)
            }
            other => rt(format!("not a function: {}", other.display())),
        }
    }

    fn call_lambda(&mut self, l: &Rc<Lambda>, args: Vec<Value>) -> NyResult<Value> {
        let env = l.closure.child();
        let mut it = args.into_iter();
        for p in &l.params {
            match it.next() {
                Some(v) => env.define(p, v),
                None => {
                    return rt(format!(
                        "too few arguments (missing {})",
                        p.to_ascii_uppercase()
                    ))
                }
            }
        }
        for (p, default) in &l.optional {
            match it.next() {
                Some(v) => env.define(p, v),
                None => {
                    let d = self.eval(default, &env)?;
                    env.define(p, d);
                }
            }
        }
        let mut rest: Vec<Value> = it.collect();
        if !l.keys.is_empty() {
            // `:name value` pairs anywhere in the remaining arguments
            let mut given: Vec<(String, Value)> = Vec::new();
            let mut others = Vec::new();
            let mut i = 0;
            while i < rest.len() {
                match &rest[i] {
                    Value::Sym(k) if k.starts_with(':') && i + 1 < rest.len() => {
                        given.push((k[1..].to_string(), rest[i + 1].clone()));
                        i += 2;
                    }
                    other => {
                        others.push(other.clone());
                        i += 1;
                    }
                }
            }
            for (k, default) in &l.keys {
                match given.iter().find(|(g, _)| g == k) {
                    Some((_, v)) => env.define(k, v.clone()),
                    None => {
                        let d = self.eval(default, &env)?;
                        env.define(k, d);
                    }
                }
            }
            rest = others;
        }
        match &l.rest {
            Some(r) => env.define(r, Value::list(rest)),
            None if !rest.is_empty() => return rt("too many arguments"),
            None => {}
        }
        match self.eval_body(&l.body, &env) {
            Err(NyError::ReturnFrom(v)) => Ok(v),
            r => r,
        }
    }

    /// `` `(a ,b ,@c) ``: copy the template, evaluating `,x` and splicing `,@x`.
    fn backquote(&mut self, v: &Value, env: &Env) -> NyResult<Value> {
        let Some(items) = v.as_list() else {
            return Ok(v.clone());
        };
        if items.is_empty() {
            return Ok(Value::Nil);
        }
        if let Value::Sym(h) = &items[0] {
            if h == "comma" {
                return self.eval(items.get(1).unwrap_or(&Value::Nil), env);
            }
        }
        let mut out = Vec::with_capacity(items.len());
        for it in items {
            if let Some(l) = it.as_list() {
                if let Some(Value::Sym(h)) = l.first() {
                    if h == "comma-at" {
                        let spliced = self.eval(l.get(1).unwrap_or(&Value::Nil), env)?;
                        out.extend(spliced.as_list().unwrap_or(&[]).iter().cloned());
                        continue;
                    }
                }
            }
            out.push(self.backquote(it, env)?);
        }
        Ok(Value::list(out))
    }

    /// `(seqrep (i n) expr)` / `(simrep (i n) expr)`: evaluate `expr` n times and join.
    fn rep(&mut self, args: &[Value], env: &Env, sequential: bool) -> NyResult<Value> {
        let Some(spec) = args.first().and_then(|a| a.as_list()) else {
            return rt("seqrep needs (var count)");
        };
        let Some(Value::Sym(var)) = spec.first() else {
            return rt("seqrep needs a variable");
        };
        let n = int_of(&self.eval(spec.get(1).unwrap_or(&Value::Nil), env)?)?;
        let inner = env.child();
        let mut parts: Vec<Value> = Vec::new();
        for i in 0..n.max(0) {
            inner.define(var, Value::Int(i));
            parts.push(self.eval_body(&args[1..], &inner)?);
        }
        self.builtin(if sequential { "seq" } else { "sim" }, parts)
    }

    /// `(osc pitch dur table phase)` family: one period from a table (or a sine), at `hz`.
    fn table_osc(
        &mut self,
        hz_or_sound: &Value,
        dur: f64,
        table: &Value,
        phase_deg: f64,
    ) -> NyResult<Value> {
        let sr = self.sr;
        let period: Option<Rc<Sound>> = match table {
            Value::List(l) => match l.first() {
                Some(Value::Sound(s)) => Some(s.clone()),
                _ => None,
            },
            Value::Sound(s) => Some(s.clone()),
            _ => None,
        };
        let phase0 = (phase_deg / 360.0).rem_euclid(1.0);
        let hz_sound = match hz_or_sound {
            Value::Sound(s) => Some(s.clone()),
            _ => None,
        };
        let hz_const = match hz_or_sound {
            Value::Sound(_) => 0.0,
            o => num_of(o)?,
        };
        let n = match &hz_sound {
            Some(s) => s.data.len(),
            None => (dur * sr).round() as usize,
        };
        let mut ph = phase0;
        let mut data = Vec::with_capacity(n);
        for i in 0..n {
            let v = match &period {
                Some(p) if !p.data.is_empty() => {
                    let pos = ph * p.data.len() as f64;
                    let i0 = pos.floor() as usize % p.data.len();
                    let i1 = (i0 + 1) % p.data.len();
                    let t = (pos - pos.floor()) as f32;
                    p.data[i0] + (p.data[i1] - p.data[i0]) * t
                }
                _ => (ph * std::f64::consts::TAU).sin() as f32,
            };
            data.push(v);
            let hz = match &hz_sound {
                Some(s) => s.data[i] as f64,
                None => hz_const,
            };
            ph = (ph + hz / sr).rem_euclid(1.0);
        }
        Ok(Value::Sound(Rc::new(Sound::new(sr, data))))
    }

    fn table(&self, period: Sound) -> Value {
        let step = 69.0 + 12.0 * ((1.0 / period.duration().max(1e-9)) / 440.0).log2();
        Value::list(vec![
            Value::Sound(Rc::new(period)),
            Value::Float(step),
            Value::T,
        ])
    }

    fn parse_lambda(&mut self, params: &Value, body: &[Value], env: &Env) -> NyResult<Value> {
        let Some(ps) = params.as_list() else {
            return rt("bad lambda list");
        };
        let mut l = Lambda {
            is_macro: false,
            params: Vec::new(),
            optional: Vec::new(),
            keys: Vec::new(),
            rest: None,
            body: body.to_vec(),
            closure: env.clone(),
        };
        let mut mode = 0;
        for p in ps {
            match p {
                Value::Sym(s) if s == "&optional" => mode = 1,
                Value::Sym(s) if s == "&rest" => mode = 2,
                Value::Sym(s) if s == "&key" => mode = 3,
                Value::Sym(s) if s == "&aux" => mode = 4,
                Value::Sym(s) => match mode {
                    0 => l.params.push(s.clone()),
                    3 => l.keys.push((s.clone(), Value::Nil)),
                    1 | 4 => l.optional.push((s.clone(), Value::Nil)),
                    _ => l.rest = Some(s.clone()),
                },
                Value::List(pair) => {
                    if let Some(Value::Sym(s)) = pair.first() {
                        let default = pair.get(1).cloned().unwrap_or(Value::Nil);
                        if mode == 3 {
                            l.keys.push((s.clone(), default));
                        } else {
                            l.optional.push((s.clone(), default));
                        }
                    }
                }
                _ => return rt("bad parameter"),
            }
        }
        Ok(Value::Lambda(Rc::new(l)))
    }

    /// Special forms; `Ok(None)` means "not a special form".
    fn special(&mut self, name: &str, args: &[Value], env: &Env) -> NyResult<Option<Value>> {
        let v = match name {
            "quote" => args.first().cloned().unwrap_or(Value::Nil),
            "function" => match args.first() {
                Some(Value::Sym(s)) => match env.get(s) {
                    Some(v) => v,
                    None => Value::Sym(s.clone()),
                },
                Some(l) => self.eval(l, env)?,
                None => Value::Nil,
            },
            "lambda" => self.parse_lambda(
                args.first().unwrap_or(&Value::Nil),
                args.get(1..).unwrap_or(&[]),
                env,
            )?,
            "defun" | "defmacro" => {
                let Some(Value::Sym(fname)) = args.first() else {
                    return rt("defun needs a name");
                };
                // Global name, but the closure keeps the lexical scope (nested defuns close
                // over the outer function's parameters, as XLISP does).
                let f = self.parse_lambda(
                    args.get(1).unwrap_or(&Value::Nil),
                    args.get(2..).unwrap_or(&[]),
                    env,
                )?;
                let f = match (name, f) {
                    ("defmacro", Value::Lambda(l)) => {
                        let mut m = (*l).clone();
                        m.is_macro = true;
                        Value::Lambda(Rc::new(m))
                    }
                    (_, f) => f,
                };
                self.env.define(fname, f);
                Value::Sym(fname.clone())
            }
            "if" => {
                let c = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                if c.truthy() {
                    self.eval(args.get(1).unwrap_or(&Value::Nil), env)?
                } else {
                    self.eval_body(args.get(2..).unwrap_or(&[]), env)?
                }
            }
            "when" => {
                let c = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                if c.truthy() {
                    self.eval_body(&args[1..], env)?
                } else {
                    Value::Nil
                }
            }
            "unless" => {
                let c = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                if !c.truthy() {
                    self.eval_body(&args[1..], env)?
                } else {
                    Value::Nil
                }
            }
            "cond" => {
                for clause in args {
                    let Some(c) = clause.as_list() else { continue };
                    if c.is_empty() {
                        continue;
                    }
                    let t = self.eval(&c[0], env)?;
                    if t.truthy() {
                        return Ok(Some(if c.len() == 1 {
                            t
                        } else {
                            self.eval_body(&c[1..], env)?
                        }));
                    }
                }
                Value::Nil
            }
            "case" => {
                let key = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                for clause in &args[1..] {
                    let Some(c) = clause.as_list() else { continue };
                    if c.is_empty() {
                        continue;
                    }
                    let hit = match &c[0] {
                        Value::T => true,
                        Value::Sym(s) if s == "otherwise" => true,
                        Value::List(ks) => ks.contains(&key),
                        k => *k == key,
                    };
                    if hit {
                        return Ok(Some(self.eval_body(&c[1..], env)?));
                    }
                }
                Value::Nil
            }
            "and" => {
                let mut last = Value::T;
                for a in args {
                    last = self.eval(a, env)?;
                    if !last.truthy() {
                        return Ok(Some(Value::Nil));
                    }
                }
                last
            }
            "or" => {
                for a in args {
                    let v = self.eval(a, env)?;
                    if v.truthy() {
                        return Ok(Some(v));
                    }
                }
                Value::Nil
            }
            "progn" => self.eval_body(args, env)?,
            "prog1" => {
                let first = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                self.eval_body(args.get(1..).unwrap_or(&[]), env)?;
                first
            }
            "prog" | "prog*" => {
                let inner = env.child();
                self.bind_lets(
                    args.first().unwrap_or(&Value::Nil),
                    env,
                    &inner,
                    name == "prog*",
                )?;
                match self.eval_body(&args[1..], &inner) {
                    Ok(_) => Value::Nil,
                    Err(NyError::Return(v)) => v,
                    Err(e) => return Err(e),
                }
            }
            "let" | "let*" => {
                let inner = env.child();
                self.bind_lets(
                    args.first().unwrap_or(&Value::Nil),
                    env,
                    &inner,
                    name == "let*",
                )?;
                self.eval_body(args.get(1..).unwrap_or(&[]), &inner)?
            }
            "setq" | "psetq" => {
                let mut last = Value::Nil;
                for pair in args.chunks(2) {
                    let Value::Sym(s) = &pair[0] else {
                        return rt("setq needs a symbol");
                    };
                    last = self.eval(pair.get(1).unwrap_or(&Value::Nil), env)?;
                    env.set(s, last.clone());
                }
                last
            }
            "setf" => {
                let mut last = Value::Nil;
                for pair in args.chunks(2) {
                    last = self.eval(pair.get(1).unwrap_or(&Value::Nil), env)?;
                    self.set_place(&pair[0], last.clone(), env)?;
                }
                last
            }
            "incf" | "decf" => {
                let cur = self.eval(&args[0], env)?;
                let d = match args.get(1) {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Int(1),
                };
                let v = self.arith(if name == "incf" { "+" } else { "-" }, vec![cur, d])?;
                self.set_place(&args[0], v.clone(), env)?;
                v
            }
            "push" => {
                let v = self.eval(&args[0], env)?;
                let cur = self.eval(&args[1], env)?;
                let mut items = vec![v];
                items.extend(cur.as_list().unwrap_or(&[]).iter().cloned());
                let nv = Value::list(items);
                self.set_place(&args[1], nv.clone(), env)?;
                nv
            }
            "pop" => {
                let cur = self.eval(&args[0], env)?;
                let l = cur.as_list().unwrap_or(&[]).to_vec();
                let first = l.first().cloned().unwrap_or(Value::Nil);
                self.set_place(
                    &args[0],
                    Value::list(l.get(1..).unwrap_or(&[]).to_vec()),
                    env,
                )?;
                first
            }
            "dolist" => {
                let Some(spec) = args.first().and_then(|a| a.as_list()) else {
                    return rt("dolist spec");
                };
                let Some(Value::Sym(var)) = spec.first() else {
                    return rt("dolist var");
                };
                let list = self.eval(spec.get(1).unwrap_or(&Value::Nil), env)?;
                let inner = env.child();
                inner.define(var, Value::Nil);
                let items = list.as_list().map(|l| l.to_vec()).unwrap_or_default();
                for it in items {
                    inner.define(var, it);
                    match self.eval_body(&args[1..], &inner) {
                        Ok(_) => {}
                        Err(NyError::Return(v)) => return Ok(Some(v)),
                        Err(e) => return Err(e),
                    }
                }
                inner.define(var, Value::Nil);
                match spec.get(2) {
                    Some(r) => self.eval(r, &inner)?,
                    None => Value::Nil,
                }
            }
            "dotimes" => {
                let Some(spec) = args.first().and_then(|a| a.as_list()) else {
                    return rt("dotimes spec");
                };
                let Some(Value::Sym(var)) = spec.first() else {
                    return rt("dotimes var");
                };
                let n = self.eval(spec.get(1).unwrap_or(&Value::Nil), env)?;
                let n = int_of(&n)?;
                let inner = env.child();
                for i in 0..n.max(0) {
                    inner.define(var, Value::Int(i));
                    match self.eval_body(&args[1..], &inner) {
                        Ok(_) => {}
                        Err(NyError::Return(v)) => return Ok(Some(v)),
                        Err(e) => return Err(e),
                    }
                }
                inner.define(var, Value::Int(n));
                match spec.get(2) {
                    Some(r) => self.eval(r, &inner)?,
                    None => Value::Nil,
                }
            }
            "do" | "do*" => {
                let inner = env.child();
                let specs = args
                    .first()
                    .and_then(|a| a.as_list())
                    .unwrap_or(&[])
                    .to_vec();
                let star = name == "do*";
                // init
                for s in &specs {
                    let (var, init, _) = do_spec(s)?;
                    let v = self.eval(&init, if star { &inner } else { env })?;
                    inner.define(&var, v);
                }
                let end = args
                    .get(1)
                    .and_then(|a| a.as_list())
                    .unwrap_or(&[])
                    .to_vec();
                loop {
                    let t = self.eval(end.first().unwrap_or(&Value::Nil), &inner)?;
                    if t.truthy() {
                        return Ok(Some(self.eval_body(end.get(1..).unwrap_or(&[]), &inner)?));
                    }
                    match self.eval_body(args.get(2..).unwrap_or(&[]), &inner) {
                        Ok(_) => {}
                        Err(NyError::Return(v)) => return Ok(Some(v)),
                        Err(e) => return Err(e),
                    }
                    if star {
                        for s in &specs {
                            let (var, _, step) = do_spec(s)?;
                            if let Some(st) = step {
                                let v = self.eval(&st, &inner)?;
                                inner.define(&var, v);
                            }
                        }
                    } else {
                        let mut news = Vec::new();
                        for s in &specs {
                            let (var, _, step) = do_spec(s)?;
                            if let Some(st) = step {
                                news.push((var, self.eval(&st, &inner)?));
                            }
                        }
                        for (var, v) in news {
                            inner.define(&var, v);
                        }
                    }
                }
            }
            "loop" => loop {
                match self.eval_body(args, env) {
                    Ok(_) => {}
                    Err(NyError::Return(v)) => return Ok(Some(v)),
                    Err(e) => return Err(e),
                }
            },
            "while" => {
                loop {
                    let c = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                    if !c.truthy() {
                        break;
                    }
                    match self.eval_body(&args[1..], env) {
                        Ok(_) => {}
                        Err(NyError::Return(v)) => return Ok(Some(v)),
                        Err(e) => return Err(e),
                    }
                }
                Value::Nil
            }
            "return" => {
                let v = match args.first() {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Nil,
                };
                return Err(NyError::Return(v));
            }
            "catch" | "unwind-protect" | "errset" => {
                let tag = if name == "catch" {
                    self.eval(args.first().unwrap_or(&Value::Nil), env)?
                } else {
                    Value::Nil
                };
                let body = if name == "catch" { &args[1..] } else { args };
                match self.eval_body(body, env) {
                    Ok(v) => {
                        if name == "errset" {
                            Value::list(vec![v])
                        } else {
                            v
                        }
                    }
                    Err(NyError::Runtime(_)) if name == "errset" => Value::Nil,
                    Err(NyError::Throw(t, v)) if name == "catch" && t == tag => v,
                    Err(e) => return Err(e),
                }
            }
            "throw" => {
                let tag = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                let v = match args.get(1) {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Nil,
                };
                return Err(NyError::Throw(tag, v));
            }
            "return-from" => {
                let v = match args.get(1) {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Nil,
                };
                return Err(NyError::ReturnFrom(v));
            }
            "backquote" => self.backquote(args.first().unwrap_or(&Value::Nil), env)?,
            "sound-srate-abs" | "control-srate-abs" => {
                self.eval_body(args.get(1..).unwrap_or(&[]), env)?
            }
            "abs-env" => {
                let old = self.stretch;
                self.stretch = 1.0;
                let r = self.eval_body(args, env);
                self.stretch = old;
                r?
            }
            "stretch" | "stretch-abs" => {
                let k = num_of(&self.eval(args.first().unwrap_or(&Value::Nil), env)?)?;
                let old = self.stretch;
                self.stretch = if name == "stretch" { old * k } else { k };
                let r = self.eval_body(args.get(1..).unwrap_or(&[]), env);
                self.stretch = old;
                r?
            }
            "progv" => {
                let syms = self.eval(args.first().unwrap_or(&Value::Nil), env)?;
                let vals = self.eval(args.get(1).unwrap_or(&Value::Nil), env)?;
                let inner = env.child();
                let vals = vals.as_list().unwrap_or(&[]).to_vec();
                for (i, s) in syms.as_list().unwrap_or(&[]).iter().enumerate() {
                    if let Value::Sym(s) = s {
                        inner.define(s, vals.get(i).cloned().unwrap_or(Value::Nil));
                    }
                }
                self.eval_body(args.get(2..).unwrap_or(&[]), &inner)?
            }
            "seqrep" => self.rep(args, env, true)?,
            "simrep" => self.rep(args, env, false)?,
            "defvar" | "defparameter" => {
                let Some(Value::Sym(s)) = args.first() else {
                    return rt("defvar needs a symbol");
                };
                if name == "defparameter" || self.env.get(s).is_none() {
                    let v = match args.get(1) {
                        Some(e) => self.eval(e, env)?,
                        None => Value::Nil,
                    };
                    self.env.define(s, v);
                }
                Value::Sym(s.clone())
            }
            "flet" | "labels" => {
                let inner = env.child();
                for b in args.first().and_then(|a| a.as_list()).unwrap_or(&[]) {
                    let Some(bl) = b.as_list() else { continue };
                    let Some(Value::Sym(fname)) = bl.first() else {
                        continue;
                    };
                    let f = self.parse_lambda(
                        bl.get(1).unwrap_or(&Value::Nil),
                        bl.get(2..).unwrap_or(&[]),
                        &inner,
                    )?;
                    inner.define(fname, f);
                }
                self.eval_body(&args[1..], &inner)?
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    fn bind_lets(
        &mut self,
        bindings: &Value,
        outer: &Env,
        inner: &Env,
        sequential: bool,
    ) -> NyResult<()> {
        let Some(bs) = bindings.as_list() else {
            return rt("let bindings");
        };
        let mut pending = Vec::new();
        for b in bs {
            match b {
                Value::Sym(s) => inner.define(s, Value::Nil),
                Value::List(pair) => {
                    let Some(Value::Sym(s)) = pair.first() else {
                        return rt("let binding");
                    };
                    let v = self.eval(
                        pair.get(1).unwrap_or(&Value::Nil),
                        if sequential { inner } else { outer },
                    )?;
                    if sequential {
                        inner.define(s, v);
                    } else {
                        pending.push((s.clone(), v));
                    }
                }
                _ => return rt("let binding"),
            }
        }
        for (s, v) in pending {
            inner.define(&s, v);
        }
        Ok(())
    }

    fn set_place(&mut self, place: &Value, v: Value, env: &Env) -> NyResult<()> {
        match place {
            Value::Sym(s) => {
                env.set(s, v);
                Ok(())
            }
            Value::List(l) => match l.first() {
                Some(Value::Sym(h)) if h == "aref" => {
                    let arr = self.eval(&l[1], env)?;
                    let i = int_of(&self.eval(&l[2], env)?)? as usize;
                    match arr {
                        Value::Array(a) => {
                            let mut a = a.borrow_mut();
                            if i >= a.len() {
                                return rt("aref index out of range");
                            }
                            a[i] = v;
                            Ok(())
                        }
                        _ => rt("setf aref on a non-array"),
                    }
                }
                Some(Value::Sym(h)) if h == "get" => {
                    let s = self.eval(&l[1], env)?;
                    let p = self.eval(&l[2], env)?;
                    if let (Value::Sym(s), Value::Sym(p)) = (s, p) {
                        self.put_prop(&s, &p, v);
                    }
                    Ok(())
                }
                Some(Value::Sym(h)) if h == "nth" => {
                    let i = int_of(&self.eval(&l[1], env)?)? as usize;
                    let list = self.eval(&l[2], env)?;
                    let mut items = list.as_list().unwrap_or(&[]).to_vec();
                    if i < items.len() {
                        items[i] = v;
                    }
                    self.set_place(&l[2], Value::list(items), env)
                }
                Some(Value::Sym(h)) if h == "car" || h == "cdr" || h == "first" || h == "rest" => {
                    let list = self.eval(&l[1], env)?;
                    let items = list.as_list().unwrap_or(&[]).to_vec();
                    let nv = if h == "car" || h == "first" {
                        let mut it = items;
                        if it.is_empty() {
                            it.push(v);
                        } else {
                            it[0] = v;
                        }
                        Value::list(it)
                    } else {
                        let mut it = vec![items.first().cloned().unwrap_or(Value::Nil)];
                        it.extend(v.as_list().unwrap_or(&[]).iter().cloned());
                        Value::list(it)
                    };
                    self.set_place(&l[1], nv, env)
                }
                _ => rt("unsupported setf place"),
            },
            _ => rt("unsupported setf place"),
        }
    }

    // ---------------------------------------------------------------- builtins

    fn builtin(&mut self, name: &str, args: Vec<Value>) -> NyResult<Value> {
        // Multichannel expansion: a sound primitive given an array of sounds applies per channel
        // (not the primitives that take a sample array as an argument or return one).
        if is_sound_op(name)
            && !matches!(
                name,
                "snd-from-array" | "snd-fetch-array" | "snd-samples" | "snd-fetch" | "snd-pwl"
            )
        {
            if let Some(n) = args.iter().find_map(|a| match a {
                Value::Array(arr) => Some(arr.borrow().len()),
                _ => None,
            }) {
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let chan: Vec<Value> = args
                        .iter()
                        .map(|a| match a {
                            Value::Array(arr) => arr.borrow().get(i).cloned().unwrap_or(Value::Nil),
                            o => o.clone(),
                        })
                        .collect();
                    out.push(self.builtin(name, chan)?);
                }
                return Ok(Value::Array(Rc::new(std::cell::RefCell::new(out))));
            }
        }
        let a = |i: usize| args.get(i).cloned().unwrap_or(Value::Nil);
        let v = match name {
            "+" | "-" | "*" | "/" | "min" | "max" | "rem" | "mod" | "logand" | "logior" => {
                self.arith(name, args)?
            }
            "abs" => match a(0) {
                Value::Int(i) => Value::Int(i.abs()),
                o => Value::Float(num_of(&o)?.abs()),
            },
            "sqrt" => Value::Float(num_of(&a(0))?.sqrt()),
            "expt" | "power" => {
                let (b, e) = (a(0), a(1));
                if let (Value::Int(b), Value::Int(e)) = (&b, &e) {
                    if *e >= 0 {
                        Value::Int(b.pow(*e as u32))
                    } else {
                        Value::Float((*b as f64).powi(*e as i32))
                    }
                } else {
                    Value::Float(num_of(&b)?.powf(num_of(&e)?))
                }
            }
            "exp" => Value::Float(num_of(&a(0))?.exp()),
            "log" => Value::Float(num_of(&a(0))?.ln()),
            "sin" => Value::Float(num_of(&a(0))?.sin()),
            "cos" => Value::Float(num_of(&a(0))?.cos()),
            "tan" => Value::Float(num_of(&a(0))?.tan()),
            "atan" => match args.len() {
                2 => Value::Float(num_of(&a(0))?.atan2(num_of(&a(1))?)),
                _ => Value::Float(num_of(&a(0))?.atan()),
            },
            "truncate" => Value::Int(num_of(&a(0))?.trunc() as i64),
            "round" => Value::Int(num_of(&a(0))?.round() as i64),
            "floor" => Value::Int(num_of(&a(0))?.floor() as i64),
            "ceiling" => Value::Int(num_of(&a(0))?.ceil() as i64),
            "float" => Value::Float(num_of(&a(0))?),
            "1+" => self.arith("+", vec![a(0), Value::Int(1)])?,
            "1-" => self.arith("-", vec![a(0), Value::Int(1)])?,
            "ash" => {
                let (x, n) = (int_of(&a(0))?, int_of(&a(1))?);
                Value::Int(if n >= 0 { x << n } else { x >> (-n) })
            }
            "random" => {
                self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let r = (self.rng >> 8) as f64 / (1u32 << 24) as f64;
                match a(0) {
                    Value::Int(n) => Value::Int(((r * n as f64) as i64).min(n - 1).max(0)),
                    o => Value::Float(r * num_of(&o)?),
                }
            }
            "rrandom" => {
                self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Value::Float((self.rng >> 8) as f64 / (1u32 << 24) as f64)
            }
            "=" | "/=" | "<" | ">" | "<=" | ">=" => {
                let mut ok = true;
                for w in args.windows(2) {
                    let (x, y) = (&w[0], &w[1]);
                    let r = if x.is_number() && y.is_number() {
                        let (x, y) = (num_of(x)?, num_of(y)?);
                        match name {
                            "=" => x == y,
                            "/=" => x != y,
                            "<" => x < y,
                            ">" => x > y,
                            "<=" => x <= y,
                            _ => x >= y,
                        }
                    } else if let (Value::Str(x), Value::Str(y)) = (x, y) {
                        match name {
                            "=" => x == y,
                            "/=" => x != y,
                            "<" => x < y,
                            ">" => x > y,
                            "<=" => x <= y,
                            _ => x >= y,
                        }
                    } else {
                        return rt(format!(
                            "{name}: expected numbers, got {} and {}",
                            x.type_name(),
                            y.type_name()
                        ));
                    };
                    ok &= r;
                }
                bool_v(ok)
            }
            "eq" | "eql" | "equal" => bool_v(a(0) == a(1)),
            "null" | "not" => bool_v(!a(0).truthy()),
            "atom" => bool_v(!matches!(a(0), Value::List(_))),
            "listp" => bool_v(matches!(a(0), Value::List(_) | Value::Nil)),
            "consp" => bool_v(matches!(a(0), Value::List(_))),
            "numberp" => bool_v(a(0).is_number()),
            "integerp" => bool_v(matches!(a(0), Value::Int(_))),
            "floatp" => bool_v(matches!(a(0), Value::Float(_))),
            "stringp" => bool_v(matches!(a(0), Value::Str(_))),
            "symbolp" => bool_v(matches!(a(0), Value::Sym(_) | Value::Nil | Value::T)),
            "soundp" | "sound-p" => bool_v(matches!(a(0), Value::Sound(_))),
            "arrayp" => bool_v(matches!(a(0), Value::Array(_))),
            "functionp" => bool_v(matches!(a(0), Value::Lambda(_) | Value::Builtin(_))),
            "boundp" => match a(0) {
                Value::Sym(s) => bool_v(self.env.get(&s).is_some()),
                _ => Value::Nil,
            },
            "fboundp" => match a(0) {
                Value::Sym(s) => bool_v(
                    matches!(self.env.get(&s), Some(Value::Lambda(_)))
                        || BUILTINS.contains(&s.as_str()),
                ),
                _ => Value::Nil,
            },
            "zerop" => bool_v(num_of(&a(0))? == 0.0),
            "plusp" => bool_v(num_of(&a(0))? > 0.0),
            "minusp" => bool_v(num_of(&a(0))? < 0.0),
            "evenp" => bool_v(int_of(&a(0))? % 2 == 0),
            "oddp" => bool_v(int_of(&a(0))? % 2 != 0),
            // lists
            "car" | "first" => list_of(&a(0))?.first().cloned().unwrap_or(Value::Nil),
            "cdr" | "rest" => Value::list(list_of(&a(0))?.get(1..).unwrap_or(&[]).to_vec()),
            "cadr" | "second" => list_of(&a(0))?.get(1).cloned().unwrap_or(Value::Nil),
            "caddr" | "third" => list_of(&a(0))?.get(2).cloned().unwrap_or(Value::Nil),
            "cadddr" | "fourth" => list_of(&a(0))?.get(3).cloned().unwrap_or(Value::Nil),
            "cddr" => Value::list(list_of(&a(0))?.get(2..).unwrap_or(&[]).to_vec()),
            "cdddr" => Value::list(list_of(&a(0))?.get(3..).unwrap_or(&[]).to_vec()),
            "caar" => list_of(&list_of(&a(0))?.first().cloned().unwrap_or(Value::Nil))?
                .first()
                .cloned()
                .unwrap_or(Value::Nil),
            "cdar" => Value::list(
                list_of(&list_of(&a(0))?.first().cloned().unwrap_or(Value::Nil))?
                    .get(1..)
                    .unwrap_or(&[])
                    .to_vec(),
            ),
            "cons" => {
                let mut items = vec![a(0)];
                items.extend(a(1).as_list().unwrap_or(&[]).iter().cloned());
                Value::list(items)
            }
            "list" => Value::list(args),
            "append" => {
                let mut items = Vec::new();
                for x in &args {
                    items.extend(list_of(x)?.iter().cloned());
                }
                Value::list(items)
            }
            "reverse" => {
                let mut items = list_of(&a(0))?.to_vec();
                items.reverse();
                Value::list(items)
            }
            "length" => match a(0) {
                Value::Str(s) => Value::Int(s.chars().count() as i64),
                Value::Array(arr) => Value::Int(arr.borrow().len() as i64),
                o => Value::Int(list_of(&o)?.len() as i64),
            },
            "nth" => {
                let i = int_of(&a(0))?;
                list_of(&a(1))?
                    .get(i.max(0) as usize)
                    .cloned()
                    .unwrap_or(Value::Nil)
            }
            "nthcdr" => {
                let i = int_of(&a(0))?.max(0) as usize;
                Value::list(list_of(&a(1))?.get(i..).unwrap_or(&[]).to_vec())
            }
            "last" => {
                let av = a(0);
                let l = list_of(&av)?;
                match l.last() {
                    Some(v) => Value::list(vec![v.clone()]),
                    None => Value::Nil,
                }
            }
            "butlast" => {
                let av = a(0);
                let l = list_of(&av)?;
                let n = args.get(1).map(int_of).transpose()?.unwrap_or(1).max(0) as usize;
                Value::list(l[..l.len().saturating_sub(n)].to_vec())
            }
            "copy-list" => a(0),
            "member" => {
                let av = a(1);
                let l = list_of(&av)?;
                match l.iter().position(|x| *x == a(0)) {
                    Some(i) => Value::list(l[i..].to_vec()),
                    None => Value::Nil,
                }
            }
            "position" => {
                let av = a(1);
                let l = list_of(&av)?;
                match l.iter().position(|x| *x == a(0)) {
                    Some(i) => Value::Int(i as i64),
                    None => Value::Nil,
                }
            }
            "find" => {
                let av = a(1);
                let l = list_of(&av)?;
                l.iter()
                    .find(|x| **x == a(0))
                    .cloned()
                    .unwrap_or(Value::Nil)
            }
            "assoc" => {
                let av = a(1);
                let l = list_of(&av)?;
                l.iter()
                    .find(|p| {
                        p.as_list()
                            .and_then(|p| p.first())
                            .map(|k| *k == a(0))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .unwrap_or(Value::Nil)
            }
            "remove" => {
                let av = a(1);
                let l = list_of(&av)?;
                Value::list(l.iter().filter(|x| **x != a(0)).cloned().collect())
            }
            "remove-if" | "remove-if-not" => {
                let f = a(0);
                let l = list_of(&a(1))?.to_vec();
                let mut out = Vec::new();
                for x in l {
                    let keep = self.apply(&f, vec![x.clone()])?.truthy();
                    if keep == (name == "remove-if-not") {
                        out.push(x);
                    }
                }
                Value::list(out)
            }
            "mapcar" | "mapc" => {
                let f = a(0);
                let lists: Vec<Vec<Value>> = args[1..]
                    .iter()
                    .map(|l| list_of(l).map(|l| l.to_vec()))
                    .collect::<NyResult<_>>()?;
                let n = lists.iter().map(|l| l.len()).min().unwrap_or(0);
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let call: Vec<Value> = lists.iter().map(|l| l[i].clone()).collect();
                    out.push(self.apply(&f, call)?);
                }
                if name == "mapc" {
                    a(1)
                } else {
                    Value::list(out)
                }
            }
            "every" | "some" => {
                let f = a(0);
                let l = list_of(&a(1))?.to_vec();
                let mut result = if name == "every" {
                    Value::T
                } else {
                    Value::Nil
                };
                for x in l {
                    let r = self.apply(&f, vec![x])?;
                    if name == "every" && !r.truthy() {
                        result = Value::Nil;
                        break;
                    }
                    if name == "some" && r.truthy() {
                        result = r;
                        break;
                    }
                }
                result
            }
            "reduce" => {
                let f = a(0);
                let l = list_of(&a(1))?.to_vec();
                let mut it = l.into_iter();
                let mut acc = it.next().unwrap_or(Value::Nil);
                for x in it {
                    acc = self.apply(&f, vec![acc, x])?;
                }
                acc
            }
            "sort" => {
                let f = a(1);
                let mut l = list_of(&a(0))?.to_vec();
                // insertion sort with the predicate (lists are short in plug-ins)
                for i in 1..l.len() {
                    let mut j = i;
                    while j > 0 {
                        let less = self
                            .apply(&f, vec![l[j].clone(), l[j - 1].clone()])?
                            .truthy();
                        if !less {
                            break;
                        }
                        l.swap(j, j - 1);
                        j -= 1;
                    }
                }
                Value::list(l)
            }
            "funcall" => {
                let f = a(0);
                self.apply(&f, args[1..].to_vec())?
            }
            "apply" => {
                let f = a(0);
                let mut call: Vec<Value> = args[1..args.len().saturating_sub(1)].to_vec();
                if args.len() > 1 {
                    call.extend(list_of(&args[args.len() - 1])?.iter().cloned());
                }
                self.apply(&f, call)?
            }
            // arrays
            "make-array" => {
                let n = int_of(&a(0))?.max(0) as usize;
                Value::Array(Rc::new(std::cell::RefCell::new(vec![Value::Nil; n])))
            }
            "vector" => Value::Array(Rc::new(std::cell::RefCell::new(args))),
            "aref" => match a(0) {
                Value::Array(arr) => {
                    let i = int_of(&a(1))?;
                    arr.borrow()
                        .get(i.max(0) as usize)
                        .cloned()
                        .unwrap_or(Value::Nil)
                }
                Value::Str(s) => {
                    let i = int_of(&a(1))?.max(0) as usize;
                    s.chars()
                        .nth(i)
                        .map(|c| Value::Str(c.to_string()))
                        .unwrap_or(Value::Nil)
                }
                _ => return rt("aref on a non-array"),
            },
            // strings
            "strcat" => Value::Str(args.iter().map(str_of).collect::<Vec<_>>().join("")),
            "string" => Value::Str(str_of(&a(0))),
            "string-upcase" | "nstring-upcase" => Value::Str(str_of(&a(0)).to_uppercase()),
            "string-downcase" | "nstring-downcase" => Value::Str(str_of(&a(0)).to_lowercase()),
            "string-trim" => Value::Str(
                str_of(&a(1))
                    .trim_matches(|c| str_of(&a(0)).contains(c))
                    .to_string(),
            ),
            "string-left-trim" => Value::Str(
                str_of(&a(1))
                    .trim_start_matches(|c| str_of(&a(0)).contains(c))
                    .to_string(),
            ),
            "string-right-trim" => Value::Str(
                str_of(&a(1))
                    .trim_end_matches(|c| str_of(&a(0)).contains(c))
                    .to_string(),
            ),
            "string=" | "string-equal" => bool_v(if name == "string=" {
                str_of(&a(0)) == str_of(&a(1))
            } else {
                str_of(&a(0)).eq_ignore_ascii_case(&str_of(&a(1)))
            }),
            "string<" => bool_v(str_of(&a(0)) < str_of(&a(1))),
            "string-search" => {
                let (pat, s) = (str_of(&a(0)), str_of(&a(1)));
                match s.find(&pat) {
                    Some(i) => Value::Int(i as i64),
                    None => Value::Nil,
                }
            }
            "char" => {
                let i = int_of(&a(1))?.max(0) as usize;
                str_of(&a(0))
                    .chars()
                    .nth(i)
                    .map(|c| Value::Str(c.to_string()))
                    .unwrap_or(Value::Nil)
            }
            "char-code" | "char-int" => {
                Value::Int(str_of(&a(0)).chars().next().map(|c| c as i64).unwrap_or(0))
            }
            "code-char" | "int-char" => Value::Str(
                char::from_u32(int_of(&a(0))? as u32)
                    .unwrap_or('?')
                    .to_string(),
            ),
            "digit-char-p" => match str_of(&a(0)).chars().next().and_then(|c| c.to_digit(10)) {
                Some(d) => Value::Int(d as i64),
                None => Value::Nil,
            },
            "subseq" => {
                let s = str_of(&a(0));
                let start = int_of(&a(1))?.max(0) as usize;
                let end = args
                    .get(2)
                    .filter(|v| v.truthy())
                    .map(int_of)
                    .transpose()?
                    .map(|e| e as usize)
                    .unwrap_or(s.chars().count());
                Value::Str(
                    s.chars()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect(),
                )
            }
            "string-rep" => {
                let s = str_of(&a(0));
                let n = int_of(&a(1))?.max(0) as usize;
                Value::Str(s.repeat(n))
            }
            "intern" => Value::sym(&str_of(&a(0))),
            "symbol-name" => match a(0) {
                Value::Sym(s) => Value::Str(s.to_ascii_uppercase()),
                Value::Nil => Value::Str("NIL".into()),
                Value::T => Value::Str("T".into()),
                o => return rt(format!("symbol-name: not a symbol: {}", o.display())),
            },
            "read-from-string" => {
                let s = str_of(&a(0));
                Reader::new(&s).read().map_err(NyError::Read)?
            }
            "parse-integer" => match str_of(&a(0)).trim().parse::<i64>() {
                Ok(i) => Value::Int(i),
                Err(_) => Value::Nil,
            },
            "format" => {
                let dest = a(0);
                let s = format_str(&str_of(&a(1)), &args[2..]);
                if dest.truthy() {
                    self.output.push_str(&s);
                    Value::Nil
                } else {
                    Value::Str(s)
                }
            }
            "print" | "prin1" => {
                self.output.push_str(&a(0).display());
                self.output.push('\n');
                a(0)
            }
            "princ" => {
                self.output.push_str(&a(0).display());
                a(0)
            }
            "terpri" => {
                self.output.push('\n');
                Value::Nil
            }
            "error" => {
                let msg = format_str(&str_of(&a(0)), &args[1..]);
                return rt(format!("error: {msg}"));
            }
            "get" => match (a(0), a(1)) {
                (Value::Sym(s), Value::Sym(p)) => {
                    self.props.get(&(s, p)).cloned().unwrap_or(Value::Nil)
                }
                _ => Value::Nil,
            },
            "putprop" => {
                if let (Value::Sym(s), Value::Sym(p)) = (a(0), a(2)) {
                    self.put_prop(&s, &p, a(1));
                }
                a(1)
            }
            "remprop" => {
                if let (Value::Sym(s), Value::Sym(p)) = (a(0), a(1)) {
                    self.props.remove(&(s, p));
                }
                Value::Nil
            }
            "s-plot" | "setf-hack" => Value::Nil,
            "_" => a(0),
            "string-append" => Value::Str(args.iter().map(str_of).collect::<Vec<_>>().join("")),
            "char=" | "char-equal" | "char/=" | "char<" | "char>" => {
                let (x, y) = (str_of(&a(0)), str_of(&a(1)));
                let (x, y) = if name == "char-equal" {
                    (x.to_lowercase(), y.to_lowercase())
                } else {
                    (x, y)
                };
                bool_v(match name {
                    "char/=" => x != y,
                    "char<" => x < y,
                    "char>" => x > y,
                    _ => x == y,
                })
            }
            "char-upcase" => Value::Str(str_of(&a(0)).to_uppercase()),
            "char-downcase" => Value::Str(str_of(&a(0)).to_lowercase()),
            "alphanumericp" => bool_v(str_of(&a(0)).chars().all(|c| c.is_alphanumeric())),
            "display" => {
                let line = args
                    .iter()
                    .map(|v| v.display())
                    .collect::<Vec<_>>()
                    .join(" : ");
                self.output.push_str(&line);
                self.output.push('\n');
                a(0)
            }
            "snd-fft" | "snd-ifft" | "pv-time-pitch" | "phasevocoder" => {
                return rt(format!(
                "{} is not supported yet by EthnoFlow's Nyquist (FFT and phase-vocoder primitives)",
                name.to_ascii_uppercase()
            ))
            }
            "step-to-hz" => Value::Float(sound::step_to_hz(num_of(&a(0))?)),
            "hz-to-step" => Value::Float(69.0 + 12.0 * (num_of(&a(0))?.max(1e-9) / 440.0).log2()),
            "db-to-linear" => match a(0) {
                Value::Sound(s) => Value::Sound(Rc::new(sound::map1(&s, |v| 10f32.powf(v / 20.0)))),
                o => Value::Float(10f64.powf(num_of(&o)? / 20.0)),
            },
            "linear-to-db" => match a(0) {
                Value::Sound(s) => Value::Sound(Rc::new(sound::map1(&s, |v| {
                    20.0 * v.abs().max(1e-9).log10()
                }))),
                o => Value::Float(20.0 * num_of(&o)?.abs().max(1e-9).log10()),
            },
            "get-duration" => Value::Float(num_of(&a(0))? * self.stretch),
            "get-loud" | "get-transpose" => Value::Float(0.0),
            "get-sustain" => Value::Float(1.0),
            "get-tempo" => Value::Float(100.0),
            "eval" => {
                let expr = a(0);
                let genv = self.env.clone();
                self.eval(&expr, &genv)?
            }
            "make-string-input-stream" => a(0),
            "read" => {
                // (read string-stream [eof-error eof-value]): the first form of the text
                let text = str_of(&a(0));
                match Reader::new(&text).read() {
                    Ok(v) => v,
                    Err(_) => a(2),
                }
            }
            "local-to-global" => Value::Float(num_of(&a(0))?),
            "sound-srate" | "control-srate" | "abs-env" | "snd-copy" | "cue" | "sound" => a(0),
            "const" => {
                let dur = args.get(1).map(num_of).transpose()?.unwrap_or(1.0) * self.stretch;
                let k = num_of(&a(0))? as f32;
                Value::Sound(Rc::new(sound::map1(&sound::rest(self.sr, dur), |_| k)))
            }
            "multichan-expand" => {
                let f = a(0);
                let rest = args[1..].to_vec();
                let n = rest.iter().find_map(|x| match x {
                    Value::Array(arr) => Some(arr.borrow().len()),
                    _ => None,
                });
                match n {
                    None => self.apply(&f, rest)?,
                    Some(n) => {
                        let mut out = Vec::with_capacity(n);
                        for i in 0..n {
                            let chan: Vec<Value> = rest
                                .iter()
                                .map(|x| match x {
                                    Value::Array(arr) => {
                                        arr.borrow().get(i).cloned().unwrap_or(Value::Nil)
                                    }
                                    o => o.clone(),
                                })
                                .collect();
                            out.push(self.apply(&f, chan)?);
                        }
                        Value::Array(Rc::new(std::cell::RefCell::new(out)))
                    }
                }
            }
            _ if is_sound_op(name) => self.sound_builtin(name, args)?,
            _ => {
                return rt(format!(
                    "unsupported function: {}",
                    name.to_ascii_uppercase()
                ))
            }
        };
        Ok(v)
    }

    fn arith(&mut self, op: &str, args: Vec<Value>) -> NyResult<Value> {
        if args
            .iter()
            .any(|a| matches!(a, Value::Sound(_) | Value::Array(_)))
        {
            return match op {
                "+" => self.sound_builtin("sum", args),
                "-" => self.sound_builtin("diff", args),
                "*" => self.sound_builtin("mult", args),
                _ => rt(format!("{op} on a sound")),
            };
        }
        let all_int = args.iter().all(|a| matches!(a, Value::Int(_)));
        if args.is_empty() {
            return Ok(match op {
                "+" => Value::Int(0),
                "*" => Value::Int(1),
                _ => return rt(format!("{op} needs arguments")),
            });
        }
        if all_int {
            let mut it = args.iter().map(|a| int_of(a).unwrap());
            let mut acc = it.next().unwrap();
            if args.len() == 1 && op == "-" {
                return Ok(Value::Int(-acc));
            }
            if args.len() == 1 && op == "/" {
                return Ok(Value::Int(if acc == 0 { 0 } else { 1 / acc }));
            }
            for x in it {
                acc = match op {
                    "+" => acc.wrapping_add(x),
                    "-" => acc.wrapping_sub(x),
                    "*" => acc.wrapping_mul(x),
                    "/" => {
                        if x == 0 {
                            return rt("division by zero");
                        }
                        acc / x
                    }
                    "rem" => {
                        if x == 0 {
                            return rt("division by zero");
                        }
                        acc % x
                    }
                    "mod" => {
                        if x == 0 {
                            return rt("division by zero");
                        }
                        acc.rem_euclid(x)
                    }
                    "min" => acc.min(x),
                    "max" => acc.max(x),
                    "logand" => acc & x,
                    "logior" => acc | x,
                    _ => unreachable!(),
                };
            }
            return Ok(Value::Int(acc));
        }
        let mut it = args.iter().map(num_of);
        let mut acc = it.next().unwrap()?;
        if args.len() == 1 && op == "-" {
            return Ok(Value::Float(-acc));
        }
        if args.len() == 1 && op == "/" {
            return Ok(Value::Float(1.0 / acc));
        }
        for x in it {
            let x = x?;
            acc = match op {
                "+" => acc + x,
                "-" => acc - x,
                "*" => acc * x,
                "/" => {
                    if x == 0.0 {
                        return rt("division by zero");
                    }
                    acc / x
                }
                "rem" => acc % x,
                "mod" => acc.rem_euclid(x),
                "min" => acc.min(x),
                "max" => acc.max(x),
                _ => return rt(format!("{op} needs integers")),
            };
        }
        Ok(Value::Float(acc))
    }

    fn sound_builtin(&mut self, name: &str, args: Vec<Value>) -> NyResult<Value> {
        let sr = self.sr;
        let st = self.stretch;
        let a = |i: usize| args.get(i).cloned().unwrap_or(Value::Nil);
        let snd = |v: &Value| -> NyResult<Rc<Sound>> {
            match v {
                Value::Sound(s) => Ok(s.clone()),
                o => rt(format!("{name}: expected a sound, got {}", o.type_name())),
            }
        };
        let v = match name {
            "mult" | "prod" => {
                let mut acc: Option<Sound> = None;
                let mut k = 1.0f32;
                for x in &args {
                    match x {
                        Value::Sound(s) => {
                            acc = Some(match acc {
                                None => (**s).clone(),
                                Some(p) => sound::mult(&p, s),
                            })
                        }
                        o => k *= num_of(o)? as f32,
                    }
                }
                match acc {
                    Some(s) => Value::Sound(Rc::new(sound::scale(k, &s))),
                    None => Value::Float(k as f64),
                }
            }
            "sum" | "snd-add" => {
                let mut acc: Option<Sound> = None;
                let mut k = 0.0f32;
                for x in &args {
                    match x {
                        Value::Sound(s) => {
                            acc = Some(match acc {
                                None => (**s).clone(),
                                Some(p) => sound::map2(&p, s, |x, y| x + y),
                            })
                        }
                        o => k += num_of(o)? as f32,
                    }
                }
                match acc {
                    Some(s) => Value::Sound(Rc::new(sound::offset(k, &s))),
                    None => Value::Float(k as f64),
                }
            }
            "diff" => {
                if !args.iter().any(|x| matches!(x, Value::Sound(_))) {
                    // Plain numbers: (diff 1 x) = 1 - x
                    let nums: Vec<Value> = args.clone();
                    return self.arith("-", nums);
                }
                let first = a(0);
                let rest: Vec<Value> = args[1..].to_vec();
                let mut acc = match &first {
                    Value::Sound(s) => (**s).clone(),
                    o => {
                        // number - sound
                        let k = num_of(o)? as f32;
                        let s = snd(&a(1))?;
                        return Ok(Value::Sound(Rc::new(sound::map1(&s, |v| k - v))));
                    }
                };
                if rest.is_empty() {
                    acc = sound::scale(-1.0, &acc);
                }
                for x in &rest {
                    acc = match x {
                        Value::Sound(s) => sound::map2(&acc, s, |x, y| x - y),
                        o => sound::offset(-(num_of(o)? as f32), &acc),
                    };
                }
                Value::Sound(Rc::new(acc))
            }
            "scale" | "snd-scale" => {
                Value::Sound(Rc::new(sound::scale(num_of(&a(0))? as f32, &*snd(&a(1))?)))
            }
            "scale-db" => Value::Sound(Rc::new(sound::scale(
                10f32.powf(num_of(&a(0))? as f32 / 20.0),
                &*snd(&a(1))?,
            ))),
            "snd-offset" => {
                Value::Sound(Rc::new(sound::offset(num_of(&a(1))? as f32, &*snd(&a(0))?)))
            }
            "snd-prod" => Value::Sound(Rc::new(sound::mult(&*snd(&a(0))?, &*snd(&a(1))?))),
            "hp" => Value::Sound(Rc::new(sound::hp(&*snd(&a(0))?, cutoff(&a(1))?))),
            "lp" | "tone" => Value::Sound(Rc::new(sound::lp(&*snd(&a(0))?, cutoff(&a(1))?))),
            "atone" => {
                let s = snd(&a(0))?;
                let low = sound::lp(&s, cutoff(&a(1))?);
                Value::Sound(Rc::new(sound::map2(&s, &low, |x, y| x - y)))
            }
            "lowpass2" | "highpass2" | "bandpass2" | "notch2" | "reson" | "areson" => {
                let s = snd(&a(0))?;
                let hz = cutoff(&a(1))?;
                let q = args
                    .get(2)
                    .map(num_of)
                    .transpose()?
                    .unwrap_or(std::f64::consts::FRAC_1_SQRT_2) as f32;
                let kind = match name {
                    "lowpass2" => BiquadKind::Low,
                    "highpass2" => BiquadKind::High,
                    "notch2" | "areson" => BiquadKind::Notch,
                    _ => BiquadKind::Band,
                };
                let q = if name == "reson" || name == "areson" {
                    (hz / q.max(1.0)).max(0.1)
                } else {
                    q
                };
                Value::Sound(Rc::new(sound::biquad(&s, kind, hz, q)))
            }
            "lowpass4" | "lowpass6" | "lowpass8" | "highpass4" | "highpass6" | "highpass8" => {
                let mut s = (*snd(&a(0))?).clone();
                let hz = cutoff(&a(1))?;
                let n: usize = name[name.len() - 1..].parse::<usize>().unwrap_or(4) / 2;
                let kind = if name.starts_with("low") {
                    BiquadKind::Low
                } else {
                    BiquadKind::High
                };
                for _ in 0..n {
                    s = sound::biquad(&s, kind, hz, std::f32::consts::FRAC_1_SQRT_2);
                }
                Value::Sound(Rc::new(s))
            }
            "eq-lowshelf" | "eq-highshelf" | "eq-band" => {
                let s = snd(&a(0))?;
                let hz = cutoff(&a(1))?;
                let gain_db = num_of(&a(2))? as f32;
                let g = 10f32.powf(gain_db / 20.0);
                let kind = match name {
                    "eq-lowshelf" => BiquadKind::Low,
                    "eq-highshelf" => BiquadKind::High,
                    _ => BiquadKind::Band,
                };
                let q = if name == "eq-band" {
                    args.get(3).map(num_of).transpose()?.unwrap_or(1.0) as f32
                } else {
                    std::f32::consts::FRAC_1_SQRT_2
                };
                let band = sound::biquad(&s, kind, hz, q);
                Value::Sound(Rc::new(sound::map2(&s, &band, move |x, b| {
                    x + (g - 1.0) * b
                })))
            }
            "pwl" | "pwl-list" => {
                let nums = flat_nums(&args, name == "pwl-list")?;
                let mut pts: Vec<(f64, f64)> = nums
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| (c[0] * st, c[1]))
                    .collect();
                if nums.len() % 2 == 1 {
                    pts.push((nums[nums.len() - 1] * st, 0.0));
                }
                Value::Sound(Rc::new(sound::pwl(sr, &pts)))
            }
            "pwlv" | "pwlv-list" => {
                let nums = flat_nums(&args, name == "pwlv-list")?;
                // v1 t2 v2 t3 v3 … tn vn
                let mut pts: Vec<(f64, f64)> = Vec::new();
                if let Some(v1) = nums.first() {
                    pts.push((0.0, *v1));
                }
                for c in nums[1.min(nums.len())..].chunks(2) {
                    if c.len() == 2 {
                        pts.push((c[0] * st, c[1]));
                    }
                }
                Value::Sound(Rc::new(sound::pwlv(sr, &pts)))
            }
            "env" => {
                let n: Vec<f64> = args.iter().map(num_of).collect::<NyResult<_>>()?;
                if n.len() < 6 {
                    return rt("env needs t1 t2 t4 l1 l2 l3 [dur]");
                }
                let dur = n.get(6).copied().unwrap_or(1.0) * st;
                Value::Sound(Rc::new(sound::env(
                    sr,
                    n[0] * st,
                    n[1] * st,
                    n[2] * st,
                    n[3],
                    n[4],
                    n[5],
                    dur,
                )))
            }
            "osc" => {
                // (osc pitch [dur table phase])
                let hz = Value::Float(sound::step_to_hz(num_of(&a(0))?));
                let dur = args.get(1).map(num_of).transpose()?.unwrap_or(1.0) * st;
                let phase = args.get(3).map(num_of).transpose()?.unwrap_or(0.0);
                self.table_osc(&hz, dur, &a(2), phase)?
            }
            "sine" => {
                // (sine step [dur]): a sine at the pitch, like osc with the sine table
                let hz = Value::Float(sound::step_to_hz(num_of(&a(0))?));
                let dur = args.get(1).map(num_of).transpose()?.unwrap_or(1.0) * st;
                self.table_osc(&hz, dur, &Value::Nil, 0.0)?
            }
            "snd-oneshot" => {
                // (snd-oneshot sound onthresh ontime): 1 for ontime seconds after a trigger, else 0
                let s = snd(&a(0))?;
                let thr = num_of(&a(1))? as f32;
                let hold = (num_of(&a(2))? * s.sr).round().max(0.0) as usize;
                let mut left = 0usize;
                let data: Vec<f32> = s
                    .data
                    .iter()
                    .map(|x| {
                        if *x > thr {
                            left = hold;
                        }
                        if left > 0 {
                            left -= 1;
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::at(s.sr, s.start, data)))
            }
            "osc-tri" | "osc-saw" => {
                // (osc-tri hz) / (osc-saw hz): hz in Hertz, one (stretched) second
                let hz = Value::Float(num_of(&a(0))?);
                let shape = if name == "osc-tri" { "tri" } else { "saw" };
                let period = sound::osc(sr, sr / 2048.0, 2048.0 / sr, shape);
                self.table_osc(&hz, st, &Value::Sound(Rc::new(period)), 0.0)?
            }
            "osc-pulse" => {
                // (osc-pulse hz bias): pulse width from bias (-1..1), a number or a sound
                let hz = num_of(&a(0))?;
                let n = (st * sr).round() as usize;
                let bias_s = match a(1) {
                    Value::Sound(s) => Some(s),
                    _ => None,
                };
                let bias_k = match a(1) {
                    Value::Sound(_) => 0.0,
                    o => num_of(&o).unwrap_or(0.0) as f32,
                };
                let data: Vec<f32> = (0..n)
                    .map(|i| {
                        let ph = (i as f64 * hz / sr).fract() as f32;
                        let bias = bias_s
                            .as_ref()
                            .and_then(|s| s.data.get(i).copied())
                            .unwrap_or(bias_k);
                        if ph * 2.0 - 1.0 < bias {
                            1.0
                        } else {
                            -1.0
                        }
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::new(sr, data)))
            }
            "snd-buzz" => {
                // (snd-buzz n sr hz t0 modulation)
                let n = int_of(&a(0))?.max(1) as usize;
                let rate = num_of(&a(1))?;
                let hz = num_of(&a(2))?;
                let t0 = num_of(&a(3))?;
                let m = snd(&a(4))?;
                let data: Vec<f32> = (0..m.data.len())
                    .map(|i| {
                        let t = i as f64 / rate + m.data[i] as f64;
                        (1..=n)
                            .map(|k| (t * hz * k as f64 * std::f64::consts::TAU).sin())
                            .sum::<f64>() as f32
                            / n as f32
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::at(rate, t0, data)))
            }
            "buzz" => {
                // (buzz n hz modulation): n equal harmonics of hz, phase-modulated by `modulation`
                let n = int_of(&a(0))?.max(1) as usize;
                let hz = num_of(&a(1))?;
                let m = snd(&a(2))?;
                let len = m.data.len();
                let data: Vec<f32> = (0..len)
                    .map(|i| {
                        let t = i as f64 / sr + m.data[i] as f64;
                        (1..=n)
                            .map(|k| (t * hz * k as f64 * std::f64::consts::TAU).sin())
                            .sum::<f64>() as f32
                            / n as f32
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::new(sr, data)))
            }
            "loud" => Value::Sound(Rc::new(sound::scale(
                10f32.powf(num_of(&a(0))? as f32 / 20.0),
                &*snd(&a(1))?,
            ))),
            "hzosc" => {
                // (hzosc hz-or-sound [table phase])
                let phase = args.get(2).map(num_of).transpose()?.unwrap_or(0.0);
                self.table_osc(&a(0), st, &a(1), phase)?
            }
            "lfo" => {
                // (lfo hz [dur table phase]) — control-rate in Nyquist; here at the sound rate.
                let dur = args.get(1).map(num_of).transpose()?.unwrap_or(1.0) * st;
                let phase = args.get(3).map(num_of).transpose()?.unwrap_or(0.0);
                self.table_osc(&a(0), dur, &a(2), phase)?
            }
            "fmosc" => {
                // (fmosc pitch modulation [table phase]): hz = base + modulation
                let base = sound::step_to_hz(num_of(&a(0))?) as f32;
                let m = snd(&a(1))?;
                let hz = Value::Sound(Rc::new(sound::offset(base, &m)));
                let phase = args.get(3).map(num_of).transpose()?.unwrap_or(0.0);
                self.table_osc(&hz, m.duration(), &a(2), phase)?
            }
            "build-harmonic" => {
                let h = num_of(&a(0))?;
                let n = int_of(&a(1))?.max(1) as usize;
                let data: Vec<f32> = (0..n)
                    .map(|i| (i as f64 / n as f64 * h * std::f64::consts::TAU).sin() as f32)
                    .collect();
                Value::Sound(Rc::new(Sound::new(sr, data)))
            }
            "maketable" => {
                let s = snd(&a(0))?;
                self.table((*s).clone())
            }
            "exp-dec" => {
                let (hold, halfdec, len) = (
                    num_of(&a(0))? * st,
                    num_of(&a(1))?.max(1e-6) * st,
                    num_of(&a(2))? * st,
                );
                let n = (len * sr).round() as usize;
                let data: Vec<f32> = (0..n)
                    .map(|i| {
                        let t = i as f64 / sr;
                        if t < hold {
                            1.0
                        } else {
                            0.5f64.powf((t - hold) / halfdec) as f32
                        }
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::new(sr, data)))
            }
            "pwev" | "pwev-list" | "pwevr" => {
                // v1 t2 v2 … tn vn with exponential interpolation (values > 0)
                let nums = flat_nums(&args, name == "pwev-list")?;
                let mut pts: Vec<(f64, f64)> = Vec::new();
                if let Some(v1) = nums.first() {
                    pts.push((0.0, *v1));
                }
                for c in nums[1.min(nums.len())..].chunks(2) {
                    if c.len() == 2 {
                        pts.push((c[0] * st, c[1]));
                    }
                }
                Value::Sound(Rc::new(sound::pwev(sr, &pts)))
            }
            "snd-const" => {
                // (snd-const value t0 srate dur)
                let k = num_of(&a(0))? as f32;
                let t0 = num_of(&a(1))?;
                let rate = num_of(&a(2))?;
                let dur = num_of(&a(3))?;
                let n = (dur * rate).round().max(0.0) as usize;
                Value::Sound(Rc::new(Sound::at(rate, t0, vec![k; n])))
            }
            "snd-pwl" => {
                // (snd-pwl t0 sr (n1 v1 n2 v2 … nk)) with sample counts
                let t0 = num_of(&a(0))?;
                let rate = num_of(&a(1))?;
                let nums: Vec<f64> = list_of(&a(2))?
                    .iter()
                    .map(num_of)
                    .collect::<NyResult<_>>()?;
                let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
                for c in nums.chunks(2) {
                    if c.len() == 2 {
                        pts.push((c[0] / rate, c[1]));
                    } else {
                        pts.push((c[0] / rate, 0.0));
                    }
                }
                let mut s = sound::pwlv(rate, &pts);
                s.start = t0;
                Value::Sound(Rc::new(s))
            }
            "snd-biquad" | "biquad" | "biquad-m" => {
                // Nyquist: (snd-biquad x b0 b1 b2 a1 a2 z1 z2) with y = Σb·x + a1·y1 + a2·y2;
                // (biquad x b0 b1 b2 a0 a1 a2) in RBJ form (a0 normalises, a1/a2 subtracted).
                let s = snd(&a(0))?;
                let (b0, b1, b2) = (
                    num_of(&a(1))? as f32,
                    num_of(&a(2))? as f32,
                    num_of(&a(3))? as f32,
                );
                let (b0, b1, b2, a1, a2) = if name == "snd-biquad" {
                    (b0, b1, b2, num_of(&a(4))? as f32, num_of(&a(5))? as f32)
                } else {
                    let a0 = num_of(&a(4))? as f32;
                    let a0 = if a0 == 0.0 { 1.0 } else { a0 };
                    (
                        b0 / a0,
                        b1 / a0,
                        b2 / a0,
                        -(num_of(&a(5))? as f32) / a0,
                        -(num_of(&a(6))? as f32) / a0,
                    )
                };
                Value::Sound(Rc::new(sound::biquad_raw(&s, b0, b1, b2, a1, a2)))
            }
            "snd-follow" | "follow" => {
                // (snd-follow sound floor risetime falltime lookahead)
                let s = snd(&a(0))?;
                let floor = num_of(&a(1))? as f32;
                let rise = num_of(&a(2))?;
                let fall = num_of(&a(3))?;
                Value::Sound(Rc::new(sound::follow(&s, floor, rise, fall)))
            }
            "snd-pluck" | "pluck" => {
                // (snd-pluck sr hz t0 dur final-amp) / (pluck step [dur final-amp])
                let (rate, hz, t0, dur, amp) = if name == "snd-pluck" {
                    (
                        num_of(&a(0))?,
                        num_of(&a(1))?,
                        num_of(&a(2))?,
                        num_of(&a(3))?,
                        num_of(&a(4))?,
                    )
                } else {
                    (
                        sr,
                        sound::step_to_hz(num_of(&a(0))?),
                        0.0,
                        args.get(1).map(num_of).transpose()?.unwrap_or(1.0) * st,
                        args.get(2).map(num_of).transpose()?.unwrap_or(0.001),
                    )
                };
                self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let mut s = sound::pluck(rate, hz, dur, amp, self.rng);
                s.start = t0;
                Value::Sound(Rc::new(s))
            }
            "snd-fetch-array" => {
                // (snd-fetch-array sound len step): the next block, nil at the end
                let s = snd(&a(0))?;
                let len = int_of(&a(1))?.max(0) as usize;
                let step = int_of(&a(2))?.max(1) as usize;
                let key = Rc::as_ptr(&s) as usize;
                let pos = self.streams.get(&key).map(|(_, p)| *p).unwrap_or(0);
                if pos >= s.data.len() {
                    Value::Nil
                } else {
                    let end = (pos + len).min(s.data.len());
                    let items: Vec<Value> = s.data[pos..end]
                        .iter()
                        .map(|v| Value::Float(*v as f64))
                        .collect();
                    self.streams.insert(key, (s.clone(), pos + step));
                    Value::Array(Rc::new(std::cell::RefCell::new(items)))
                }
            }
            "set-logical-stop" | "set-logical-stop-abs" | "snd-set-logical-stop" => a(0),
            "noise" => {
                let dur = args.first().map(num_of).transpose()?.unwrap_or(1.0) * st;
                self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Value::Sound(Rc::new(sound::noise(sr, dur, self.rng)))
            }
            "s-rest" | "s-rest-abs" => {
                let dur = args.first().map(num_of).transpose()?.unwrap_or(1.0)
                    * if name == "s-rest" { st } else { 1.0 };
                Value::Sound(Rc::new(sound::rest(sr, dur)))
            }
            "extract" | "extract-abs" => {
                let (t0, t1) = (num_of(&a(0))?, num_of(&a(1))?);
                Value::Sound(Rc::new(sound::extract(&*snd(&a(2))?, t0, t1)))
            }
            "seq" => {
                let parts: Vec<Sound> = args
                    .iter()
                    .map(|x| snd(x).map(|s| (*s).clone()))
                    .collect::<NyResult<_>>()?;
                Value::Sound(Rc::new(sound::seq(&parts)))
            }
            "sim" => {
                let parts: Vec<Sound> = args
                    .iter()
                    .map(|x| snd(x).map(|s| (*s).clone()))
                    .collect::<NyResult<_>>()?;
                Value::Sound(Rc::new(sound::sim(&parts)))
            }
            "at" | "at-abs" => {
                let t = num_of(&a(0))? * if name == "at" { st } else { 1.0 };
                let s = snd(&a(1))?;
                Value::Sound(Rc::new(Sound {
                    sr: s.sr,
                    start: s.start + t,
                    data: s.data.clone(),
                }))
            }
            "stretch" | "stretch-abs" => {
                let k = num_of(&a(0))?;
                Value::Sound(Rc::new(sound::stretch(&*snd(&a(1))?, k)))
            }
            "peak" | "snd-maxsamp" => Value::Float(sound::peak(&*snd(&a(0))?) as f64),
            "snd-length" => {
                let s = snd(&a(0))?;
                let max = args.get(1).map(int_of).transpose()?.unwrap_or(i64::MAX);
                Value::Int((s.data.len() as i64).min(max))
            }
            "snd-srate" => Value::Float(snd(&a(0))?.sr),
            "snd-t0" => Value::Float(snd(&a(0))?.start),
            "snd-from-array" => {
                let t0 = num_of(&a(0))?;
                let rate = num_of(&a(1))?;
                let data: Vec<f32> = match a(2) {
                    Value::Array(arr) => arr
                        .borrow()
                        .iter()
                        .map(|v| num_of(v).map(|f| f as f32))
                        .collect::<NyResult<_>>()?,
                    o => {
                        return rt(format!(
                            "snd-from-array: expected an array, got {}",
                            o.type_name()
                        ))
                    }
                };
                Value::Sound(Rc::new(Sound {
                    sr: rate,
                    start: t0,
                    data,
                }))
            }
            "snd-samples" => {
                let s = snd(&a(0))?;
                let n = args
                    .get(1)
                    .map(int_of)
                    .transpose()?
                    .unwrap_or(s.data.len() as i64)
                    .max(0) as usize;
                let items: Vec<Value> = s
                    .data
                    .iter()
                    .take(n)
                    .map(|v| Value::Float(*v as f64))
                    .collect();
                Value::Array(Rc::new(std::cell::RefCell::new(items)))
            }
            "snd-fetch" => {
                // Streams one sample per call from this sound object; nil at the end.
                let s = snd(&a(0))?;
                let key = Rc::as_ptr(&s) as usize;
                let pos = self.streams.get(&key).map(|(_, p)| *p).unwrap_or(0);
                match s.data.get(pos) {
                    Some(v) => {
                        let v = *v;
                        self.streams.insert(key, (s.clone(), pos + 1));
                        Value::Float(v as f64)
                    }
                    None => Value::Nil,
                }
            }
            "snd-sref" | "sref" => {
                let s = snd(&a(0))?;
                let t = num_of(&a(1))?;
                let i = ((t - s.start) * s.sr).round().max(0.0) as usize;
                Value::Float(s.data.get(i).copied().unwrap_or(0.0) as f64)
            }
            "s-min" | "s-max" => {
                // Either argument may be the number: (s-max 1 sig) and (s-max sig 1).
                let (first, second) = if matches!(a(0), Value::Sound(_)) {
                    (a(0), a(1))
                } else {
                    (a(1), a(0))
                };
                let s = snd(&first)?;
                match second {
                    Value::Sound(o) => Value::Sound(Rc::new(sound::map2(
                        &s,
                        &o,
                        if name == "s-min" {
                            |x: f32, y: f32| x.min(y)
                        } else {
                            |x: f32, y: f32| x.max(y)
                        },
                    ))),
                    o => {
                        let k = num_of(&o)? as f32;
                        Value::Sound(Rc::new(sound::map1(
                            &s,
                            if name == "s-min" {
                                Box::new(move |x: f32| x.min(k)) as Box<dyn Fn(f32) -> f32>
                            } else {
                                Box::new(move |x: f32| x.max(k))
                            },
                        )))
                    }
                }
            }
            "s-abs" | "snd-abs" => Value::Sound(Rc::new(sound::map1(&*snd(&a(0))?, |x| x.abs()))),
            "s-sqrt" => Value::Sound(Rc::new(sound::map1(&*snd(&a(0))?, |x| x.max(0.0).sqrt()))),
            "s-exp" | "snd-exp" => Value::Sound(Rc::new(sound::map1(&*snd(&a(0))?, |x| x.exp()))),
            "s-log" | "snd-log" => {
                Value::Sound(Rc::new(sound::map1(&*snd(&a(0))?, |x| x.max(1e-9).ln())))
            }
            "clip" => {
                let k = num_of(&a(1))?.abs() as f32;
                match a(0) {
                    Value::Sound(s) => {
                        Value::Sound(Rc::new(sound::map1(&s, move |x| x.clamp(-k, k))))
                    }
                    o => Value::Float(num_of(&o)?.clamp(-(k as f64), k as f64)),
                }
            }
            "shape" => {
                // (shape signal table origin): table maps [-1,1] → value; we approximate with the sound as a lookup.
                let s = snd(&a(0))?;
                let table = snd(&a(1))?;
                let origin = num_of(&a(2))? as f32;
                let n = table.data.len() as f32;
                Value::Sound(Rc::new(sound::map1(&s, move |x| {
                    let idx =
                        ((x + origin) * n / (2.0 * origin.max(1e-6))).clamp(0.0, n - 1.0) as usize;
                    table.data.get(idx).copied().unwrap_or(0.0)
                })))
            }
            "normalize" => {
                let s = snd(&a(0))?;
                let p = sound::peak(&s);
                let target = args.get(1).map(num_of).transpose()?.unwrap_or(1.0) as f32;
                Value::Sound(Rc::new(if p > 0.0 {
                    sound::scale(target / p, &s)
                } else {
                    (*s).clone()
                }))
            }
            "quantize" | "snd-quantize" => {
                let steps = num_of(&a(1))? as f32;
                Value::Sound(Rc::new(sound::map1(&*snd(&a(0))?, move |x| {
                    (x * steps).round() / steps
                })))
            }
            "snd-avg" | "rms" => {
                let s = snd(&a(0))?;
                let block = args.get(1).map(int_of).transpose()?.unwrap_or(1000).max(1) as usize;
                let step = args
                    .get(2)
                    .map(int_of)
                    .transpose()?
                    .unwrap_or(block as i64)
                    .max(1) as usize;
                // op-average = 1, op-peak = 2, op-rms = 3 (Nyquist constants)
                let op = if name == "rms" {
                    3
                } else {
                    match a(3) {
                        Value::Int(i) => i,
                        _ => 1,
                    }
                };
                let data: Vec<f32> = (0..s.data.len())
                    .step_by(step)
                    .map(|i| {
                        let w = &s.data[i..(i + block).min(s.data.len())];
                        match op {
                            3 => (w.iter().map(|v| v * v).sum::<f32>() / w.len().max(1) as f32)
                                .sqrt(),
                            2 => w.iter().fold(0.0f32, |m, v| m.max(v.abs())),
                            _ => w.iter().sum::<f32>() / w.len().max(1) as f32,
                        }
                    })
                    .collect();
                Value::Sound(Rc::new(Sound {
                    sr: s.sr / step as f64,
                    start: s.start,
                    data,
                }))
            }
            "integrate" => {
                let s = snd(&a(0))?;
                let mut acc = 0.0f32;
                let dt = 1.0 / s.sr as f32;
                let data: Vec<f32> = s
                    .data
                    .iter()
                    .map(|x| {
                        acc += x * dt;
                        acc
                    })
                    .collect();
                Value::Sound(Rc::new(Sound::at(s.sr, s.start, data)))
            }
            "slope" => {
                let s = snd(&a(0))?;
                let sr_f = s.sr as f32;
                let data: Vec<f32> = s.data.windows(2).map(|w| (w[1] - w[0]) * sr_f).collect();
                Value::Sound(Rc::new(Sound {
                    sr: s.sr,
                    start: s.start,
                    data,
                }))
            }
            "force-srate" | "resample" => {
                // (force-srate rate sound) but (resample sound rate)
                let (rate, s) = if name == "resample" {
                    (num_of(&a(1))?, snd(&a(0))?)
                } else {
                    (num_of(&a(0))?, snd(&a(1))?)
                };
                let factor = rate / s.sr;
                let mut r = sound::stretch(&s, factor);
                r.sr = rate;
                Value::Sound(Rc::new(r))
            }
            "jcrev" | "stkrev" | "nrev" => {
                // (jcrev sound decay mix): Chowning-style reverb, 4 combs + 3 allpasses.
                let s = snd(&a(0))?;
                let decay = num_of(&a(1))?.max(0.01);
                let mix = num_of(&a(2))?.clamp(0.0, 1.0) as f32;
                Value::Sound(Rc::new(sound::jcrev(&s, decay, mix)))
            }
            "feedback-delay" | "delay" | "comb" | "alpass" => {
                let s = snd(&a(0))?;
                let (delay_s, fb) = if name == "comb" || name == "alpass" {
                    // (comb snd hz decay): delay = 1/hz, feedback from decay
                    let hz = num_of(&a(1))?.max(1.0);
                    let decay = num_of(&a(2))?.max(1e-3);
                    let d = 1.0 / hz;
                    (d, 10f64.powf(-3.0 * d / decay) as f32)
                } else {
                    (num_of(&a(1))?, num_of(&a(2))? as f32)
                };
                let d = ((delay_s * s.sr).round() as usize).max(1);
                let mut out = s.data.clone();
                for i in d..out.len() {
                    let y = out[i - d] * fb;
                    out[i] += y;
                }
                Value::Sound(Rc::new(Sound {
                    sr: s.sr,
                    start: s.start,
                    data: out,
                }))
            }
            "gate" => {
                // (gate sound lookahead risetime falltime floor threshold): crude per-sample envelope gate
                let s = snd(&a(0))?;
                let floor = num_of(&a(4))? as f32;
                let thr = num_of(&a(5))? as f32;
                let rise = (num_of(&a(2))? * s.sr).max(1.0) as f32;
                let fall = (num_of(&a(3))? * s.sr).max(1.0) as f32;
                let mut g = 1.0f32;
                let data = s
                    .data
                    .iter()
                    .map(|x| {
                        let target = if x.abs() > thr { 1.0 } else { floor };
                        g += (target - g) / if target > g { rise } else { fall };
                        g
                    })
                    .collect();
                Value::Sound(Rc::new(Sound {
                    sr: s.sr,
                    start: s.start,
                    data,
                }))
            }
            "snd-tapv" | "snd-xform" => {
                return rt(format!(
                    "{} is not supported by this Nyquist implementation",
                    name.to_ascii_uppercase()
                ))
            }
            _ => {
                return rt(format!(
                    "unsupported sound function: {}",
                    name.to_ascii_uppercase()
                ))
            }
        };
        Ok(v)
    }
}

fn is_sound_op(name: &str) -> bool {
    matches!(
        name,
        "mult"
            | "prod"
            | "sum"
            | "diff"
            | "scale"
            | "scale-db"
            | "hp"
            | "lp"
            | "tone"
            | "atone"
            | "lowpass2"
            | "highpass2"
            | "bandpass2"
            | "notch2"
            | "reson"
            | "areson"
            | "lowpass4"
            | "lowpass6"
            | "lowpass8"
            | "highpass4"
            | "highpass6"
            | "highpass8"
            | "eq-lowshelf"
            | "eq-highshelf"
            | "eq-band"
            | "extract"
            | "extract-abs"
            | "at"
            | "at-abs"
            | "stretch"
            | "stretch-abs"
            | "peak"
            | "snd-maxsamp"
            | "snd-length"
            | "snd-srate"
            | "snd-t0"
            | "snd-samples"
            | "snd-fetch"
            | "snd-sref"
            | "sref"
            | "s-min"
            | "s-max"
            | "s-abs"
            | "s-sqrt"
            | "s-exp"
            | "s-log"
            | "clip"
            | "shape"
            | "normalize"
            | "quantize"
            | "snd-quantize"
            | "snd-avg"
            | "rms"
            | "integrate"
            | "slope"
            | "force-srate"
            | "resample"
            | "feedback-delay"
            | "delay"
            | "comb"
            | "alpass"
            | "gate"
            | "snd-offset"
            | "snd-scale"
            | "snd-add"
            | "snd-prod"
            | "snd-tapv"
            | "snd-xform"
            | "hzosc"
            | "pwl"
            | "pwlv"
            | "pwl-list"
            | "pwlv-list"
            | "env"
            | "osc"
            | "osc-tri"
            | "osc-saw"
            | "osc-pulse"
            | "lfo"
            | "noise"
            | "s-rest"
            | "seq"
            | "sim"
            | "snd-from-array"
            | "snd-fetch-array"
            | "snd-abs"
            | "jcrev"
            | "stkrev"
            | "nrev"
            | "sine"
            | "snd-oneshot"
            | "loud"
            | "buzz"
            | "snd-buzz"
            | "snd-const"
            | "exp-dec"
            | "pwev"
            | "pwev-list"
            | "pwevr"
            | "build-harmonic"
            | "maketable"
            | "snd-pwl"
            | "snd-pluck"
            | "pluck"
            | "s-rest-abs"
            | "db-to-linear"
            | "snd-biquad"
            | "biquad"
            | "biquad-m"
            | "snd-follow"
            | "follow"
            | "set-logical-stop"
            | "set-logical-stop-abs"
            | "snd-set-logical-stop"
            | "snd-exp"
            | "snd-log"
            | "fmosc"
            | "linear-to-db"
    )
}

fn cutoff(v: &Value) -> NyResult<f32> {
    match v {
        Value::Sound(s) => Ok(s.data.first().copied().unwrap_or(1000.0)),
        o => Ok(num_of(o)? as f32),
    }
}

fn flat_nums(args: &[Value], list_form: bool) -> NyResult<Vec<f64>> {
    if list_form {
        list_of(args.first().unwrap_or(&Value::Nil))?
            .iter()
            .map(num_of)
            .collect()
    } else {
        args.iter().map(num_of).collect()
    }
}

fn do_spec(s: &Value) -> NyResult<(String, Value, Option<Value>)> {
    match s {
        Value::Sym(v) => Ok((v.clone(), Value::Nil, None)),
        Value::List(l) => {
            let Some(Value::Sym(v)) = l.first() else {
                return rt("do spec");
            };
            Ok((
                v.clone(),
                l.get(1).cloned().unwrap_or(Value::Nil),
                l.get(2).cloned(),
            ))
        }
        _ => rt("do spec"),
    }
}

fn bool_v(b: bool) -> Value {
    if b {
        Value::T
    } else {
        Value::Nil
    }
}

pub fn num_of(v: &Value) -> NyResult<f64> {
    v.as_f64()
        .ok_or_else(|| NyError::Runtime(format!("expected a number, got {}", v.display())))
}

pub fn int_of(v: &Value) -> NyResult<i64> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Float(f) => Ok(*f as i64),
        o => rt(format!("expected an integer, got {}", o.display())),
    }
}

fn list_of(v: &Value) -> NyResult<&[Value]> {
    v.as_list()
        .ok_or_else(|| NyError::Runtime(format!("expected a list, got {}", v.type_name())))
}

fn str_of(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        o => o.display(),
    }
}

/// XLISP `format` directives: ~a ~s ~% ~~ ~d ~f (with optional width/precision like ~5,2f).
fn format_str(fmt: &str, args: &[Value]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '~' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        // parameters
        let mut params = String::new();
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ',' || chars[i] == '@') {
            params.push(chars[i]);
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let d = chars[i].to_ascii_lowercase();
        i += 1;
        match d {
            'a' | 's' | 'd' | 'f' => {
                let v = it.next().cloned().unwrap_or(Value::Nil);
                let s = match (d, &v) {
                    ('s', Value::Str(s)) => format!("\"{s}\""),
                    ('f', v) => {
                        let prec = params
                            .split(',')
                            .nth(1)
                            .and_then(|p| p.parse::<usize>().ok());
                        match (v.as_f64(), prec) {
                            (Some(f), Some(p)) => format!("{f:.p$}"),
                            (Some(f), None) => Value::Float(f).display(),
                            _ => v.display(),
                        }
                    }
                    _ => v.display(),
                };
                let width = params
                    .split(',')
                    .next()
                    .and_then(|p| p.parse::<usize>().ok())
                    .unwrap_or(0);
                if params.contains('@') {
                    out.push_str(&format!("{s:>width$}"));
                } else {
                    out.push_str(&format!("{s:<width$}"));
                }
            }
            '%' | 'n' => out.push('\n'),
            '~' => out.push('~'),
            '&' => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
            }
            other => {
                out.push('~');
                out.push(other);
            }
        }
    }
    out
}
