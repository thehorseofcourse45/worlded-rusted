//! Blends.txt: edge-overlay rules (port of knoxbuild/blends.py).
use crate::rules::{clean_lines, read_list, Value};
use crate::CompileError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Blend {
    pub layer: String,
    pub main: String,
    pub tile: String,
    pub dir: String,
    pub exclude: Vec<String>,
    pub exclude2: Vec<Vec<String>>,
}

fn err(path: &Path, m: &str) -> CompileError {
    CompileError::Parse(path.to_path_buf(), m.to_string())
}

pub fn parse_blends(text: &str) -> Result<Vec<Blend>, CompileError> {
    parse_blends_from(Path::new("Blends.txt"), text)
}

fn parse_blends_from(path: &Path, text: &str) -> Result<Vec<Blend>, CompileError> {
    let mut out = Vec::new();
    let mut cur: Option<HashMap<String, Value>> = None;
    let lines = clean_lines(text);
    let mut i = 0;
    while i < lines.len() {
        let s = &lines[i];
        if s == "blend" {
            cur = Some(HashMap::new());
        } else if s == "}" && cur.is_some() {
            let c = cur.take().unwrap();
            let one = |k: &str| match c.get(k) {
                Some(Value::Str(v)) => Ok(v.clone()),
                _ => Err(err(path, &format!("blend without a single-line {k}"))),
            };
            out.push(Blend {
                layer: one("layer")?,
                main: one("mainTile")?,
                tile: one("blendTile")?,
                dir: one("dir")?,
                exclude: match c.get("exclude") {
                    None => Vec::new(),
                    Some(Value::Str(v)) => v.split_whitespace().map(String::from).collect(),
                    Some(Value::List(_)) => return Err(err(path, "exclude must be on one line")),
                },
                exclude2: match c.get("exclude2") {
                    None => Vec::new(),
                    Some(Value::List(l)) => l.iter().map(|p| p.split_whitespace().map(String::from).collect()).collect(),
                    Some(Value::Str(_)) => return Err(err(path, "exclude2 must be a list")),
                },
            });
        } else if let (Some(c), Some(eq)) = (cur.as_mut(), s.find('=')) {
            let (k, v) = (s[..eq].trim(), s[eq + 1..].trim());
            if v == "[" {
                let (items, end) = read_list(&lines, i);
                c.insert(k.to_string(), Value::List(items));
                i = end;
            } else {
                c.insert(k.to_string(), Value::Str(v.to_string()));
            }
        }
        i += 1;
    }
    Ok(out)
}

pub fn load_blends(path: &Path) -> Result<Vec<Blend>, CompileError> {
    let bytes = std::fs::read(path).map_err(|e| CompileError::Io(PathBuf::from(path), e))?;
    parse_blends_from(path, &String::from_utf8_lossy(&bytes))
}
