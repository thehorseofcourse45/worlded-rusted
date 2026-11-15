//! Map compiler: map folder to lot files. A port of knoxbuild/ and tools/map_source.py.
pub mod blend_layers;
pub mod blends;
pub mod bmp;
pub mod buildroofs;
pub mod buildtiles;
pub mod header;
pub mod rules;
pub mod tbx;
pub mod terrain;

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
