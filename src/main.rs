//! The `grimmoire` executable: sets up the terminal and runs the application.

use std::io;
use std::process::ExitCode;

use grimmoire::app::App;
use grimmoire::terminal::{self, Terminal};

fn main() -> ExitCode {
    terminal::install_panic_hook();

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // The terminal has already been restored by the time the error
            // surfaces here.
            eprintln!("grimmoire: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Creates the terminal and runs the application to completion.
fn run() -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    let mut app = App::new();
    app.run(&mut terminal)
}
