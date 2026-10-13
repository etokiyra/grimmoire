//! The opening cinematic: what a new journey is announced with.
//!
//! This plays once, fullscreen, between choosing **New Game** and the first
//! frame of the world. It is a **placeholder** for a longer opening — a first
//! scene rather than the finished article — but it is a real one: a vast
//! cathedral under a huge moon, a great circle drawing itself in the sky, the
//! wizard arriving at its door, a figure watching from the far dark, and the
//! chapter's name written across the top.
//!
//! # What it is made of
//!
//! Nothing here is a new kind of drawing. The sky is [`crate::ui::atmosphere`],
//! the building is [`crate::ui::cathedral`], the figures are the supplied sprites
//! drawn from [`crate::ui::sprite`], and the words are the chapter's own name out
//! of the model. The part that is new is only their **timing**.
//!
//! # Beats
//!
//! Every beat is a window inside [`OPENING`], eased at both ends, so nothing
//! starts or stops with a jolt:
//!
//! | Beat | When | What it is for |
//! | --- | --- | --- |
//! | dark | 0.00–0.20 | the sky comes up out of black, in steps |
//! | rise | 0.10–0.38 | the building climbs out of the ground |
//! | circle | 0.22–0.48 | the great circle draws itself clockwise |
//! | light | 0.34–0.58 | the windows light, in order, from the door out |
//! | figures | 0.44–0.64 | the wizard arrives; something watches from the far dark |
//! | words | 0.58–0.86 | the name is written, then the chapter beneath it |
//! | settle | 0.86–0.94 | it holds, and is looked at |
//! | away | 0.94–1.00 | the ground comes up and takes it |
//!
//! The cadences are deliberately different: the sky drifts on the atmosphere's
//! own 4 Hz clock, the circle turns on the frame clock, the motes rise on theirs.
//! Nothing pulses in step with anything else, and nothing flickers.
//!
//! # Skipping, and finishing
//!
//! `esc` or `enter` hands the screen back at once — the application's own
//! curtain covers the change, so a skip still reads as a transition rather than
//! a jump. A release never skips, and skipping is not a menu action: it does one
//! thing, which is to let the game underneath start.
//!
//! When the sequence ends on its own the ground rises over it first, and then the
//! same hand-back happens.
//!
//! # Handing over
//!
//! The cinematic does **not** build a world or touch a save. It ends by returning
//! [`Message::Finished`], and the title screen — which owns the save store and
//! knows what the player asked for — pushes the game. That is the same route a
//! conversation uses to change the world, and it is why this file can be replaced
//! wholesale without touching `New Game` at all.

use std::time::Duration;

use super::FrameContext;
use super::atmosphere::{Atmosphere, Drift, Moon, Ramp, Rgb, Sky};
use super::cathedral;
use super::ornament;
use super::screen::{Command, Message, Screen};
use super::sprite::{self, ArtInks};
use crate::game::{Figure, areas};
use crate::input::{Key, KeyCode};
use crate::render::{CanvasView, Frame, Rect, Style, glyph};
use crate::terminal::{Capabilities, ColorSupport};

/// How long the whole sequence takes, if nobody skips it.
const OPENING: Duration = PROLOGUE_HOLD;

/// The prologue, scene by scene: what it shows and how long it holds.
///
/// A table rather than a script, because that is all this is. A future opening
/// with more scenes adds rows here and a `compose` arm for each; nothing about
/// skipping, resizing, handing over or the sky depends on how many there are.
///
/// The first four are the story: the village and its water, the shrine deciding,
/// the walk down to the water, and what was left afterwards. The fifth is the
/// chapter card, which is the scene the previous phase built.
static SCENES: [Scene; 5] = [
    Scene::Village,
    Scene::Rite,
    Scene::Going,
    Scene::After,
    Scene::Chapter,
];

/// How long each scene holds, in order.
///
/// These are set by what is *in* each scene, not by a house style: a caption
/// takes about two and a half seconds to read twice, so every scene is at least
/// that plus its fades, and the ones carrying the weight get more.
///
/// - **Village** establishes the place and the ordinary life in it. Shortest.
/// - **Rite** is the decision: two captions, a colonnade, and a dial being read.
///   It needs the time to be understood, not just seen.
/// - **Going** is the turn. A hand letting go is the beat the whole prologue is
///   built around, so it is the longest of the story scenes, and its caption
///   arrives *late*, after the figure has already started walking.
/// - **After** is four words and a building with nothing lit in it. It has to sit
///   in silence after everything that came before, not hurry on to the card.
/// - **Chapter** is the card: the building rises, the windows light, the name is
///   written. Every one of those is a beat rather than a state, so it is long
///   enough for all three to land.
static SCENE_HOLD: [Duration; 5] = [
    Duration::from_millis(4400),
    Duration::from_millis(5400),
    Duration::from_millis(6000),
    Duration::from_millis(5200),
    Duration::from_millis(5600),
];

/// The whole prologue: the scenes, holding for as long as they take.
const PROLOGUE_HOLD: Duration = Duration::from_millis(4400 + 5400 + 6000 + 5200 + 5600);

/// What each scene is read with, in the order the scenes come.
///
/// Kept as data rather than inline at each call site so that the pacing can be
/// tested against the captions that actually ship: the test walks this table and
/// checks that every one of these lines is on screen, at full strength, in the
/// last moments of its own scene.
static SCENE_CAPTIONS: [&[&str]; 5] = [
    &[
        "The water had failed before.",
        "They knew what it cost, and they paid it.",
    ],
    &[
        "This time the well was dry and the wood was dying.",
        "Somebody had to be given, or everybody would be.",
    ],
    &[
        "She said she would go down herself.",
        "I have told the moon, she wrote. I did not answer.",
    ],
    &["The water never came back.", "She did."],
    // The chapter card names itself, so it has no caption.
    &[],
];

/// How long each caption waits before arriving.
///
/// A scene that shows something irreversible wants a beat of image first, so the
/// words comment on what the player has already seen rather than announcing it.
static CAPTION_DELAY: [Duration; 5] = [
    Duration::ZERO,
    Duration::ZERO,
    Duration::from_millis(1100),
    Duration::from_millis(700),
    Duration::ZERO,
];

/// How long a caption takes to come up and to go away.
///
/// Fixed in *seconds* rather than as a fraction of the scene: a caption that
/// fades over a fifth of a five-second hold takes a second to become readable,
/// which is a second of the scene spent on the fade instead of on the words.
const CAPTION_FADE: Duration = Duration::from_millis(600);

/// The shortest a caption may be left fully readable, whatever the scene.
///
/// A test holds every scene to this, so shortening a hold until its text cannot
/// be read is a build failure rather than a judgement call.
#[cfg(test)]
const CAPTION_READABLE: Duration = Duration::from_millis(2200);

/// How long the ground takes to rise over it at the end.
const AWAY: Duration = Duration::from_millis(420);
/// How many steps the sky comes up in. A field is a cached picture and cannot
/// fade by itself, so the dark is a short series of skies rather than a blend.
const RISE_STEPS: u32 = 3;

