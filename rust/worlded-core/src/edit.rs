//! Tile edits and their undo history. UI-free.
use crate::tmxdoc::TmxDoc;

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub layer: usize,
    pub x: u32,
    pub y: u32,
    /// Stored gid before the change. `History::apply` refills it from the document.
    pub old: u32,
    pub new: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edit(pub Vec<Change>);

pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Undo-stack length at the last save; `None` once that state can no longer be reached.
    saved: Option<usize>,
    group: Option<Edit>,
}

impl Default for History {
    fn default() -> History {
        History { undo: vec![], redo: vec![], saved: Some(0), group: None }
    }
}

impl History {
    pub fn new() -> History {
        History::default()
    }

    /// Apply an edit. Changes that do not change a square, or that the document refuses
    /// (outside the map, unsaveable layer), are dropped; an empty result is not a step.
    pub fn apply(&mut self, doc: &mut TmxDoc, edit: Edit) {
        let mut done = Edit::default();
        for c in edit.0 {
            let old = doc.raw_gid(c.layer, c.x, c.y);
            if old != c.new && doc.set_gid(c.layer, c.x, c.y, c.new) {
                done.0.push(Change { old, ..c });
            }
        }
        if done.0.is_empty() {
            return;
        }
        match &mut self.group {
            Some(g) => g.0.extend(done.0),
            None => self.push(done),
        }
    }

    fn push(&mut self, edit: Edit) {
        self.redo.clear();
        if self.saved.is_some_and(|s| s > self.undo.len()) {
            self.saved = None;
        }
        self.undo.push(edit);
    }

    /// Edits applied until `end_group` form one undo step (a drag).
    pub fn begin_group(&mut self) {
        self.group.get_or_insert_with(Edit::default);
    }

    pub fn end_group(&mut self) {
        if let Some(g) = self.group.take() {
            if !g.0.is_empty() {
                self.push(g);
            }
        }
    }

    pub fn undo(&mut self, doc: &mut TmxDoc) -> bool {
        self.end_group();
        let Some(edit) = self.undo.pop() else { return false };
        for c in edit.0.iter().rev() {
            doc.set_gid(c.layer, c.x, c.y, c.old);
        }
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self, doc: &mut TmxDoc) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        for c in &edit.0 {
            doc.set_gid(c.layer, c.x, c.y, c.new);
        }
        self.undo.push(edit);
        true
    }

    pub fn is_clean(&self) -> bool {
        self.group.as_ref().is_none_or(|g| g.0.is_empty()) && self.saved == Some(self.undo.len())
    }

    pub fn mark_saved(&mut self) {
        self.end_group();
        self.saved = Some(self.undo.len());
    }
}
