//! Remembered settings: a `key=value` text file in the user config folder.
use std::path::PathBuf;

#[derive(Default, Debug, PartialEq, Clone)]
pub struct Settings {
    pub pz_install: Option<PathBuf>,
    pub last_project: Option<PathBuf>,
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    base.join("worlded-rusted").join("settings.txt")
}

impl Settings {
    pub fn load_from(path: &std::path::Path) -> Settings {
        let mut s = Settings::default();
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            match line.split_once('=') {
                Some(("pz_install", v)) if !v.is_empty() => s.pz_install = Some(v.into()),
                Some(("last_project", v)) if !v.is_empty() => s.last_project = Some(v.into()),
                _ => {}
            }
        }
        s
    }

    pub fn save_to(&self, path: &std::path::Path) {
        let line = |k: &str, v: &Option<PathBuf>| v.as_ref().map(|p| format!("{k}={}\n", p.display())).unwrap_or_default();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, line("pz_install", &self.pz_install) + &line("last_project", &self.last_project));
    }

    /// `<install>/media/texturepacks`, when an install is set.
    pub fn pack_dir(&self) -> Option<PathBuf> {
        self.pz_install.as_ref().map(|p| p.join("media").join("texturepacks"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_missing_file_gives_default() {
        let p = std::env::temp_dir().join("wa_settings_test").join("s.txt");
        let _ = std::fs::remove_file(&p);
        assert_eq!(Settings::load_from(&p), Settings::default());
        let s = Settings { pz_install: Some("C:/pz".into()), last_project: Some("D:/m/a.pzw".into()) };
        s.save_to(&p);
        assert_eq!(Settings::load_from(&p), s);
        assert_eq!(s.pack_dir().unwrap(), PathBuf::from("C:/pz").join("media").join("texturepacks"));
    }
}
