//! The `grimmoire` executable: checks the environment, sets up the terminal, and
//! runs the application.
//!
//! Startup is deliberately strict. Grimmoire targets Kitty, and rather than
//! silently falling back to a degraded or misleading rendering mode it reports
//! what is wrong and exits before touching the terminal.

use std::io;
use std::process::ExitCode;

use grimmoire::app::{App, Startup};
use grimmoire::assets::Assets;
use grimmoire::terminal::{self, Capabilities, GraphicsSupport, Terminal};
use grimmoire::ui::StageScreen;
use grimmoire::ui::art::Art;

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
                    // Nothing has been written to the terminal yet, so this lands
                    // on an ordinary screen.
                    eprintln!("grimmoire: {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

/// Whether to draw interface pieces as pixel art.
///
/// Off by default, and deliberately so: the game is ASCII. This exists so the
/// software pixel renderer can be looked at in the presentation study without
/// making it part of the game's identity.
fn pixel_interface_requested(args: &mut impl Iterator<Item = String>) -> bool {
    let from_env = matches!(
        std::env::var("GRIMMOIRE_PIXEL_UI").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    );
    from_env || args.any(|arg| arg == "--pixel-ui")
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

/// Resolves the environment, then runs the requested screen to completion.
fn run(mode: Mode) -> io::Result<()> {
    let capabilities = Capabilities::detect();
    let graphics = terminal::detect();

    // Fail before the alternate screen, so the message is readable.
    match graphics {
        GraphicsSupport::Unsupported => return Err(graphics_error()),
        GraphicsSupport::Unverified => {
            eprintln!(
                "grimmoire: warning: this terminal looks like Kitty but did not confirm the \
                 protocol; images may not appear. Set GRIMMOIRE_ASSUME_KITTY=1 to silence this."
            );
        }
        GraphicsSupport::Confirmed | GraphicsSupport::Forced => {}
    }

    // The world needs no images: it is ASCII. Optional interface art is built
    // only when the development flag asks for it.
    let assets = if pixel_interface_requested(&mut std::env::args().skip(1)) {
        Assets::built()
    } else {
        Assets::default()
    };
    let art = Art::resolve(&assets);

    let startup = Startup {
        capabilities,
        graphics,
        assets,
        art,
        save: grimmoire::save::SaveStore::default_location(),
    };

    let mut terminal = Terminal::new()?;
    let mut app = match mode {
        Mode::Study => App::with_root(startup, Box::new(StageScreen::new())),
        Mode::Play | Mode::Help => App::new(startup),
    };
    app.run(&mut terminal)
}

/// The error shown when the terminal cannot draw Grimmoire at all.
fn graphics_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "this terminal cannot draw Grimmoire.\n\n\
         Grimmoire targets the Kitty terminal emulator and uses its graphics protocol for\n\
         artwork, so it will not start somewhere it would look broken. Launch it in Kitty:\n\n\
             kitty grimmoire\n\n\
         If you are already in Kitty, check that $KITTY_WINDOW_ID is set and that no\n\
         multiplexer is stripping the protocol.\n\n\
         For development in another terminal, set GRIMMOIRE_ASSUME_KITTY=1 to render the\n\
         cell layer only; images will not appear, and that is expected.",
    )
}

/// Prints usage.
fn print_usage() {
    println!("grimmoire — a terminal-native, ASCII-first dark-fantasy wizard RPG");
    println!();
    println!("usage: grimmoire [--study] [--pixel-ui]");
    println!();
    println!("  (no arguments)  play the exploration prototype");
    println!("  --study         open the presentation study: a rendering prototype");
    println!("  --pixel-ui      development: allow pixel-art interface pieces");
    println!("  -h, --help      show this message");
    println!();
    println!("controls: wasd or arrows to move (hold to keep walking), shift to sprint,");
    println!("          e to interact, h for help, q or Esc to quit.");
    println!();
    println!("Requires the Kitty terminal emulator (graphics protocol) and a viewport of");
    println!("at least 80x24.");
}

#[cfg(test)]
mod tests {
    use super::Mode;

    fn parse(args: &[&str]) -> Mode {
        Mode::from_args(args.iter().map(|arg| (*arg).to_string()))
    }

    #[test]
    fn the_default_mode_is_the_game() {
        assert_eq!(Mode::Play, parse(&[]));
        assert_eq!(Mode::Play, parse(&["--unknown"]));
    }

    #[test]
    fn the_study_flags_select_the_study() {
        assert_eq!(Mode::Study, parse(&["--study"]));
        assert_eq!(Mode::Study, parse(&["--stage"]));
        assert_eq!(Mode::Study, parse(&["--study", "--anything"]));
    }

    #[test]
    fn help_wins() {
        assert_eq!(Mode::Help, parse(&["--help"]));
        assert_eq!(Mode::Help, parse(&["-h"]));
        assert_eq!(Mode::Help, parse(&["--study", "-h"]));
    }
}
