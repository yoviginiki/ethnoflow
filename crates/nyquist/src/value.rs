//! Values of the interpreter. Sounds are fixed arrays of samples at a rate with a start
//! time (so `at` / `sim` / `seq` can place them); Nyquist's lazy streams are not modelled.
//! Stereo in Nyquist is an *array* of sounds; `Value::Array` holds that.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub sr: f64,
    /// Start time in seconds (0 for a plain sound).
    pub start: f64,
    pub data: Vec<f32>,
}

impl Sound {
    pub fn new(sr: f64, data: Vec<f32>) -> Self {
        Self::at(sr, 0.0, data)
    }
    pub fn at(sr: f64, start: f64, data: Vec<f32>) -> Self {
        Self { sr, start, data }
    }
    pub fn duration(&self) -> f64 {
        self.data.len() as f64 / self.sr
    }
}

#[derive(Debug, Clone)]
pub struct Lambda {
    /// A `defmacro`: called with unevaluated arguments, the result is evaluated again.
    pub is_macro: bool,
    pub params: Vec<String>,
    /// `&optional` parameters with their defaults (evaluated at call time).
    pub optional: Vec<(String, Value)>,
    pub rest: Option<String>,
    pub body: Vec<Value>,
    pub closure: Env,
}

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    T,
    Int(i64),
    Float(f64),
    Str(String),
    Sym(String),
    List(Rc<Vec<Value>>),
    Array(Rc<RefCell<Vec<Value>>>),
    Sound(Rc<Sound>),
    Lambda(Rc<Lambda>),
    Builtin(&'static str),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) | (Value::T, Value::T) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => *a as f64 == *b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Sym(a), Value::Sym(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Sound(a), Value::Sound(b)) => Rc::ptr_eq(a, b),
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn sym(s: &str) -> Value {
        Value::Sym(s.to_ascii_lowercase())
    }
    pub fn list(items: Vec<Value>) -> Value {
        if items.is_empty() {
            Value::Nil
        } else {
            Value::List(Rc::new(items))
        }
    }
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            Value::Nil => Some(&[]),
            _ => None,
        }
    }
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Nil)
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::T => "t",
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Sym(_) => "symbol",
            Value::List(_) => "list",
            Value::Array(_) => "array",
            Value::Sound(_) => "sound",
            Value::Lambda(_) => "function",
            Value::Builtin(_) => "builtin",
        }
    }
    /// Printed form (for messages and `print`).
    pub fn display(&self) -> String {
        match self {
            Value::Nil => "NIL".into(),
            Value::T => "T".into(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                }
            }
            Value::Str(s) => s.clone(),
            Value::Sym(s) => s.to_ascii_uppercase(),
            Value::List(l) => format!(
                "({})",
                l.iter().map(|v| v.display()).collect::<Vec<_>>().join(" ")
            ),
            Value::Array(a) => format!(
                "#({})",
                a.borrow()
                    .iter()
                    .map(|v| v.display())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Value::Sound(s) => format!("#<Sound: {:.3} s @ {} Hz>", s.duration(), s.sr),
            Value::Lambda(_) => "#<Closure>".into(),
            Value::Builtin(n) => format!("#<Subr-{n}>"),
        }
    }
}

/// Lexical environment: a chain of frames.
#[derive(Debug, Clone)]
pub struct Env(pub Rc<RefCell<Frame>>);

#[derive(Debug)]
pub struct Frame {
    pub vars: std::collections::HashMap<String, Value>,
    pub parent: Option<Env>,
}

impl Env {
    pub fn root() -> Env {
        Env(Rc::new(RefCell::new(Frame {
            vars: std::collections::HashMap::new(),
            parent: None,
        })))
    }
    pub fn child(&self) -> Env {
        Env(Rc::new(RefCell::new(Frame {
            vars: std::collections::HashMap::new(),
            parent: Some(self.clone()),
        })))
    }
    pub fn get(&self, name: &str) -> Option<Value> {
        let f = self.0.borrow();
        if let Some(v) = f.vars.get(name) {
            return Some(v.clone());
        }
        f.parent.as_ref().and_then(|p| p.get(name))
    }
    pub fn define(&self, name: &str, v: Value) {
        self.0.borrow_mut().vars.insert(name.to_string(), v);
    }
    /// Assign to the frame that holds `name` (or the root when none does).
    pub fn set(&self, name: &str, v: Value) {
        {
            let mut f = self.0.borrow_mut();
            if f.vars.contains_key(name) {
                f.vars.insert(name.to_string(), v);
                return;
            }
        }
        let parent = self.0.borrow().parent.clone();
        match parent {
            Some(p) => p.set(name, v),
            None => self.define(name, v),
        }
    }
}
