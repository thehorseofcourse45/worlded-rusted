//! Map compiler: map folder to lot files. A port of knoxbuild/ and tools/map_source.py.
pub mod bmp;

use std::path::PathBuf;

#[derive(Debug)]
pub enum CompileError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            CompileError::Parse(p, s) => write!(f, "{}: {s}", p.display()),
        }
    }
}
impl std::error::Error for CompileError {}
