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
use worlded_compile::bmpimport;
use worlded_compile::pipeline::{compile_map, find_rules, Options};

const USAGE: &str = "worlded: compile a map into lot files, no WorldEd or KnoxMap needed.

    worlded <map folder> [<out folder>] [--seed N] [--worlded-seams] [--keep-source DIR] [--threads N]
    worlded <project>.pzw [<out folder>] [--keep-source DIR] [--threads N]
    worlded import-bmp <project>.pzw <image.bmp> [<image_veg.bmp>]

A map folder holds <name>.bmp, <name>_veg.bmp, <name>.pzw and buildings/*.tbx.
A .pzw project compiles from the .tmx maps its cells list (ground from the pictures embedded in them, merged with their tile layers), plus buildings/*.tbx beside it.
`import-bmp` writes the picture into every cell's .tmx as `<bmp-image>` elements (the `_veg` file is found by name when not given), like WorldEd's BMPtoTMX.
Writes the .lotheader / .lotpack / chunkdata files into <out folder> (default: <map folder>/lots, or lots beside the .pzw).
Double-click to be asked for the folder, or drag a map folder onto worlded.exe.";

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

fn run(args: Vec<String>) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "import-bmp") {
        let rest = &args[1..];
        if rest.len() < 2 || rest.len() > 3 {
            return Err(format!("worlded import-bmp <project>.pzw <image.bmp> [<image_veg.bmp>]\n\n{USAGE}"));
        }
        let n = bmpimport::import_bmp(Path::new(&rest[0]), Path::new(&rest[1]), rest.get(2).map(|s| Path::new(s))).map_err(|e| e.to_string())?;
        println!("{n} cell maps got the pictures of {}", rest[1]);
        return Ok(());
    }
    let mut positional: Vec<String> = Vec::new();
    let (mut seed, mut seams, mut keep, mut threads) = (1i64, false, None::<PathBuf>, None::<usize>);
    let mut given: Vec<&'static str> = Vec::new(); // options only the bitmap mode takes
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--worlded-seams" => {
                seams = true;
                given.push("--worlded-seams");
            }
            "--seed" => {
                seed = it.next().and_then(|v| v.parse().ok()).ok_or("--seed needs a number")?;
                given.push("--seed");
            }
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
    let pzw_mode = project.extension().is_some_and(|e| e.eq_ignore_ascii_case("pzw")) && !project.is_dir();
    if pzw_mode {
        if let Some(opt) = given.first() {
            return Err(format!("{opt} is for a map folder; a .pzw project takes its seed and blending from the maps\n\n{USAGE}"));
        }
    }
    let default_out = if pzw_mode { project.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).join("lots") } else { project.join("lots") };
    let out = positional.pop().map(PathBuf::from).unwrap_or(default_out);
    let opts = Options { seed, seams, workers: threads, keep_source: keep, rules: find_rules()? };
    let started = std::time::Instant::now();
    let n = compile_map(&project, &out, &opts, &|done, rows| println!("row {done} of {rows}")).map_err(|e| e.to_string())?;
    println!("{n} cells compiled into {} in {:.1} s", out.display(), started.elapsed().as_secs_f32());
    Ok(())
}

fn main() -> ExitCode {
    let pause = alone_in_console();
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().to_string()).collect();
    let result = std::panic::catch_unwind(|| run(args)).unwrap_or_else(|_| Err("internal error (see the message above); please report it with the map that caused it".to_string()));
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
