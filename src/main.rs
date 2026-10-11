//! The `grimmoire` executable: parses its arguments, sets up the terminal, and
//! runs the application.

use std::io;
use std::process::ExitCode;

use grimmoire::app::App;
use grimmoire::terminal::{self, Capabilities, Terminal};
use grimmoire::ui::StageScreen;

/// What the user asked to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The game.
    Play,
    /// The presentation study, for evaluating the rendering direction.
    Study,
    /// Print usage and exit.
    Help,
}

fn main() -> ExitCode {
    match Mode::from_args(std::env::args().skip(1)) {
        Mode::Help => {
            print_usage();
            ExitCode::SUCCESS
        }
        mode => {
            terminal::install_panic_hook();
            match run(mode) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    // The terminal has already been restored by the time the
                    // error surfaces here.
                    eprintln!("grimmoire: {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

impl Mode {
    /// Interprets command-line arguments.
    fn from_args(args: impl Iterator<Item = String>) -> Mode {
        let mut mode = Mode::Play;
        for arg in args {
            match arg.as_str() {
                "--study" | "--stage" => mode = Mode::Study,
                "--help" | "-h" => return Mode::Help,
                _ => {}
            }
        }
        mode
    }
}

/// Creates the terminal and runs the application to completion.
fn run(mode: Mode) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    let mut app = match mode {
        Mode::Study => App::with_root(Capabilities::detect(), Box::new(StageScreen::new())),
        Mode::Play | Mode::Help => App::new(),
    };
    app.run(&mut terminal)
}

/// Prints usage.
fn print_usage() {
    println!("grimmoire — a terminal-native, ASCII-first dark-fantasy wizard RPG");
    println!();
    println!("usage: grimmoire [--study]");
    println!();
    println!("  (no arguments)  play the exploration prototype");
    println!("  --study         open the presentation study: a rendering prototype");
    println!("                  for judging the visual direction and graphics support");
    println!("  -h, --help      show this message");
    println!();
    println!("Grimmoire needs a terminal of at least 80x24.");
}