/// The inks the cathedral is drawn in here: darker and colder than the title's,
/// because this one stands against a moon rather than being lit from the front.
const STONE: cathedral::Inks = cathedral::Inks {
    stone: Rgb::new(14, 14, 26),
    stone_high: Rgb::new(24, 24, 40),
    edge: Rgb::new(96, 116, 168),
    glass: Rgb::new(255, 206, 120),
    door: Rgb::new(220, 164, 92),
    dark_window: Rgb::new(10, 10, 20),
};

/// One scene of the prologue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scene {
    /// The village below the wood, and a well with no water in it.
    Village,
    /// The shrine's hall: three cold braziers and a decision being made.
    Rite,
    /// The walk down to the water, and a hand letting go.
    Going,
    /// What was left: the chapel, and one figure in the doorway.
    After,
    /// The chapter card.
    Chapter,
}

/// A smooth 0-to-1 ramp between two points of the sequence.
fn ease(progress: f32, from: f32, to: f32) -> f32 {
    if to <= from {
        return f32::from(progress >= to);
    }
    let t = ((progress - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The sky, at a given brightness.
///
/// The whole point of the first beat is that the night arrives rather than being
/// switched on, and the module has no brightness knob: a sky is a picture. So the
/// stops are taken from the shipped night and mixed towards black here, which
/// costs the module nothing and keeps this file's art in this file.
fn opening_sky(brightness: f32) -> Sky {
    let base = Sky::night();
    let black = Rgb::new(3, 3, 9);
    let dim = |colour: Rgb| colour.mix(black, 1.0 - brightness.clamp(0.0, 1.0));
    let mut sky = Sky {
        ramp: Ramp::new(
            base.ramp
                .stops()
                .iter()
                .map(|stop| dim(*stop))
                .collect::<Vec<_>>(),
        ),
        glow: dim(base.glow),
        ..base
    };
    // A huge moon, high and to the right, clear of the name. It is not there at
    // all until the sky is most of the way up: a moon cannot be dimmed, so it
    // arrives rather than fading, and arriving into a brightening sky reads as
    // coming out from behind the dark.
    // High and far to the right, and with a tight halo: a wide one washes out
    // the end of the name, which is written along the same band of sky.
    sky.moon = (brightness > 0.55).then_some(Moon {
        u: 0.82,
        v: 0.10,
        radius: 0.070,
        halo: 0.19,
    });
    sky.stars = 0.7 + brightness * 0.9;
    sky.aurora = Some(dim(Rgb::new(104, 128, 226)));
    sky.drift = Drift::Band {
        top: 0.0,
        bottom: 0.28,
    };
    sky.tick_rate = 4.0;
    sky
}

/// The sky for a scene.
///
/// The chapter card keeps the night the previous phase composed. The four story
/// scenes are the same night seen at different hours: a dusk with the village
/// lights coming on, a cold blue hour at the shrine, the last dark before
/// morning at the water, and almost nothing at all afterwards.
fn scene_sky(scene: Scene, brightness: f32) -> Sky {
    let mut sky = opening_sky(brightness);
    match scene {
        Scene::Village => {
            sky.glow = Rgb::new(255, 158, 96);
            sky.glow_row = 0.72;
            sky.glow_width = 0.30;
            sky.glow_span = 0.9;
            sky.glow_strength = 0.34;
        }
        Scene::Rite => {
            sky.glow = Rgb::new(120, 150, 226);
            sky.glow_row = 0.24;
            sky.glow_strength = 0.20;
            sky.stars = 1.2;
        }
        Scene::Going => {
            sky.glow = Rgb::new(86, 108, 178);
            sky.glow_row = 0.62;
            sky.glow_width = 0.14;
            sky.glow_strength = 0.14;
            sky.stars = 0.7;
        }
        Scene::After => {
            sky.glow = Rgb::new(72, 84, 132);
            sky.glow_strength = 0.08;
            sky.aurora = None;
            sky.stars = 0.5;
            sky.moon = None;
        }
        Scene::Chapter => {}
    }
    sky
}

/// The opening screen.
#[derive(Debug)]
pub struct OpeningScreen {
    /// Time since it began.
    age: Duration,
    /// Time since it began leaving, if it is.
    leaving: Option<Duration>,
    /// The sky, and its cache.
    atmosphere: Atmosphere,
    /// Which step of the sky's rise is loaded, so it is rebuilt only on a change.
    step: u32,
    /// Which scene the sky was built for.
    scene: Scene,
    /// What the terminal can show.
    support: ColorSupport,
    /// The viewport the sky was resolved for.
    area: Rect,
    /// The chapter's name, out of the model rather than invented here.
    chapter: String,
}

impl OpeningScreen {
    /// Creates the opening.
    #[must_use]
    pub fn new() -> Self {
        OpeningScreen {
            age: Duration::ZERO,
            leaving: None,
            atmosphere: Atmosphere::new(opening_sky(0.0), ColorSupport::Ansi16),
            step: 0,
            scene: Scene::Village,
            support: ColorSupport::Ansi16,
            area: Rect::new(0, 0, 0, 0),
            // The chapter names itself out of the model rather than out of this
            // file, so a rename anywhere else renames it here too.
            chapter: areas::chapel().name().to_uppercase(),
        }
    }

    /// Where the sequence is: which scene, how long it has been held, and how
    /// long it holds for. Times, not fractions, because the caption's fades are
    /// measured in seconds and a fraction of a scene is a different length in
    /// every scene.
    fn scene_clock(&self) -> (Scene, Duration, Duration) {
        let mut left = self.age;
        for (index, hold) in SCENE_HOLD.iter().enumerate() {
            if left < *hold {
                return (SCENES[index], left, *hold);
            }
            left = left.saturating_sub(*hold);
        }
        let last = SCENE_HOLD[SCENES.len() - 1];
        (Scene::Chapter, last, last)
    }

    /// Where the sequence is: which scene, and how far through it, 0 to 1.
    ///
    /// This is the *current scene's* progress rather than the whole prologue's.
    /// The chapter card is the last scene, and it is the scene that has to build
    /// itself — the building rising, the windows lighting, the name arriving — so
    /// reading whole-sequence progress here would have all of that finished
    /// before the card was even reached.
    fn where_we_are(&self) -> (Scene, f32) {
        let (scene, elapsed, hold) = self.scene_clock();
        (
            scene,
            (elapsed.as_secs_f32() / hold.as_secs_f32()).clamp(0.0, 1.0),
        )
    }

    /// How far through its own scene the sequence is, 0 to 1.
    ///
    /// The *current scene's* progress, not the prologue's. The chapter card is the
    /// last scene and the one that has to build itself — the building rising, the
    /// windows lighting, the name arriving — so whole-prologue progress would have
    /// all of that finished before the card was even reached. Every fraction below
    /// is therefore a fraction of one scene's hold, not of the whole sequence.
    fn progress(&self) -> f32 {
        self.where_we_are().1
    }

    /// Seconds since the prologue began, for motion that must not stop when a
    /// scene's reveal is over.
    ///
    /// A scene's `progress` drives its *reveal*, which is finite by design. The
    /// clock drives the weather on top of it — water moving, a lamp breathing,
    /// dust crossing a colonnade — so that a long hold stays alive without
    /// restarting the reveal or competing with the words.
    fn clock(&self) -> f32 {
        self.age.as_secs_f32()
    }

    /// How bright the sky should be by now, 0 to 1.
    ///
    /// Only the first scene arrives out of the dark. The rest cut, because a cut
    /// between two held images is what a sequence of scenes *is*.
    fn brightness(&self) -> f32 {
        let (scene, progress) = self.where_we_are();
        if scene == Scene::Village {
            ease(progress, 0.0, 0.30)
        } else {
            1.0
        }
    }

    /// The step of the rise that brightness falls in.
    fn rise_step(&self) -> u32 {
        ((self.brightness() * (RISE_STEPS - 1) as f32).round() as u32).min(RISE_STEPS - 1)
    }

    /// Begins handing the screen back.
    fn depart(&mut self) {
        if self.leaving.is_none() {
            self.leaving = Some(Duration::ZERO);
        }
    }

    /// Rebuilds the sky for the step the rise has reached.
    fn load_sky(&mut self) {
        self.scene = self.where_we_are().0;
        self.step = self.rise_step();
        // Only the first scene arrives out of the dark; the rest cut, because a
        // cut between two held images is what a sequence of scenes is.
        let brightness = if self.scene == Scene::Village {
            self.step as f32 / (RISE_STEPS - 1) as f32
        } else {
            1.0
        };
        self.atmosphere = Atmosphere::new(scene_sky(self.scene, brightness), self.support);
        self.atmosphere.resolve(self.area);
    }

    /// Where the horizon is.
    fn horizon(&self, area: Rect) -> u16 {
        area.bottom().saturating_sub((area.height / 6).clamp(3, 7))
    }

    /// The building, the sky and the circle: everything drawn behind the actors.
    fn compose_scene(&self, frame: &mut Frame, area: Rect, support: ColorSupport) {
        let progress = self.progress();
        let horizon = self.horizon(area);
        let mut view = frame.view(area);

        // The great circle, in the sky behind the building, drawing itself
        // clockwise out of nothing. It is the one shape a cell grid makes well.
        self.compose_circle(&mut view, area, horizon, progress, support);

        // The building: as tall as the room above the horizon allows, which at a
        // full viewport is most of the screen. It is the composition's mass.
        let space = horizon
            .saturating_sub(area.y)
            .saturating_sub(area.height / 8);
        if space >= 10 {
            cathedral::draw(
                &mut view,
                horizon,
                area.width,
                space,
                &cathedral::Look {
                    inks: STONE,
                    support,
                    risen: ease(progress, 0.10, 0.38),
                    lit: ease(progress, 0.34, 0.58),
                },
            );
        }

        // A rule along the horizon, so the ground reads as ground.
        let ink = Rgb::new(58, 64, 96).mix(Rgb::new(255, 210, 150), self.brightness() * 0.2);
        for x in 0..area.width {
            let _ = x;
        }
        for x in 0..area.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(ink.to_color(support)),
            );
        }
    }

    /// Says the words this scene is read with, if it has any.
    ///
    /// One place rather than four, so the pacing test can hold the shipped
    /// caption table to the shipped hold table.
    fn scene_caption(&self, view: &mut CanvasView, area: Rect) {
        let (scene, elapsed, hold) = self.scene_clock();
        let index = SCENES.iter().position(|item| *item == scene).unwrap_or(0);
        self.caption(view, area, index, elapsed, hold);
    }

    /// The caption of the scene at `index`, high in the frame.
    ///
    /// The fades are fixed in seconds rather than as a fraction of the scene, so
    /// the window a caption is fully readable is `hold - delay - 2 *
    /// CAPTION_FADE` whatever the scene is. The delay lets a scene hold its image
    /// for a beat before naming it, which is what makes the words land as a
    /// comment on what the player has already seen rather than as a title card.
    fn caption(
        &self,
        view: &mut CanvasView,
        area: Rect,
        index: usize,
        elapsed: Duration,
        hold: Duration,
    ) {
        let lines = SCENE_CAPTIONS[index];
        if lines.is_empty() {
            return;
        }
        let delay = CAPTION_DELAY[index];
        // How far above the horizon the words sit. High in the sky for the
        // scenes that are sky and figures; lower for the ruined chapel, whose
        // facade fills the frame and whose rose window would otherwise be
        // written over. The words belong in the dark of the nave, under it.
        let gap = match SCENES[index] {
            Scene::After => 6,
            _ => 11,
        };
        let fade = CAPTION_FADE.as_secs_f32();
        let since = elapsed.saturating_sub(delay).as_secs_f32();
        let left = (hold.as_secs_f32() - elapsed.as_secs_f32()).max(0.0);
        let raw = (since / fade)
            .clamp(0.0, 1.0)
            .min((left / fade).clamp(0.0, 1.0));
        let shown = raw * raw * (3.0 - 2.0 * raw);
        if shown <= 0.02 {
            return;
        }
        let ink = Rgb::new(216, 212, 228).mix(Rgb::new(18, 18, 28), 1.0 - shown);
        // High in the sky, well clear of the horizon: a caption laid across a
        // figure is a caption nobody reads, and every one of these scenes has
        // somebody standing at the bottom of it.
        let row = self
            .horizon(area)
            .saturating_sub(gap + lines.len() as u16 * 2);
        for (offset, line) in lines.iter().enumerate() {
            let width = glyph::display_width(line);
            if width > area.width {
                continue;
            }
            view.put_str(
                area.centered_x(width),
                row.saturating_add(offset as u16 * 2),
                line,
                Style::new().with_fg(ink.to_color(self.support)),
            );
        }
    }

    /// A distant silhouette: supplied art, dimmed to a shape.
    fn silhouette(level: u8, eye: u8, support: ColorSupport) -> ArtInks {
        let dim = |value: u8| Rgb::new(value / 3, value / 3, value).to_color(support);
        ArtInks {
            outline: dim(level),
            body: dim(level / 2),
            shade: dim(level / 3),
            light: dim(level),
            highlight: dim(level / 3),
            metal: dim(level / 2),
            eye: dim(eye),
            accent: dim(level / 3),
        }
    }

    /// The village below the wood, and one light going out.
    fn compose_village(&self, frame: &mut Frame, area: Rect, progress: f32, clock: f32) {
        let horizon = self.horizon(area);
        let support = self.support;
        let mut view = frame.view(area);
        let stone = Rgb::new(22, 20, 34);
        let roof = Rgb::new(40, 35, 54);
        let lit = Rgb::new(255, 176, 96);

        // Stepped roofs along the horizon, so it reads as a place and not a
        // fence. The lamps go out one by one: the village stopping for the
        // night, which is when it decided.
        let out = (progress * 7.0) as usize;
        for (index, offset) in [-0.36f32, -0.24, -0.08, 0.02, 0.18, 0.34]
            .iter()
            .enumerate()
        {
            let left = (f32::from(area.width) * (0.5 + offset)) as i32;
            let span = (f32::from(area.width) * 0.09).round().max(4.0) as i32;
            let height = 3 + (index % 3) as i32;
            for dy in 0..height {
                let y = horizon.saturating_sub(dy as u16);
                for dx in 0..span {
                    let x = left + dx;
                    if x < 0 || x >= i32::from(area.width) || y >= area.height {
                        continue;
                    }
                    let ink = if dy == height - 1 { roof } else { stone };
                    view.put_char(
                        x as u16,
                        y,
                        '█',
                        Style::new().with_fg(ink.to_color(support)),
                    );
                }
            }
            if index >= out {
                // A lamp is a small fire, so it breathes rather than glowing
                // steadily. The flicker runs on the clock rather than on the
                // reveal, so the village stays alive through the long hold after
                // the last lamp has gone out — which is when it matters most.
                let flicker = 0.78 + 0.22 * (clock * 2.4 + index as f32 * 1.7).sin();
                let lamp = lit.mix(Rgb::new(146, 78, 30), 1.0 - flicker);
                for (dx, dy) in [(span / 3, 1), (span / 3 + 2, 2)] {
                    let x = left + dx;
                    let y = horizon.saturating_sub(dy as u16);
                    if x >= 0 && x < i32::from(area.width) && y < area.height {
                        view.put_char(
                            x as u16,
                            y,
                            '▖',
                            Style::new().with_fg(lamp.to_color(support)).bold(),
                        );
                    }
                }
            }
        }

        // The well, with its rope still over the bar.
        let well_x = area.centered_x(9).saturating_sub(area.width / 6);
        for (offset, line) in ["  ___  ", " |   | ", " |___| "].iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = well_x.saturating_add(column as u16);
                let y = horizon.saturating_sub(2).saturating_add(offset as u16);
                if x < area.width && y < area.height {
                    view.put_char(
                        x,
                        y,
                        ch,
                        Style::new().with_fg(Rgb::new(126, 110, 88).to_color(support)),
                    );
                }
            }
        }

        // One figure walking back towards the houses, away from the well.
        let walker = sprite::for_figure(Figure::Veiled);
        let travelled = (ease(progress, 0.20, 0.80) * 7.0) as u16;
        let x = well_x
            .saturating_sub(12)
            .saturating_add(travelled)
            .min(area.width.saturating_sub(walker.width));
        let y = horizon.saturating_sub(walker.height.saturating_sub(1));
        if y + walker.height <= area.height {
            sprite::draw(
                &mut view,
                x,
                y,
                walker,
                sprite::Pose::standing(),
                Self::silhouette(120, 200, support),
                false,
            );
        }

        self.scene_caption(&mut view, area);
    }

    /// The shrine's hall: three cold braziers, a dial, and a decision.
    fn compose_rite(&self, frame: &mut Frame, area: Rect, progress: f32, clock: f32) {
        let horizon = self.horizon(area);
        let support = self.support;
        let mut view = frame.view(area);
        let pillar = Rgb::new(66, 64, 96);
        let dial = Rgb::new(158, 178, 228);

        // A colonnade, so the room reads as a room rather than as a night.
        let columns = (area.width / 14).clamp(3, 8);
        for index in 0..columns {
            let x = (f32::from(area.width) * (index as f32 + 0.7) / columns as f32) as u16;
            for dy in 0..7u16 {
                let y = horizon.saturating_sub(dy);
                view.put_char(
                    x,
                    y,
                    if dy == 6 { '▄' } else { '║' },
                    Style::new().with_fg(pillar.to_color(support)),
                );
            }
        }
        for x in 0..area.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(Rgb::new(30, 30, 48).to_color(support)),
            );
        }

        // The dial, read left to right: waning, full, waxing.
        let dial_x = area.centered_x(9);
        let row = horizon.saturating_sub(3);
        let shown = ease(progress, 0.05, 0.45);
        for (index, mark) in ['◜', '◯', '◝'].iter().enumerate() {
            if (index as f32 + 0.5) / 3.0 > shown {
                continue;
            }
            view.put_char(
                dial_x.saturating_add(index as u16 * 3),
                row,
                *mark,
                Style::new().with_fg(dial.to_color(support)).bold(),
            );
        }

        // Three braziers, and nobody lights them. The refusal is the scene.
        for index in 0..3u16 {
            view.put_char(
                dial_x.saturating_add(index * 3),
                row.saturating_sub(2),
                '▄',
                Style::new().with_fg(Rgb::new(52, 50, 74).to_color(support)),
            );
        }

        // Cold air crossing the hall, on the clock: ten slow motes, so a long
        // hold reads as a cold room rather than as a still frame. They never
        // touch the dial, the figures or the caption band.
        for step in 0..10u16 {
            let seed = f32::from(step) * 2.399;
            let travel = (clock * (0.30 + (step % 3) as f32 * 0.11) + seed).fract();
            let x = (f32::from(area.width) * travel) as u16;
            let y = horizon.saturating_sub(2 + (seed * 7.0).sin().abs() as u16 * 4);
            if y >= area.y && y < horizon {
                view.put_char(
                    x.min(area.width.saturating_sub(1)),
                    y,
                    '·',
                    Style::new().with_fg(Rgb::new(74, 88, 132).to_color(support)),
                );
            }
        }

        // The priestess at the dial, and the lantern keeper at the far end.
        let priest = sprite::for_figure(Figure::Priestess);
        let x = area.centered_x(priest.width);
        let y = horizon.saturating_sub(priest.height.saturating_sub(1));
        if y + priest.height <= area.height {
            sprite::draw(
                &mut view,
                x,
                y,
                priest,
                sprite::Pose::standing(),
                priest.inks,
                false,
            );
        }
        let watcher = sprite::for_figure(Figure::Lantern);
        let wx = area.width.saturating_sub(watcher.width + 6);
        if y + watcher.height <= area.height {
            sprite::draw(
                &mut view,
                wx,
                y,
                watcher,
                sprite::Pose::standing(),
                Self::silhouette(110, 190, support),
                true,
            );
        }

        self.scene_caption(&mut view, area);
    }

    /// The water, and two figures at its edge.
    fn compose_going(&self, frame: &mut Frame, area: Rect, progress: f32, clock: f32) {
        let horizon = self.horizon(area);
        let support = self.support;
        let mut view = frame.view(area);
        let glint = Rgb::new(152, 178, 234);

        // Still water below the horizon, brightest where the moon is, so the
        // eye has somewhere to go.
        for y in horizon..area.height {
            let depth = f32::from(y - horizon) / f32::from(area.height - horizon).max(1.0);
            let ink = Rgb::new(28, 40, 74).mix(Rgb::new(8, 9, 18), depth * 0.7);
            view.fill_style(
                Rect::new(0, y, area.width, 1),
                Style::new().with_bg(ink.to_color(support)),
            );
            for x in 0..area.width {
                let centre = 1.0 - ((f32::from(x) / f32::from(area.width)) - 0.5).abs() * 2.0;
                if centre > 0.5 && (usize::from(x) + usize::from(y)) % 7 == 0 {
                    // The moonlight on the water travels, row by row, on the
                    // clock. It is the only thing that moves in the longest hold
                    // of the prologue, and it moves very slowly on purpose.
                    let swell = 0.45 + 0.55 * (clock * 0.8 + f32::from(y) * 0.6).sin();
                    let spark = glint.mix(Rgb::new(44, 56, 96), 1.0 - swell);
                    view.put_char(x, y, '·', Style::new().with_fg(spark.to_color(support)));
                }
            }
        }
        for x in 0..area.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(Rgb::new(46, 46, 68).to_color(support)),
            );
        }

        // Two figures on the lip. The one on the right walks in as the scene
        // runs; the one on the left does not move at all, which is the beat.
        let near = sprite::for_figure(Figure::Mage);
        let far = sprite::for_figure(Figure::Veiled);
        let left_x = area.centered_x(near.width).saturating_sub(area.width / 6);
        let y = horizon.saturating_sub(near.height.saturating_sub(1));
        if y + near.height <= area.height {
            sprite::draw(
                &mut view,
                left_x,
                y,
                near,
                sprite::Pose::standing(),
                near.inks,
                false,
            );
        }
        let gone = (ease(progress, 0.20, 0.92) * 6.0) as u16;
        let right_x = area
            .centered_x(far.width)
            .saturating_add(area.width / 6)
            .saturating_add(gone);
        if y + far.height <= area.height && right_x + far.width <= area.width {
            sprite::draw(
                &mut view,
                right_x,
                y,
                far,
                sprite::Pose::standing(),
                Self::silhouette(130, 220, support),
                true,
            );
        }

        self.scene_caption(&mut view, area);
    }

    /// What was left: the chapel, with nothing lit in it.
    fn compose_after(&self, frame: &mut Frame, area: Rect, progress: f32, clock: f32) {
        let horizon = self.horizon(area);
        let support = self.support;
        let mut view = frame.view(area);
        let space = horizon
            .saturating_sub(area.y)
            .saturating_sub(area.height / 8);
        if space >= 10 {
            cathedral::draw(
                &mut view,
                horizon,
                area.width,
                space,
                &cathedral::Look {
                    inks: cathedral::Inks {
                        stone: Rgb::new(12, 12, 22),
                        stone_high: Rgb::new(20, 20, 34),
                        edge: Rgb::new(62, 72, 106),
                        glass: Rgb::new(120, 130, 160),
                        door: Rgb::new(96, 84, 78),
                        dark_window: Rgb::new(8, 9, 16),
                    },
                    support,
                    risen: 1.0,
                    // Nothing is lit. The windows are glass and nothing else.
                    lit: 0.0,
                },
            );
        }
        for x in 0..area.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(Rgb::new(46, 46, 68).to_color(support)),
            );
        }

        // One figure in the doorway, not moving.
        let holder = sprite::for_figure(Figure::Veiled);
        let x = area.centered_x(holder.width);
        let y = horizon.saturating_sub(holder.height.saturating_sub(1));
        if ease(progress, 0.10, 0.55) > 0.2 && y + holder.height <= area.height {
            sprite::draw(
                &mut view,
                x,
                y,
                holder,
                sprite::Pose::standing(),
                Self::silhouette(140, 210, support),
                false,
            );
        }

        // Almost nothing moves here, and that is the scene: a mote or two crossing
        // the empty nave on the clock, slow enough that a player may not notice.
        // The stillness is the point; this is only so the frame is not a still.
        for step in 0..3u16 {
            let seed = f32::from(step) * 3.1;
            let travel = (clock * 0.05 + seed).fract();
            let x = (f32::from(area.width) * travel) as u16;
            let y = horizon.saturating_sub(6 + (seed * 5.0).sin().abs() as u16 * 5);
            if y >= area.y && y < horizon {
                view.put_char(
                    x.min(area.width.saturating_sub(1)),
                    y,
                    '·',
                    Style::new().with_fg(Rgb::new(54, 58, 80).to_color(support)),
                );
            }
        }

        self.scene_caption(&mut view, area);
    }

    /// The magic circle: an outer ring, an inner ring of runes, and four spokes.
    fn compose_circle(
        &self,
        view: &mut CanvasView,
        area: Rect,
        horizon: u16,
        progress: f32,
        support: ColorSupport,
    ) {
        let drawn = ease(progress, 0.22, 0.48);
        if drawn <= 0.0 {
            return;
        }
        let bounds = view.bounds();
        // In the sky, to one side, where nothing else is: a circle *behind* the
        // building is hidden by it, and one wide enough to frame the whole facade
        // cannot fit the height of a terminal. As its own light in the open it
        // reads at once, which is the whole point of drawing it.
        let radius = (f32::from(area.width) * 0.15)
            .min(f32::from(horizon.saturating_sub(area.y)) * 0.30)
            .max(5.0);
        let centre_x = f32::from(bounds.width) * 0.30;
        let centre_y = f32::from(horizon.saturating_sub(area.y)) * 0.30;
        let turn = self.age.as_secs_f32() * 0.12;
        // Bright, because a pale grey ring on a night sky is noise. The two rings
        // are told apart by colour and by glyph, not by brightness alone.
        let outer = Rgb::new(196, 216, 255);
        let inner = Rgb::new(255, 214, 150);

        // The sweep runs from the top clockwise and hides what it has not reached,
        // so the circle is drawn rather than switched on.
        let ring = |view: &mut CanvasView, ratio: f32, glyphs: &[char], ink: Rgb, from: f32| {
            let steps = 44;
            for index in 0..steps {
                let t = index as f32 / steps as f32;
                if (t - from).rem_euclid(1.0) > drawn {
                    continue;
                }
                let angle = t * std::f32::consts::TAU + turn;
                let x = centre_x + angle.cos() * radius * ratio;
                let y = centre_y + angle.sin() * radius * ratio * 0.44;
                if x < 0.0 || y < 0.0 {
                    continue;
                }
                let (x, y) = (x as u16, y as u16);
                if x >= bounds.width || y >= bounds.height {
                    continue;
                }
                view.put_char(
                    x,
                    y,
                    glyphs[index % glyphs.len()],
                    Style::new().with_fg(ink.to_color(support)).bold(),
                );
            }
        };
        ring(view, 1.0, &['◆', '·'], outer, 0.0);
        ring(view, 0.70, &['✧', '·'], inner, 0.5);

        // Four spokes, drawn once the ring has closed, so the circle fills in
        // rather than arriving all at once.
        if drawn < 0.85 {
            return;
        }
        for spoke in 0..4 {
            let angle = spoke as f32 / 4.0 * std::f32::consts::TAU + turn;
            for step in 0..8 {
                let along = 0.70 + step as f32 / 8.0 * 0.30;
                let x = centre_x + angle.cos() * radius * along;
                let y = centre_y + angle.sin() * radius * along * 0.44;
                if x < 0.0 || y < 0.0 {
                    continue;
                }
                let (x, y) = (x as u16, y as u16);
                if x >= bounds.width || y >= bounds.height {
                    continue;
                }
                view.put_char(
                    x,
                    y,
                    if step % 3 == 0 { '◆' } else { '·' },
                    Style::new().with_fg(inner.to_color(support)),
                );
            }
        }
    }

    /// The wizard, something watching from the far dark, and the motes.
    fn compose_actors(&self, frame: &mut Frame, area: Rect, support: ColorSupport) {
        let progress = self.progress();
        let horizon = self.horizon(area);
        let mut view = frame.view(area);
        let arrived = ease(progress, 0.44, 0.64);
        if arrived <= 0.0 {
            return;
        }

        // The wizard, at the door, on the left of the centre so the rose window
        // above is not covered.
        let hero = sprite::for_figure(Figure::Mage);
        let hero_x = area
            .centered_x(hero.width)
            .saturating_sub(area.width / 9)
            .max(1);
        let hero_y = horizon.saturating_sub(hero.height.saturating_sub(1));
        if hero_y + hero.height <= area.height {
            // A pool of light at the feet, widening as the figure settles.
            let pool = (arrived * 9.0).round() as u16;
            for dx in 0..pool {
                for dy in 0..2u16 {
                    let x = hero_x.saturating_sub(pool / 2).saturating_add(dx);
                    let y = hero_y.saturating_add(hero.height).saturating_add(dy);
                    if x < area.width && y < area.height {
                        let glow = Rgb::new(255, 236, 190)
                            .mix(Rgb::new(10, 9, 20), 0.30 + dy as f32 * 0.3);
                        view.put_char(x, y, '▁', Style::new().with_fg(glow.to_color(support)));
                    }
                }
            }
            let pose = sprite::Pose::Idle {
                breathing: (self.age.as_secs_f32() * 0.9).sin() > 0.3,
            };
            sprite::draw(&mut view, hero_x, hero_y, hero, pose, hero.inks, false);
        }

        // Something else is out there: the same supplied art, drawn as a
        // silhouette so it reads as distance rather than as a second character
        // standing in the room.
        let far = sprite::for_figure(Figure::Veiled);
        // Outside the building, not in front of it: the facade is three fifths of
        // the width, so anything nearer the centre is hidden by it.
        let far_x = area
            .centered_x(far.width)
            .saturating_add(area.width * 36 / 100)
            .min(area.width.saturating_sub(far.width));
        let far_y = horizon
            .saturating_sub(far.height.saturating_sub(1))
            .saturating_sub(1);
        let shown = ease(progress, 0.50, 0.66);
        if shown > 0.4 && far_y + far.height <= area.height {
            let cold = |level: u8| Rgb::new(level / 3, level / 2, level).to_color(support);
            let dark = Rgb::new(18, 22, 38).to_color(support);
            let inks = ArtInks {
                outline: cold(120),
                body: dark,
                shade: dark,
                light: cold(60),
                highlight: dark,
                metal: dark,
                // Two cold points, which is what makes a silhouette a watcher.
                eye: cold(230),
                accent: dark,
            };
            sprite::draw(
                &mut view,
                far_x,
                far_y,
                far,
                sprite::Pose::standing(),
                inks,
                true,
            );
        }

        // Motes, rising through the light the sky is throwing down.
        let time = self.age.as_secs_f32();
        for index in 0..30u32 {
            let rand = |n: u32| {
                let value = index
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(n.wrapping_mul(40_503));
                (value >> 8) as f32 / 16_777_216.0
            };
            let life = (time * (0.04 + rand(1) * 0.05) + rand(2)).fract();
            let fade = (life * std::f32::consts::PI).sin().max(0.0);
            if fade * arrived <= 0.06 {
                continue;
            }
            let x = (rand(3) * f32::from(area.width)) as u16;
            let travel = (life * f32::from(horizon.saturating_sub(area.y))) as u16;
            let y = horizon.saturating_sub(travel);
            if x >= area.width || y >= area.height {
                continue;
            }
            let ink = Rgb::new(255, 226, 176).mix(Rgb::new(120, 140, 210), 1.0 - fade * arrived);
            view.put_char(
                x,
                y,
                if fade > 0.7 { '✧' } else { '·' },
                Style::new().with_fg(ink.to_color(support)),
            );
        }
    }

    /// The name, and the chapter beneath it.
    fn compose_words(&self, frame: &mut Frame, area: Rect, support: ColorSupport) {
        let progress = self.progress();
        let shown = ease(progress, 0.58, 0.74);
        if shown <= 0.0 {
            return;
        }
        let mut view = frame.view(area);

        // The name, one row, letter-spaced: the same wordmark the title uses,
        // because a game's opening and its front door should be the same voice.
        let name = "GRIMMOIRE";
        let gap = 4usize;
        let letters: Vec<char> = name.chars().collect();
        let total = letters.len() * (1 + gap);
        let reveal = (shown * total as f32).round() as usize;
        let start = area.centered_x(total as u16);
        let row = area.y.saturating_add((area.height / 12).clamp(1, 3));
        let last = letters.len().saturating_sub(1).max(1) as f32;
        for (index, letter) in letters.iter().enumerate() {
            let column = start as usize + index * (1 + gap);
            if column > start as usize + reveal && shown < 1.0 {
                break;
            }
            let t = index as f32 / last;
            let ink = Rgb::new(255, 240, 196).mix(Rgb::new(236, 116, 96), t);
            let at_edge = shown < 1.0 && column + 2 >= start as usize + reveal;
            view.put_char(
                column as u16,
                row,
                *letter,
                Style::new()
                    .with_fg(if at_edge {
                        Rgb::new(255, 250, 236).to_color(support)
                    } else {
                        ink.to_color(support)
                    })
                    .bold(),
            );
        }

        // The chapter, in its own good time, under a rule that widens.
        let chapter = ease(progress, 0.68, 0.86);
        if chapter <= 0.0 {
            return;
        }
        let rule_row = row.saturating_add(1);
        let width = ((area.width.saturating_sub(8).min(52)) as f32 * chapter).round() as u16;
        if width >= 2 {
            let left = area.centered_x(width);
            ornament::rule(
                &mut view,
                left,
                rule_row,
                width,
                Style::new().with_fg(Rgb::new(120, 106, 156).to_color(support)),
            );
            view.put_char(
                area.centered_x(width).saturating_add(width / 2),
                rule_row,
                '◆',
                Style::new().with_fg(Rgb::new(206, 154, 122).to_color(support)),
            );
        }
        if chapter > 0.5 {
            let line = format!("I · {}", self.chapter);
            view.put_str(
                area.centered_x(glyph::display_width(&line)),
                row.saturating_add(2),
                &line,
                Style::new().with_fg(Rgb::new(196, 190, 220).to_color(support)),
            );
        }
    }

    /// The ground rising to take the scene, when the sequence is ending.
    fn compose_away(&self, frame: &mut Frame, area: Rect, age: Duration, context: &FrameContext) {
        let t = (age.as_secs_f32() / AWAY.as_secs_f32()).clamp(0.0, 1.0);
        let covered = (f32::from(area.height) * t).round() as u16;
        if covered == 0 {
            return;
        }
        let mut view = frame.view(area);
        for step in 0..covered {
            let y = area.bottom().saturating_sub(1 + step);
            let lip = Rgb::new(255, 206, 140).mix(Rgb::new(9, 9, 16), step as f32 / 3.0);
            view.fill_style(
                Rect::new(area.x, y, area.width, 1),
                Style::new()
                    .with_fg(if step < 2 { lip } else { Rgb::new(9, 9, 16) }.to_color(self.support))
                    .with_bg(context.palette.background),
            );
        }
    }
}

