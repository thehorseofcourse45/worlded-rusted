//! Helpers for tests that compare against the Python oracle (tools/dump_for_rust.py).
use std::path::PathBuf;
use std::process::Command;

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs `python tools/dump_for_rust.py <args>` from the repo root; `None` when Python or its
/// modules are missing (the caller then skips). Panics if the script itself fails.
pub fn python(args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("python").arg("tools/dump_for_rust.py").args(args).current_dir(repo()).output().ok()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("ModuleNotFoundError") || err.contains("No module named") {
            return None;
        }
        panic!("dump_for_rust.py {args:?} failed: {err}");
    }
    Some(out.stdout)
}
