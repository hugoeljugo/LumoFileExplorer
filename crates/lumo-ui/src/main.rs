// Release builds are GUI-subsystem on Windows: no console window flashes when
// Explorer, Win+E or a shortcut launches Lumo. Debug builds keep the console
// so panics and logs are visible during development.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;

use lumo_ui::Start;

fn main() -> ExitCode {
    lumo_ui::install_crash_handler(lumo_ui::data_dir());
    // `lumo [PATH]`: open one tab at PATH; with no argument, restore the last session.
    let start = match std::env::args_os().nth(1) {
        Some(arg) => match std::path::absolute(&arg) {
            Ok(path) => Start::Path(path),
            Err(_) => Start::Restore,
        },
        None => Start::Restore,
    };
    match lumo_ui::run(start) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            lumo_platform::show_fatal_error("Lumo couldn't start", &err.to_string());
            ExitCode::FAILURE
        }
    }
}
