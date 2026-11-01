//! The Audacity plug-in header. Both spellings are accepted: the old `;nyquist plug-in` /
//! `;control var "Label" int "unit" init min max` lines and the 3.x `$nyquist plug-in` /
//! `$control VAR (_ "Label") choice (("Key" (_ "Label")) …) 0` lines, where `(_ "…")` marks
//! a translatable string and a `$control` may span several lines. SAL code (`;codetype sal`)
//! is reported, not run.

use serde::{Deserialize, Serialize};

use crate::reader::Reader;
use crate::value::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Int,
    Float,
    IntText,
    FloatText,
    Choice,
    String,
    Text,
    Time,
    File,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub var: String,
    pub label: String,
    pub kind: ControlKind,
    /// Unit text as written (for the old choice syntax: the comma-separated options).
    pub text: String,
    pub initial: f64,
    /// `None` when the header says `nil` (free entry).
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Choice options when `kind == Choice`.
    pub choices: Vec<String>,
    /// Default for string / text / file controls.
    pub default_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginHeader {
    pub name: String,
    /// process | generate | analyze | tool
    pub kind: String,
    pub version: u32,
    pub author: String,
    pub info: String,
    pub codetype: String,
    /// `$preview linear|enabled|disabled|selection` (empty when absent).
    pub preview: String,
    pub controls: Vec<Control>,
    /// The code with header lines blanked (line numbers preserved).
    pub code: String,
}

const HEADER_KEYS: &[&str] = &[
    "nyquist",
    "version",
    "type",
    "name",
    "author",
    "info",
    "codetype",
    "control",
    "preview",
    "debugbutton",
    "release",
    "copyright",
    "helpfile",
    "manpage",
    "debugflags",
    "categories",
    "i",
    "mergeclips",
    "restoresplits",
    "action",
];

