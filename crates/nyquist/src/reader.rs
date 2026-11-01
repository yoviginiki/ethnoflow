//! S-expression reader for the Nyquist / XLISP dialect: symbols (case-insensitive, kept
//! lower-case), integers, floats, strings with escapes, quote `'`, `#'` (function quote,
//! read as quote), `#t` / `#f`, comments `;` to end of line and `#| ... |#` blocks.

use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ReadError {
    pub line: usize,
    pub message: String,
}

pub struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Reader<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
        }
    }

    fn err(&self, m: impl Into<String>) -> ReadError {
        ReadError {
            line: self.line,
            message: m.into(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b';') => {
                    while let Some(c) = self.bump() {
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                Some(b'#') if self.src.get(self.pos + 1) == Some(&b'|') => {
                    self.pos += 2;
                    loop {
                        match self.bump() {
                            None => return,
                            Some(b'|') if self.peek() == Some(b'#') => {
                                self.pos += 1;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                _ => return,
            }
        }
    }

    /// Every top-level form of the source.
    pub fn read_all(&mut self) -> Result<Vec<Value>, ReadError> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.peek().is_none() {
                return Ok(out);
            }
            out.push(self.read()?);
        }
    }

    pub fn read(&mut self) -> Result<Value, ReadError> {
        self.skip_ws();
        let Some(c) = self.peek() else {
            return Err(self.err("unexpected end of input"));
        };
        match c {
            b'(' => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        None => return Err(self.err("missing )")),
                        Some(b')') => {
                            self.bump();
                            return Ok(Value::list(items));
                        }
                        Some(b'.') if self.is_delim_after(self.pos + 1) => {
                            // Dotted pair: (a . b) → we only support proper lists; (a . nil) ok.
                            self.bump();
                            let tail = self.read()?;
                            self.skip_ws();
                            if self.bump() != Some(b')') {
                                return Err(self.err("bad dotted pair"));
                            }
                            if let Some(rest) = tail.as_list() {
                                items.extend(rest.iter().cloned());
                                return Ok(Value::list(items));
                            }
                            return Err(self.err("dotted pairs are not supported"));
                        }
                        _ => items.push(self.read()?),
                    }
                }
            }
            b')' => Err(self.err("unexpected )")),
            b'\'' => {
                self.bump();
                let v = self.read()?;
                Ok(Value::list(vec![Value::sym("quote"), v]))
            }
            b'`' => {
                self.bump();
                let v = self.read()?;
                Ok(Value::list(vec![Value::sym("backquote"), v]))
            }
            b',' => {
                self.bump();
                let splice = self.peek() == Some(b'@');
                if splice {
                    self.bump();
                }
                let v = self.read()?;
                Ok(Value::list(vec![
                    Value::sym(if splice { "comma-at" } else { "comma" }),
                    v,
                ]))
            }
            b'#' => {
                self.bump();
                match self.peek() {
                    Some(b'\'') => {
                        self.bump();
                        let v = self.read()?;
                        Ok(Value::list(vec![Value::sym("function"), v]))
                    }
                    Some(b't') | Some(b'T') => {
                        self.bump();
                        Ok(Value::T)
                    }
                    Some(b'f') | Some(b'F') => {
                        self.bump();
                        Ok(Value::Nil)
                    }
                    Some(b'(') => {
                        // #(...) literal array
                        let v = self.read()?;
                        let items = v.as_list().map(|l| l.to_vec()).unwrap_or_default();
                        Ok(Value::Array(std::rc::Rc::new(std::cell::RefCell::new(
                            items,
                        ))))
                    }
                    Some(b'\\') => {
                        // #\c character literal (also #\Space, #\Newline, #\Tab)
                        self.bump();
                        let start = self.pos;
                        self.bump();
                        while let Some(c) = self.peek() {
                            if c.is_ascii_alphanumeric() {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                        let tok = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
                        let ch = match tok.to_ascii_lowercase().as_str() {
                            "space" => " ".to_string(),
                            "newline" | "linefeed" => "\n".to_string(),
                            "tab" => "\t".to_string(),
                            "return" => "\r".to_string(),
                            _ => tok
                                .chars()
                                .next()
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
                        };
                        Ok(Value::Str(ch))
                    }
                    _ => Err(self.err("unsupported # syntax")),
                }
            }
            b'"' => {
                self.bump();
                let mut s = String::new();
                loop {
                    match self.bump() {
                        None => return Err(self.err("unterminated string")),
                        Some(b'"') => return Ok(Value::Str(s)),
                        Some(b'\\') => match self.bump() {
                            Some(b'n') => s.push('\n'),
                            Some(b't') => s.push('\t'),
                            Some(c) => s.push(c as char),
                            None => return Err(self.err("unterminated string")),
                        },
                        Some(c) => s.push(c as char),
                    }
                }
            }
            _ => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_whitespace()
                        || c == b'('
                        || c == b')'
                        || c == b'"'
                        || c == b';'
                        || c == b','
                    {
                        break;
                    }
                    self.bump();
                }
                let tok = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
                if tok.is_empty() {
                    return Err(self.err("empty token"));
                }
                Ok(atom(tok))
            }
        }
    }

    fn is_delim_after(&self, i: usize) -> bool {
        matches!(
            self.src.get(i),
            None | Some(b' ') | Some(b'\n') | Some(b'\t') | Some(b')') | Some(b'(')
        )
    }
}

fn atom(tok: &str) -> Value {
    if let Ok(i) = tok.parse::<i64>() {
        return Value::Int(i);
    }
    let t = tok.replace(['d', 'D'], "e");
    if (tok.contains('.') || tok.contains(['e', 'E', 'd', 'D']))
        && t.parse::<f64>().is_ok()
        && tok
            .chars()
            .next()
            .map(|c| c.is_ascii_digit() || c == '-' || c == '+' || c == '.')
            .unwrap_or(false)
    {
        if let Ok(f) = t.parse::<f64>() {
            return Value::Float(f);
        }
    }
    match tok.to_ascii_lowercase().as_str() {
        "nil" => Value::Nil,
        "t" => Value::T,
        s => Value::sym(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_usual_plugin_shapes() {
        let v = Reader::new("(mult *track* 0.5) ; comment\n'(1 2.5 \"s\") #'foo #| block |# 1e3")
            .read_all()
            .unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].as_list().unwrap().len(), 3);
        assert_eq!(v[0].as_list().unwrap()[0], Value::sym("mult"));
        assert_eq!(v[3], Value::Float(1000.0));
        let q = v[1].as_list().unwrap();
        assert_eq!(q[0], Value::sym("quote"));
    }
}
