//! Atomic save: write `<file>.tmp`, rename over the target. The first save of a file in a
//! session keeps the previous version as `<file>.bak`.
use crate::CoreError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Saver {
    backed_up: HashSet<PathBuf>,
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

impl Saver {
    pub fn new() -> Saver {
        Saver::default()
    }

    /// On any error the old file is untouched and no temp file is left behind.
    pub fn save(&mut self, path: &Path, text: &str) -> Result<(), CoreError> {
        if path.exists() && !self.backed_up.contains(path) {
            std::fs::copy(path, with_suffix(path, ".bak"))?;
            self.backed_up.insert(path.to_path_buf());
        }
        let tmp = with_suffix(path, ".tmp");
        let result = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        Ok(result?)
    }
}
