//! worlded: compile a map folder into lot files, no WorldEd, KnoxMap or Python needed.
//!
//!     worlded <map folder> [<out folder>] [--seed N] [--worlded-seams] [--keep-source DIR] [--threads N]
//!
//! The map folder holds <name>.bmp, <name>_veg.bmp, <name>.pzw and buildings/*.tbx. The lot files
//! go to <out folder> (default <map folder>/lots). Double-click to be asked for the folder, or drag
//! a map folder onto worlded.exe. Rules.txt and Blends.txt are read from a `config` folder next to
//! the program (or $KNOXLOTS_CONFIG).
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use worlded_compile::pipeline::{compile_map, Options};

const USAGE: &str = "worlded: compile a map folder into lot files, no WorldEd or KnoxMap needed.\n\n    worlded <map folder> [<out folder>] [--seed N] [--worlded-seams] [--keep-source DIR] [--threads N]\n\nThe map folder holds <name>.bmp, <name>_veg.bmp, <name>.pzw and buildings/*.tbx.\nWrites the .lotheader / .lotpack / chunkdata files into <out folder> (default <map folder>/lots).\nDouble-click to be asked for the folder, or drag a map folder onto worlded.exe.";

/// True when this program owns its console alone, i.e. it was started from Explorer and the window
/// will vanish when it ends.
#[cfg(windows)]
fn alone_in_console() -> bool {
    extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    }
    let mut ids = [0u32; 4];
    // SAFETY: the buffer is valid for the 4 entries we pass.
    unsafe { GetConsoleProcessList(ids.as_mut_ptr(), 4) == 1 }
}

#[cfg(not(windows))]
fn alone_in_console() -> bool {
    false
}

/// Rules.txt: $KNOXLOTS_CONFIG, `config` next to the program, `config` in the current folder, or
/// (a build inside the repo) the repo's `config`.
fn find_rules() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("KNOXLOTS_CONFIG") {
        dirs.push(d.into());
    }
    dirs.push(exe_dir.join("config"));
    dirs.push(PathBuf::from("config"));
    for up in exe_dir.ancestors().take(5) {
        dirs.push(up.join("config"));
    }
    for d in &dirs {
        if d.join("Rules.txt").exists() && d.join("Blends.txt").exists() {
            return Ok(d.join("Rules.txt"));
        }
    }
    Err("Rules.txt and Blends.txt not found: put them in a config folder next to the program (or set KNOXLOTS_CONFIG)".into())
}

fn run(args: Vec<String>) -> Result<(), String> {
    let mut positional: Vec<String> = Vec::new();
    let (mut seed, mut seams, mut keep, mut threads) = (1i64, false, None::<PathBuf>, None::<usize>);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--worlded-seams" => seams = true,
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).ok_or("--seed needs a number")?,
            "--threads" => threads = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--threads needs a number")?),
            "--keep-source" => keep = Some(it.next().ok_or("--keep-source needs a folder")?.into()),
            _ if a.starts_with("--") => return Err(format!("unknown option {a}\n\n{USAGE}")),
            _ => positional.push(a),
        }
    }
    if positional.is_empty() {
        println!("{USAGE}\n");
        print!("Map folder (or drag it here): ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
        positional.push(line.trim().trim_matches('"').to_string());
    }
    if positional.len() > 2 || positional[0].is_empty() {
        return Err(USAGE.to_string());
    }
    let project = PathBuf::from(positional.remove(0));
    let out = positional.pop().map(PathBuf::from).unwrap_or_else(|| project.join("lots"));
    let opts = Options { seed, seams, workers: threads, keep_source: keep, rules: find_rules()? };
    let started = std::time::Instant::now();
    let n = compile_map(&project, &out, &opts, &|done, rows| println!("row {done} of {rows}")).map_err(|e| e.to_string())?;
    println!("{n} cells compiled into {} in {:.1} s", out.display(), started.elapsed().as_secs_f32());
    Ok(())
}

fn main() -> ExitCode {
    let pause = alone_in_console();
    let result = run(std::env::args().skip(1).collect());
    let code = match &result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(2)
        }
    };
    if pause {
        print!("Press Enter to close ...");
        std::io::stdout().flush().ok();
        let _ = std::io::stdin().read_line(&mut String::new());
    }
    code
}
