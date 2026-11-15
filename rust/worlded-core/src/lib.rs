//! Project model and file formats for the worlded viewer (no UI).
pub mod edit;
pub mod gids;
pub mod pack;
pub mod pzw;
pub mod save;
pub mod tmx;
pub mod tmxdoc;
pub mod tools;

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "io: {e}"),
            CoreError::Parse(s) => write!(f, "parse: {s}"),
        }
    }
}
impl std::error::Error for CoreError {}
impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e)
    }
}