impl Default for OpeningScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for OpeningScreen {
    fn name(&self) -> &'static str {
        "opening"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Presses only, and never a repeat: a held key must not skip the sequence
        // and then keep going. Skipping is not a menu action — it does one thing.
        if !key.is_down() || key.is_repeat() || self.leaving.is_some() {
            return Command::None;
        }
        match key.code {
            // Skipping is a *departure*, not a hand-back: the ground rises over
            // the scene in a fifth of a second and then the game starts. A cut
            // would be quicker and much worse to watch, and a command returned
            // here could be answered twice if the key repeated.
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                self.depart();
                Command::None
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.age = self.age.saturating_add(delta);
        if let Some(age) = self.leaving {
            let age = age.saturating_add(delta);
            self.leaving = Some(age);
            return;
        }
        // The sky comes up in steps and changes with the scene, and then drifts
        // on its own clock.
        if self.rise_step() != self.step || self.where_we_are().0 != self.scene {
            self.load_sky();
        }
        self.atmosphere.advance(delta);
        self.atmosphere.resolve(self.area);
    }

    fn after_update(&mut self) -> Command {
        // The sequence ends by itself, and then by the ground rising over it.
        if self.leaving.is_none() && self.age >= OPENING {
            self.depart();
        }
        if matches!(self.leaving, Some(age) if age >= AWAY) {
            return Command::Return(Message::Finished);
        }
        Command::None
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let area = frame.bounds();
        if area.is_empty() {
            return;
        }
        // The sky owns every cell's background; everything else is glyphs over it.
        if self.atmosphere.area() == area {
            self.atmosphere.paint(frame);
        }
        if area.width < 24 || area.height < 8 {
            // No room for a scene. Say the one thing that matters and let the
            // player finish it.
            let mut view = frame.view(area);
            let line = "GRIMMOIRE";
            view.put_str(
                area.centered_x(glyph::display_width(line)),
                area.height / 2,
                line,
                Style::new()
                    .with_fg(Rgb::new(255, 230, 170).to_color(self.support))
                    .bold(),
            );
            return;
        }
        // Five scenes, each composed as a whole image, then the chapter card.
        let (scene, progress) = self.where_we_are();
        let clock = self.clock();
        match scene {
            Scene::Village => self.compose_village(frame, area, progress, clock),
            Scene::Rite => self.compose_rite(frame, area, progress, clock),
            Scene::Going => self.compose_going(frame, area, progress, clock),
            Scene::After => self.compose_after(frame, area, progress, clock),
            Scene::Chapter => {
                self.compose_scene(frame, area, self.support);
                self.compose_actors(frame, area, self.support);
                self.compose_words(frame, area, self.support);
            }
        }
        if let Some(age) = self.leaving {
            self.compose_away(frame, area, age, context);
        }
    }

