//! An open `.tmx` with its edit history and save state. No UI; every edit goes through `apply`,
//! which says why when it refuses.
use std::path::{Path, PathBuf};
use worlded_core::edit::{Edit, History};
use worlded_core::save::Saver;
use worlded_core::tmxdoc::{self, TmxDoc};

pub struct Document {
    pub path: PathBuf,
    pub doc: TmxDoc,
    pub history: History,
    saver: Saver,
    /// Why this file cannot be edited or saved (shown as a banner); `None` when it can.
    pub read_only: Option<String>,
}

impl Document {
    pub fn open(path: &Path) -> Result<Document, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Document::from_text(path, text)
    }

    pub fn from_text(path: &Path, text: String) -> Result<Document, String> {
        let doc = tmxdoc::open(text).map_err(|e| e.to_string())?;
        let read_only = doc.read_only.clone();
        Ok(Document { path: path.to_path_buf(), doc, history: History::default(), saver: Saver::new(), read_only })
    }

    pub fn is_dirty(&self) -> bool {
        !self.history.is_clean()
    }

    /// Whether an edit to `layer` may happen now, and if not, why.
    pub fn check(&self, layer: usize, visible: bool, locked: bool) -> Result<(), String> {
        if let Some(why) = &self.read_only {
            return Err(format!("read-only: {why}"));
        }
        let Some(l) = self.doc.tmx.layers.get(layer) else { return Err("no such layer".into()) };
        if locked {
            return Err(format!("layer {} is locked", l.name));
        }
        if !visible {
            return Err(format!("layer {} is hidden", l.name));
        }
        Ok(())
    }

    pub fn apply(&mut self, layer: usize, visible: bool, locked: bool, edit: Edit) -> Result<(), String> {
        self.check(layer, visible, locked)?;
        self.history.apply(&mut self.doc, edit);
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone();
        self.write_to(&path)
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        self.write_to(path)?;
        self.path = path.to_path_buf();
        Ok(())
    }

    /// On failure the document stays dirty and the file on disk is untouched.
    fn write_to(&mut self, path: &Path) -> Result<(), String> {
        let text = self.doc.to_text().map_err(|e| e.to_string())?;
        self.saver.save(path, &text).map_err(|e| e.to_string())?;
        self.history.mark_saved();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worlded_core::edit::Change;

    const TEXT: &str = "<map width=\"2\" height=\"2\">\n <layer name=\"L\" level=\"0\" width=\"2\" height=\"2\">\n  <data encoding=\"csv\">0,0,0,0</data>\n </layer>\n</map>\n";

    fn doc() -> Document {
        Document::from_text(Path::new("x.tmx"), TEXT.to_string()).unwrap()
    }

    fn put(x: u32, new: u32) -> Edit {
        Edit(vec![Change { layer: 0, x, y: 0, old: 0, new }])
    }

    #[test]
    fn locked_or_hidden_layer_refuses_edit_with_a_reason() {
        let mut d = doc();
        assert!(d.apply(0, true, true, put(0, 1)).unwrap_err().contains("locked"));
        assert!(d.apply(0, false, false, put(0, 1)).unwrap_err().contains("hidden"));
        assert!(d.apply(3, true, false, put(0, 1)).is_err());
        assert!(!d.is_dirty());
        assert!(d.apply(0, true, false, put(0, 1)).is_ok());
        assert!(d.is_dirty());
    }

    #[test]
    fn a_drag_is_one_history_step() {
        let mut d = doc();
        d.history.begin_group();
        for x in 0..2 {
            d.apply(0, true, false, put(x, 5)).unwrap();
        }
        d.apply(0, true, false, put(1, 6)).unwrap();
        d.history.end_group();
        assert!(d.history.undo(&mut d.doc));
        assert!(!d.history.undo(&mut d.doc), "one step only");
        assert_eq!(d.doc.tmx.layers[0].gids, vec![0; 4]);
    }

    #[test]
    fn dirty_follows_history_and_save() {
        let dir = std::env::temp_dir().join(format!("worlded_doc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.tmx");
        std::fs::write(&f, TEXT).unwrap();
        let mut d = Document::open(&f).unwrap();
        d.apply(0, true, false, put(0, 7)).unwrap();
        assert!(d.is_dirty());
        d.save().unwrap();
        assert!(!d.is_dirty());
        assert!(std::fs::read_to_string(&f).unwrap().contains("7,0,0,0"));
        assert!(dir.join("a.tmx.bak").exists());
        d.history.undo(&mut d.doc);
        assert!(d.is_dirty());
        d.save().unwrap();
        assert!(std::fs::read_to_string(&f).unwrap().contains("0,0,0,0"));
        assert!(!d.is_dirty());
        // A save that fails keeps the document dirty and the file as it was.
        d.apply(0, true, false, put(1, 9)).unwrap();
        let bad = dir.join("missing_dir").join("b.tmx");
        assert!(d.save_as(&bad).is_err());
        assert!(d.is_dirty() && d.path == f);
    }

    #[test]
    fn read_only_document_refuses_every_edit() {
        let text = TEXT.replace("csv", "weird");
        let mut d = Document::from_text(Path::new("x.tmx"), text).unwrap();
        assert!(d.read_only.is_some());
        assert!(d.apply(0, true, false, put(0, 1)).unwrap_err().contains("read-only"));
        assert!(d.save().is_err());
        assert!(!d.is_dirty());
    }
}