/// Text of a header form: `"x"` → x, `(_ "x")` → x, `(_ "x" "context")` → x,
/// `("Key" (_ "Label"))` → Label, a symbol → its name.
fn text_of(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Sym(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::List(l) => match (l.first(), l.get(1)) {
            (Some(Value::Sym(u)), Some(x)) if u == "_" => text_of(x),
            (Some(Value::Str(_)), Some(x)) => text_of(x),
            (Some(x), _) => text_of(x),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn num_of(v: Option<&Value>) -> Option<f64> {
    v.and_then(|v| match v {
        Value::Str(s) => s.trim().parse().ok(),
        o => o.as_f64(),
    })
}

fn parse_control(text: &str) -> Option<Control> {
    let f = Reader::new(text).read_all().ok()?;
    if f.len() < 3 {
        return None;
    }
    let var = text_of(&f[0]).to_ascii_lowercase();
    let label = text_of(&f[1]);
    let kind = match text_of(&f[2]).to_ascii_lowercase().as_str() {
        "int" => ControlKind::Int,
        "float" | "real" => ControlKind::Float,
        "int-text" => ControlKind::IntText,
        "float-text" => ControlKind::FloatText,
        "choice" => ControlKind::Choice,
        "string" => ControlKind::String,
        "text" => ControlKind::Text,
        "time" => ControlKind::Time,
        "file" => ControlKind::File,
        _ => ControlKind::Float,
    };
    let fourth = f.get(3);
    let (text, choices): (String, Vec<String>) = match (&kind, fourth) {
        (ControlKind::Choice, Some(Value::Str(s))) => (
            s.clone(),
            s.split(',').map(|x| x.trim().to_string()).collect(),
        ),
        (ControlKind::Choice, Some(Value::List(items))) => {
            (String::new(), items.iter().map(text_of).collect())
        }
        (_, Some(v)) => (text_of(v), Vec::new()),
        (_, None) => (String::new(), Vec::new()),
    };
    let default_text = match kind {
        ControlKind::String | ControlKind::Text | ControlKind::File => {
            f.get(4).map(text_of).unwrap_or_default()
        }
        _ => String::new(),
    };
    let max_default = if choices.is_empty() {
        Some(1.0)
    } else {
        Some((choices.len() as f64 - 1.0).max(0.0))
    };
    Some(Control {
        var,
        label,
        text,
        initial: num_of(f.get(4)).unwrap_or(0.0),
        min: if matches!(f.get(5), Some(Value::Nil)) {
            None
        } else {
            num_of(f.get(5)).or(Some(0.0))
        },
        max: if matches!(f.get(6), Some(Value::Nil)) {
            None
        } else {
            num_of(f.get(6)).or(max_default)
        },
        choices,
        default_text,
        kind,
    })
}

fn paren_depth(s: &str) -> i32 {
    let mut d = 0;
    let mut in_q = false;
    let mut in_comment = false;
    let mut prev = ' ';
    for c in s.chars() {
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            prev = c;
            continue;
        }
        match c {
            '"' if prev != '\\' => in_q = !in_q,
            ';' if !in_q => in_comment = true,
            '(' if !in_q => d += 1,
            ')' if !in_q => d -= 1,
            _ => {}
        }
        prev = c;
    }
    d
}

pub fn parse_header(src: &str) -> PluginHeader {
    let mut h = PluginHeader {
        name: String::new(),
        kind: "process".into(),
        version: 1,
        author: String::new(),
        info: String::new(),
        codetype: "lisp".into(),
        preview: String::new(),
        controls: Vec::new(),
        code: String::new(),
    };
    let lines: Vec<&str> = src.lines().collect();
    let mut code = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let l = line.trim_start();
        let marker = l.chars().next();
        let is_header = matches!(marker, Some(';') | Some('$'));
        if !is_header {
            code.push_str(line);
            code.push('\n');
            i += 1;
            continue;
        }
        let rest = l[1..].trim_start();
        let (key, val) = match rest.find(char::is_whitespace) {
            Some(k) => (&rest[..k], rest[k..].trim()),
            None => (rest, ""),
        };
        let key = key.to_ascii_lowercase();
        let known = HEADER_KEYS.contains(&key.as_str());
        // A `;` line stays in the code as a comment; a `$` line never does.
        let keep = marker == Some(';');
        if !known {
            if keep {
                code.push_str(line);
            }
            code.push('\n');
            i += 1;
            continue;
        }
        let mut text = val.to_string();
        let mut consumed = 1;
        if key == "control" || key == "name" || key == "author" || key == "info" {
            while paren_depth(&text) > 0 && i + consumed < lines.len() {
                text.push('\n');
                text.push_str(lines[i + consumed].trim());
                consumed += 1;
            }
        }
        let form_text = |t: &str| -> String {
            match Reader::new(t).read().ok() {
                Some(v) => text_of(&v),
                None => t.trim_matches('"').to_string(),
            }
        };
        match key.as_str() {
            "name" => h.name = form_text(&text).replace("...", "…"),
            "type" => {
                h.kind = text
                    .split_whitespace()
                    .next()
                    .unwrap_or("process")
                    .to_ascii_lowercase()
            }
            "version" => h.version = text.trim().parse().unwrap_or(1),
            "author" => h.author = form_text(&text),
            "info" => h.info = form_text(&text),
            "codetype" => h.codetype = text.trim().to_ascii_lowercase(),
            "preview" => h.preview = text.trim().to_ascii_lowercase(),
            "control" => {
                if let Some(c) = parse_control(&text) {
                    h.controls.push(c);
                }
            }
            _ => {}
        }
        for k in 0..consumed {
            if keep && k == 0 {
                code.push_str(line);
            }
            code.push('\n');
        }
        i += consumed;
    }
    h.code = code;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_old_style_header() {
        let src = r#";nyquist plug-in
;version 4
;type process
;name "Tremolo..."
;author "Steve Daulton"
;control wave "Waveform type" choice "Sine,Triangle,Sawtooth" 0
;control phase "Starting phase (degrees)" int "" 0 -180 180
;control wet "Wet level (percent)" int "" 40 1 100
;control lfo "Frequency (Hz)" float "" 4 0.001 1000
;control txt "Label" string "" "hello"

(mult *track* 0.5)
"#;
        let h = parse_header(src);
        assert_eq!(h.name, "Tremolo…");
        assert_eq!(h.kind, "process");
        assert_eq!(h.version, 4);
        assert_eq!(h.controls.len(), 5);
        assert_eq!(h.controls[0].kind, ControlKind::Choice);
        assert_eq!(h.controls[0].choices, vec!["Sine", "Triangle", "Sawtooth"]);
        assert_eq!(h.controls[1].min, Some(-180.0));
        assert_eq!(h.controls[3].initial, 4.0);
        assert_eq!(h.controls[4].default_text, "hello");
        assert!(h.code.contains("(mult *track* 0.5)"));
        assert_eq!(h.code.lines().count(), 12);
    }

    #[test]
    fn parses_the_3x_dollar_header_with_multiline_choices() {
        let src = r#"$nyquist plug-in
$version 4
$type process
$preview linear
$name (_ "Tremolo")
$debugbutton disabled
$author (_ "Steve Daulton")
$release 2.4.0
$copyright (_ "GNU General Public License v2.0 or later")

;; comment kept in code

$control WAVE (_ "Waveform type") choice ((_ "Sine")
                                          (_ "Triangle" "waveform")
                                          ("InverseSawtooth" (_ "Inverse Sawtooth"))) 1
$control LFO (_ "Frequency (Hz)") float-text "" 4 0.001 nil
$control TEXT (_ "Label text") string "" (_ "Sound ##1")
; set tremolo waveform
(mult *track* 0.5)
"#;
        let h = parse_header(src);
        assert_eq!(h.name, "Tremolo");
        assert_eq!(h.author, "Steve Daulton");
        assert_eq!(h.preview, "linear");
        assert_eq!(h.controls.len(), 3);
        let w = &h.controls[0];
        assert_eq!(w.var, "wave");
        assert_eq!(w.label, "Waveform type");
        assert_eq!(w.choices, vec!["Sine", "Triangle", "Inverse Sawtooth"]);
        assert_eq!(w.initial, 1.0);
        assert_eq!(h.controls[1].kind, ControlKind::FloatText);
        assert_eq!(h.controls[1].max, None);
        assert_eq!(h.controls[2].default_text, "Sound ##1");
        assert!(h.code.contains("(mult *track* 0.5)"));
        assert!(h.code.contains(";; comment kept in code"));
        assert!(!h.code.contains("$control"));
        assert_eq!(h.code.lines().count(), src.lines().count());
    }
}