    fn on_capabilities(&mut self, capabilities: Capabilities) {
        if capabilities.colors == self.support {
            return;
        }
        self.support = capabilities.colors;
        self.load_sky();
    }

    fn on_resize(&mut self, area: Rect) {
        self.area = area;
        self.atmosphere.resolve(area);
    }

    fn curtain(&self) -> bool {
        // The opening brings itself in and takes itself out; a second curtain on
        // top would be two fades fighting.
        false
    }

    fn is_animating(&self) -> bool {
        true
    }

    fn next_frame(&self) -> Option<Duration> {
        Some(Duration::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::OpeningScreen;
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement, Rect};
    use crate::terminal::{Capabilities, ColorSupport, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Message, Screen};
    use crate::ui::{FrameContext, Palette};

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Unsupported,
            art: Art::none(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: true,
            key_releases: true,
            motion: crate::ui::settings::Motion::Full,
        }
    }

    fn ready() -> OpeningScreen {
        let mut screen = OpeningScreen::new();
        screen.on_capabilities(Capabilities {
            colors: ColorSupport::TrueColor,
        });
        screen.on_resize(Rect::from_size(120, 40));
        screen
    }

    /// Only the letters of one composed row, so a check is about the words and
    /// not about whatever the sky and the circle drew between them.
    fn row_letters(screen: &OpeningScreen, row: u16) -> String {
        text(screen, 120, 40)
            .lines()
            .nth(row as usize)
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_alphabetic)
            .collect()
    }

    /// Every glyph the screen composed, with the sky's half blocks and the
    /// spacing taken out, so a test can look for a word without caring how it was
    /// letter-spaced or what the field drew behind it.
    fn joined(screen: &OpeningScreen, width: u16, height: u16) -> String {
        text(screen, width, height)
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != super::super::atmosphere::HALF_BLOCK)
            .collect()
    }

    fn text(screen: &OpeningScreen, width: u16, height: u16) -> String {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images: Vec<ImagePlacement> = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        let mut out = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                out.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            out.push('\n');
        }
        out
    }

    /// Runs the sequence forward to a given moment, in frame-sized steps, the way
    /// the application would.
    fn advance_to(screen: &mut OpeningScreen, at: Duration) {
        while screen.age < at {
            screen.update(Duration::from_millis(16));
        }
    }

    #[test]
    fn the_name_and_the_chapter_arrive_rather_than_being_there() {
        let mut screen = ready();
        assert_eq!(
            String::new(),
            row_letters(&screen, 3),
            "the name was there before it was written"
        );
        // The story scenes are read without a wordmark anywhere near the top.
        let into_the_rite: Duration = super::SCENE_HOLD[..2].iter().sum();
        advance_to(&mut screen, into_the_rite);
        assert_eq!(
            String::new(),
            row_letters(&screen, 3),
            "the name arrived before the chapter card did"
        );
        // By the end of the card it has been written, and the chapter named.
        advance_to(&mut screen, super::OPENING);
        assert_eq!("GRIMMOIRE", row_letters(&screen, 3), "the name is wrong");
        assert!(
            row_letters(&screen, 5).ends_with("RUINEDCHAPEL"),
            "the chapter is wrong: {:?}",
            row_letters(&screen, 5)
        );
    }

    #[test]
    fn the_building_rises_and_its_windows_light() {
        let mut screen = ready();
        // The building is the chapter card's, so it rises while the card is on
        // screen rather than having finished somewhere behind the story.
        let chapter: Duration = super::SCENE_HOLD[..4].iter().sum();
        advance_to(&mut screen, chapter + Duration::from_millis(200));
        let early = joined(&screen, 120, 40).matches('█').count();
        advance_to(&mut screen, chapter + Duration::from_millis(3000));
        let late = joined(&screen, 120, 40).matches('█').count();
        assert!(
            late > early,
            "the building did not rise: {early} then {late}"
        );
        assert!(late > 200, "the building is only {late} cells");
    }

    #[test]
    fn every_caption_is_fully_readable_before_its_scene_ends() {
        for index in 0..super::SCENES.len() {
            let lines = super::SCENE_CAPTIONS[index];
            if lines.is_empty() {
                continue;
            }
            let start: Duration = super::SCENE_HOLD[..index].iter().sum();
            let hold = super::SCENE_HOLD[index];
            // The last moment the caption is at full strength, before its tail
            // fade begins.
            let mut screen = ready();
            advance_to(&mut screen, start + hold - super::CAPTION_FADE);
            let frame = text(&screen, 120, 40);
            for line in lines {
                assert!(
                    frame.contains(line),
                    "scene {index} lost its caption before the scene ended: {line:?}"
                );
            }
            // And the window it is fully readable for is long enough to read it.
            let window = hold - super::CAPTION_DELAY[index] - super::CAPTION_FADE * 2;
            assert!(
                window >= super::CAPTION_READABLE,
                "scene {index} holds its words readable for only {window:?}"
            );
        }
    }

    #[test]
    fn a_caption_does_not_arrive_before_its_scene_has_shown_anything() {
        // The delayed captions buy their scene a beat of image first. Checked as
        // behaviour: at the moment the scene opens, the words are not yet there.
        for (index, delay) in super::CAPTION_DELAY.iter().enumerate() {
            if delay.is_zero() {
                continue;
            }
            let start: Duration = super::SCENE_HOLD[..index].iter().sum();
            let mut screen = ready();
            advance_to(&mut screen, start + Duration::from_millis(1));
            let frame = text(&screen, 120, 40);
            for line in super::SCENE_CAPTIONS[index] {
                assert!(
                    !frame.contains(line),
                    "scene {index} said its words before it showed its image"
                );
            }
        }
    }

    #[test]
    fn a_press_skips_from_any_scene_and_hands_back_exactly_once() {
        for index in 0..super::SCENES.len() {
            let start: Duration = super::SCENE_HOLD[..index].iter().sum();
            let mut screen = ready();
            advance_to(&mut screen, start + Duration::from_millis(50));
            assert!(matches!(
                screen.handle_key(Key::new(KeyCode::Esc)),
                Command::None
            ));
            let mut handed_back = 0;
            let mut command = Command::None;
            for _ in 0..20 {
                screen.update(Duration::from_millis(100));
                command = screen.after_update();
                if !matches!(command, Command::None) {
                    handed_back += 1;
                    break;
                }
            }
            assert!(
                matches!(command, Command::Return(Message::Finished)),
                "skipping scene {index} did not hand the screen back"
            );
            assert_eq!(1, handed_back, "scene {index} answered more than once");
        }
    }

    #[test]
    fn the_sky_comes_up_out_of_the_dark_rather_than_being_switched_on() {
        let mut screen = ready();
        screen.update(Duration::from_millis(16));
        let dark = screen.atmosphere.field().crop(Rect::from_size(40, 12));
        for _ in 0..12 {
            screen.update(Duration::from_millis(100));
        }
        let lit = screen.atmosphere.field().crop(Rect::from_size(40, 12));
        let brightest = |canvas: &Canvas| {
            (0..canvas.height())
                .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
                .filter_map(|(x, y)| canvas.get(x, y))
                .filter_map(|cell| cell.style.bg.as_rgb())
                .map(|(r, g, b)| u16::from(r) + u16::from(g) + u16::from(b))
                .max()
                .unwrap_or(0)
        };
        assert!(
            brightest(&lit) > brightest(&dark),
            "the sky never brightened: {} then {}",
            brightest(&dark),
            brightest(&lit)
        );
    }

    #[test]
    fn it_finishes_on_its_own_and_hands_the_screen_back() {
        let mut screen = ready();
        let mut command = Command::None;
        for _ in 0..400 {
            screen.update(Duration::from_millis(100));
            command = screen.after_update();
            if !matches!(command, Command::None) {
                break;
            }
        }
        assert!(
            matches!(command, Command::Return(Message::Finished)),
            "the sequence did not end"
        );
    }

    #[test]
    fn a_press_skips_it_and_a_release_or_a_repeat_does_not() {
        let mut screen = ready();
        for key in [
            Key::new(KeyCode::Enter).released(),
            Key::new(KeyCode::Esc).with_kind(KeyKind::Repeat),
        ] {
            assert!(matches!(screen.handle_key(key), Command::None));
            assert!(!screen.leaving.is_some(), "{key:?} skipped the sequence");
        }
        // A press leaves at once...
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Esc)),
            Command::None
        ));
        assert!(screen.leaving.is_some(), "the press did not skip");
        // ...and the second press is ignored while it is on its way out.
        let before = screen.leaving;
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter)),
            Command::None
        ));
        assert_eq!(before, screen.leaving);
        // Then it finishes, without the player having to do anything else.
        let mut command = Command::None;
        for _ in 0..20 {
            screen.update(Duration::from_millis(100));
            command = screen.after_update();
        }
        assert!(matches!(command, Command::Return(Message::Finished)));
    }

    #[test]
    fn escaping_midway_composes_cleanly_at_any_size() {
        let mut screen = ready();
        for width in [0, 1, 12, 24, 40, 80, 120, 240] {
            for height in [0, 1, 6, 8, 12, 24, 40, 60] {
                screen.on_resize(Rect::from_size(width, height));
                let _ = text(&screen, width, height);
            }
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let mut screen = ready();
        screen.update(Duration::from_millis(3000));
        let mut canvas = Canvas::new(120, 40);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                assert_ne!(
                    Some(crate::render::Color::Default),
                    canvas.get(x, y).map(|cell| cell.style.bg),
                    "cell ({x}, {y}) would show the terminal background"
                );
            }
        }
    }

    #[test]
    fn it_is_composed_of_one_column_glyphs_only() {
        let mut screen = ready();
        for _ in 0..40 {
            screen.update(Duration::from_millis(100));
            let painted = text(&screen, 120, 40);
            for ch in painted.chars() {
                if ch == '\n' || ch == ' ' {
                    continue;
                }
                assert!(
                    crate::render::glyph::is_drawable(ch),
                    "{ch:?} would desynchronise the frame"
                );
            }
        }
    }
}
