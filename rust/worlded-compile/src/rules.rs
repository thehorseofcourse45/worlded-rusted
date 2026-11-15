//! Rules.txt: colour-to-tile rules and tile aliases (port of knoxbuild/rules.py).
use crate::CompileError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub label: String,
    pub bitmap: i32,
    pub color: (i32, i32, i32),
    pub entries: Vec<String>,
    pub layer: String,
    pub condition: Option<(i32, i32, i32)>,
}

#[derive(Debug, Default)]
pub struct Ruleset {
    pub aliases: HashMap<String, Vec<String>>,
    pub rules: Vec<Rule>,
}

impl Ruleset {
    pub fn for_bitmap(&self, bitmap: i32) -> Vec<&Rule> {
        self.rules.iter().filter(|r| r.bitmap == bitmap).collect()
    }
}

pub(crate) enum Value {
    Str(String),
    List(Vec<String>),
}

/// Lines with `//` comments and surrounding space removed.
pub(crate) fn clean_lines(text: &str) -> Vec<String> {
    text.lines().map(|l| l.split("//").next().unwrap_or("").trim().to_string()).collect()
}

/// `key = value` where key is word characters: (key, value trimmed).
pub(crate) fn key_value(line: &str) -> Option<(&str, &str)> {
    let end = line.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(line.len());
    if end == 0 {
        return None;
    }
    let rest = line[end..].trim_start().strip_prefix('=')?;
    Some((&line[..end], rest.trim()))
}

/// Items of a `[` ... `]` list that starts on line `i`; returns the items and the index of the closing line.
pub(crate) fn read_list(lines: &[String], mut i: usize) -> (Vec<String>, usize) {
    let mut items = Vec::new();
    i += 1;
    while i < lines.len() && lines[i] != "]" {
        if !lines[i].is_empty() {
            items.push(lines[i].clone());
        }
        i += 1;
    }
    (items, i)
}

fn blocks(text: &str) -> Vec<(String, HashMap<String, Value>)> {
    let lines = clean_lines(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let kind = &lines[i];
        if (kind == "alias" || kind == "rule") && i + 1 < lines.len() && lines[i + 1] == "{" {
            let kind = kind.clone();
            i += 2;
            let mut body = HashMap::new();
            while i < lines.len() && lines[i] != "}" {
                if let Some((k, v)) = key_value(&lines[i]) {
                    if v == "[" {
                        let (items, end) = read_list(&lines, i);
                        body.insert(k.to_string(), Value::List(items));
                        i = end;
                    } else {
                        body.insert(k.to_string(), Value::Str(v.to_string()));
                    }
                }
                i += 1;
            }
            out.push((kind, body));
        }
        i += 1;
    }
    out
}

fn triple(path: &Path, v: &str) -> Result<(i32, i32, i32), CompileError> {
    let t: Vec<i32> = v
        .split_whitespace()
        .map(|x| x.parse().map_err(|_| CompileError::Parse(path.to_path_buf(), format!("bad number in {v:?}"))))
        .collect::<Result<_, _>>()?;
    match t[..] {
        [r, g, b] => Ok((r, g, b)),
        _ => Err(CompileError::Parse(path.to_path_buf(), format!("expected three numbers, got {v:?}"))),
    }
}

fn need<'a>(path: &Path, b: &'a HashMap<String, Value>, key: &str, kind: &str) -> Result<&'a str, CompileError> {
    match b.get(key) {
        Some(Value::Str(s)) => Ok(s),
        _ => Err(CompileError::Parse(path.to_path_buf(), format!("{kind} block without a single-line {key}"))),
    }
}

pub fn parse_rules(text: &str) -> Result<Ruleset, CompileError> {
    parse_rules_from(Path::new("Rules.txt"), text)
}

fn parse_rules_from(path: &Path, text: &str) -> Result<Ruleset, CompileError> {
    let mut rs = Ruleset::default();
    for (kind, b) in blocks(text) {
        let entries = match b.get("tiles") {
            Some(Value::List(l)) => l.clone(),
            Some(Value::Str(s)) => vec![s.clone()],
            None => Vec::new(),
        };
        if kind == "alias" {
            rs.aliases.insert(need(path, &b, "name", "alias")?.to_string(), entries);
        } else {
            let bitmap = need(path, &b, "bitmap", "rule")?
                .trim()
                .parse()
                .map_err(|_| CompileError::Parse(path.to_path_buf(), "bad bitmap number".into()))?;
            rs.rules.push(Rule {
                label: match b.get("label") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                },
                bitmap,
                color: triple(path, need(path, &b, "color", "rule")?)?,
                entries,
                layer: need(path, &b, "layer", "rule")?.to_string(),
                condition: match b.get("condition") {
                    Some(Value::Str(s)) => Some(triple(path, s)?),
                    _ => None,
                },
            });
        }
    }
    Ok(rs)
}

pub fn load_rules(path: &Path) -> Result<Ruleset, CompileError> {
    let bytes = std::fs::read(path).map_err(|e| CompileError::Io(PathBuf::from(path), e))?;
    parse_rules_from(path, &String::from_utf8_lossy(&bytes))
}
