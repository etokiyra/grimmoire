//! Kitty graphics: capability policy and on-screen image state.
//!
//! This module owns everything the application knows about the terminal's
//! graphics: whether it can be used, which images have been transmitted, and
//! which placements are currently displayed. It is the only place that decides
//! when to transmit, place, or delete, which is what makes the image lifecycle
//! auditable and testable.
//!
//! # Lifecycle
//!
//! * An image's data is transmitted **once** and kept under a stable id; showing
//!   it again costs nothing. Kitty only re-reads data when the id is reused.
//! * A placement is identified by `(image, placement)`, so re-placing the same
//!   rectangle emits nothing, and moving one replaces it without flicker.
//! * A full redraw clears the screen, and the clear-screen escape code is
//!   specified to clear images too. When that happens the whole graphics state is
//!   re-established: data is retransmitted and placements re-issued.
//! * `clear` deletes everything belonging to the application, and is called on
//!   shutdown, before a resize, and whenever the area changes.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::assets::Assets;
use crate::render::{AssetId, ImagePlacement, Rect};

use super::kitty;

/// What is known about the terminal's graphics capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphicsSupport {
    /// The terminal answered a Kitty protocol query, so graphics are usable.
    Confirmed,
    /// The developer override forced graphics on. Used for tests and for
    /// inspecting the cell composition in a terminal that cannot paint images.
    Forced,
    /// The environment looks like Kitty but nothing was confirmed live.
    Unverified,
    /// No evidence of Kitty support; the application should refuse to start.
    #[default]
    Unsupported,
}

impl GraphicsSupport {
    /// Whether images should be emitted at all.
    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, GraphicsSupport::Unsupported)
    }

    /// Whether support was actually established rather than assumed.
    #[must_use]
    pub const fn is_verified(self) -> bool {
        matches!(self, GraphicsSupport::Confirmed | GraphicsSupport::Forced)
    }

    /// A short label for diagnostics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            GraphicsSupport::Confirmed => "kitty (confirmed)",
            GraphicsSupport::Forced => "kitty (forced)",
            GraphicsSupport::Unverified => "kitty (unverified)",
            GraphicsSupport::Unsupported => "unsupported",
        }
    }
}

/// The pure part of capability detection, so the policy is testable.
///
/// `keyboard_enhancement` is the result of the live Kitty protocol query that
/// `crossterm` performs; `None` means the terminal did not answer.
#[must_use]
pub fn classify(
    environment_claims_kitty: bool,
    live_query: Option<bool>,
    forced: bool,
) -> GraphicsSupport {
    if forced {
        return GraphicsSupport::Forced;
    }
    match live_query {
        // A terminal that answered the Kitty protocol query is speaking the
        // protocol; that is the strongest evidence available without reading
        // graphics acknowledgements (which this application never does).
        Some(true) => GraphicsSupport::Confirmed,
        Some(false) if environment_claims_kitty => GraphicsSupport::Unverified,
        None if environment_claims_kitty => GraphicsSupport::Unverified,
        _ => GraphicsSupport::Unsupported,
    }
}

/// Whether the environment claims to be Kitty.
///
/// `KITTY_WINDOW_ID` is set by Kitty itself for its children, so it is evidence
/// rather than a guess; `TERM` alone is not enough to conclude anything, which is
/// why it only ever contributes to [`GraphicsSupport::Unverified`].
#[must_use]
pub fn environment_claims_kitty(term: &str, term_program: &str, kitty_window: bool) -> bool {
    kitty_window || term.contains("kitty") || term_program.to_ascii_lowercase().contains("kitty")
}

/// Whether the developer override is set.
fn forced() -> bool {
    matches!(
        std::env::var("GRIMMOIRE_ASSUME_KITTY").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Detects graphics support from the environment and a live terminal query.
#[must_use]
pub fn detect() -> GraphicsSupport {
    let claims = environment_claims_kitty(
        &std::env::var("TERM").unwrap_or_default(),
        &std::env::var("TERM_PROGRAM").unwrap_or_default(),
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
    );
    let live = crossterm::terminal::supports_keyboard_enhancement().ok();
    classify(claims, live, forced())
}

/// One displayed image placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    image: u32,
    placement: u32,
    area: Rect,
}

/// What [`Graphics::sync`] did, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Images whose data was transmitted.
    pub transmitted: u32,
    /// Placements emitted or replaced.
    pub placed: u32,
    /// Placements removed.
    pub removed: u32,
}

impl SyncReport {
    /// Whether anything at all was written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.transmitted == 0 && self.placed == 0 && self.removed == 0
    }
}

/// The terminal's graphics state as this application believes it to be.
#[derive(Debug)]
pub struct Graphics {
    support: GraphicsSupport,
    next_id: u32,
    /// Asset handle to Kitty image id.
    loaded: HashMap<AssetId, u32>,
    /// Placements currently displayed, in the order they were requested.
    placed: Vec<Placed>,
    /// Total payload bytes transmitted, for diagnostics.
    transmitted_bytes: u64,
}

impl Graphics {
    /// Creates the state for a given support level.
    #[must_use]
    pub fn new(support: GraphicsSupport) -> Self {
        Graphics {
            support,
            // Zero is not a valid image id in this protocol.
            next_id: 1,
            loaded: HashMap::new(),
            placed: Vec::new(),
            transmitted_bytes: 0,
        }
    }

    /// The support level in force.
    #[must_use]
    pub const fn support(&self) -> GraphicsSupport {
        self.support
    }

    /// How many images have had their data transmitted.
    #[must_use]
    pub fn loaded(&self) -> usize {
        self.loaded.len()
    }

    /// How many placements are believed to be on screen.
    #[must_use]
    pub fn placed(&self) -> usize {
        self.placed.len()
    }

    /// Total image payload bytes sent to the terminal.
    #[must_use]
    pub const fn transmitted_bytes(&self) -> u64 {
        self.transmitted_bytes
    }

    /// The graphics protocol in use, for diagnostics.
    #[must_use]
    pub const fn protocol(&self) -> &'static str {
        kitty::PROTOCOL
    }

    /// Makes the terminal display exactly `wanted`.
    ///
    /// `reset` must be true when the screen has just been cleared, because the
    /// clear also clears images; the whole graphics state is then re-established.
    /// Otherwise only genuine changes are written, so ordinary text and UI
    /// updates cost no image traffic at all.
    pub fn sync<W: Write>(
        &mut self,
        out: &mut W,
        wanted: &[ImagePlacement],
        assets: &Assets,
        reset: bool,
    ) -> io::Result<SyncReport> {
        if !self.support.is_available() {
            return Ok(SyncReport::default());
        }

        let mut report = SyncReport::default();
        if reset {
            kitty::delete_all(out)?;
            self.loaded.clear();
            self.placed.clear();
        }

        // Desired state: placement ids follow the request order, so a screen that
        // keeps a stable order keeps stable ids and gets flicker-free updates.
        let mut desired: Vec<Placed> = Vec::with_capacity(wanted.len());
        for (index, request) in wanted.iter().enumerate() {
            let Some(asset) = assets.get(request.asset) else {
                continue;
            };
            let image = match self.loaded.get(&request.asset) {
                Some(id) => *id,
                None => {
                    let id = self.next_id;
                    self.next_id = self.next_id.saturating_add(1);
                    kitty::transmit_png(out, id, asset.png())?;
                    self.loaded.insert(request.asset, id);
                    self.transmitted_bytes = self
                        .transmitted_bytes
                        .saturating_add(asset.png().len() as u64);
                    report.transmitted = report.transmitted.saturating_add(1);
                    id
                }
            };
            desired.push(Placed {
                image,
                placement: index as u32 + 1,
                area: request.area,
            });
        }

        // Emit placements that are new or have moved.
        for placement in &desired {
            if !self.placed.contains(placement) {
                kitty::place(out, placement.image, placement.placement, placement.area)?;
                report.placed = report.placed.saturating_add(1);
            }
        }

        // Remove placements that are gone, identifying them by (image,
        // placement): a placement that merely moved is still the same placement,
        // and deleting it would remove the one just written.
        let live: Vec<(u32, u32)> = desired
            .iter()
            .map(|placement| (placement.image, placement.placement))
            .collect();
        for stale in &self.placed {
            if !live.contains(&(stale.image, stale.placement)) {
                kitty::delete_placement(out, stale.image, stale.placement)?;
                report.removed = report.removed.saturating_add(1);
            }
        }

        self.placed = desired;
        Ok(report)
    }

    /// Deletes every image and placement this application created.
    pub fn clear<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        self.loaded.clear();
        self.placed.clear();
        if !self.support.is_available() {
            return Ok(());
        }
        kitty::delete_all(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Graphics, GraphicsSupport, classify, environment_claims_kitty};
    use crate::assets::{Asset, Assets};
    use crate::render::{AssetId, ImagePlacement, Rect};

    fn assets() -> Assets {
        // Build a store without touching the filesystem: two distinguishable
        // payloads behind stable handles.
        let dir = std::env::temp_dir().join(format!(
            "grimmoire-graphics-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut first = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        first.extend_from_slice(&[1, 2, 3, 4]);
        std::fs::write(dir.join("a.png"), &first).expect("write");
        std::fs::write(dir.join("b.png"), &first).expect("write");
        let assets = Assets::load_dir(&dir).expect("load");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assets
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn count(sequence: &str, needle: &str) -> usize {
        sequence.matches(needle).count()
    }

    #[test]
    fn classification_distinguishes_the_four_cases() {
        assert_eq!(
            GraphicsSupport::Confirmed,
            classify(true, Some(true), false)
        );
        assert_eq!(GraphicsSupport::Unverified, classify(true, None, false));
        assert_eq!(
            GraphicsSupport::Unverified,
            classify(true, Some(false), false)
        );
        assert_eq!(GraphicsSupport::Unsupported, classify(false, None, false));
        assert_eq!(
            GraphicsSupport::Unsupported,
            classify(false, Some(false), false)
        );
        assert_eq!(
            GraphicsSupport::Forced,
            classify(false, None, true),
            "the override wins"
        );
    }

    #[test]
    fn only_kitty_capable_environments_are_recognised() {
        assert!(environment_claims_kitty("xterm-kitty", "", false));
        assert!(environment_claims_kitty("xterm-256color", "", true));
        assert!(environment_claims_kitty("xterm-256color", "kitty", false));
        assert!(!environment_claims_kitty("xterm-256color", "foot", false));
        assert!(!environment_claims_kitty("foot", "", false));
    }

    #[test]
    fn availability_excludes_unsupported() {
        assert!(!GraphicsSupport::Unsupported.is_available());
        for support in [
            GraphicsSupport::Confirmed,
            GraphicsSupport::Forced,
            GraphicsSupport::Unverified,
        ] {
            assert!(support.is_available(), "{support:?} should be usable");
        }
        assert!(GraphicsSupport::Confirmed.is_verified());
        assert!(!GraphicsSupport::Unverified.is_verified());
    }

    #[test]
    fn unsupported_emits_nothing_at_all() {
        let mut graphics = Graphics::new(GraphicsSupport::Unsupported);
        let mut out = Vec::new();
        let placement = [ImagePlacement::new(Rect::new(0, 0, 4, 2), AssetId::new(0))];
        let report = graphics
            .sync(&mut out, &placement, &assets(), false)
            .expect("sync");
        assert!(report.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn an_image_is_transmitted_once_and_then_only_placed() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let store = assets();
        let placement = [ImagePlacement::new(Rect::new(1, 1, 8, 3), AssetId::new(0))];

        let first = graphics
            .sync(&mut out, &placement, &store, true)
            .expect("sync");
        assert_eq!(1, first.transmitted);
        assert_eq!(1, first.placed);
        let transmits = count(&text(&out), "a=t");

        // Nothing changed: no traffic.
        out.clear();
        let second = graphics
            .sync(&mut out, &placement, &store, false)
            .expect("sync");
        assert!(second.is_empty(), "an unchanged frame must emit nothing");
        assert!(out.is_empty());

        // Hidden and shown again: placed, but never retransmitted.
        out.clear();
        graphics.sync(&mut out, &[], &store, false).expect("sync");
        assert_eq!(1, count(&text(&out), "a=d,d=p"), "the placement is removed");
        out.clear();
        graphics
            .sync(&mut out, &placement, &store, false)
            .expect("sync");
        assert_eq!(0, count(&text(&out), "a=t"), "data is reused");
        assert_eq!(1, count(&text(&out), "a=p"), "the placement returns");
        assert_eq!(1, transmits);
        assert_eq!(1, graphics.placed());
    }

    #[test]
    fn moving_a_placement_replaces_it_without_retransmitting() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let store = assets();
        graphics
            .sync(
                &mut out,
                &[ImagePlacement::new(Rect::new(0, 0, 8, 3), AssetId::new(0))],
                &store,
                true,
            )
            .expect("sync");

        out.clear();
        let report = graphics
            .sync(
                &mut out,
                &[ImagePlacement::new(Rect::new(0, 5, 8, 3), AssetId::new(0))],
                &store,
                false,
            )
            .expect("sync");
        assert_eq!(1, report.placed);
        assert_eq!(0, report.transmitted);
        assert_eq!(0, report.removed, "the same id and placement is reused");
        assert_eq!(1, count(&text(&out), "a=p"));
    }

    #[test]
    fn a_reset_retransmits_because_the_screen_cleared_the_images() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let store = assets();
        let placement = [ImagePlacement::new(Rect::new(0, 0, 8, 3), AssetId::new(0))];
        graphics
            .sync(&mut out, &placement, &store, true)
            .expect("sync");

        out.clear();
        let report = graphics
            .sync(&mut out, &placement, &store, true)
            .expect("sync");
        assert_eq!(1, count(&text(&out), "a=d,d=A"), "state is discarded");
        assert_eq!(1, report.transmitted);
        assert_eq!(1, report.placed);
    }

    #[test]
    fn clearing_removes_everything_and_can_be_repeated() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let store = assets();
        graphics
            .sync(
                &mut out,
                &[ImagePlacement::new(Rect::new(0, 0, 8, 3), AssetId::new(0))],
                &store,
                true,
            )
            .expect("sync");

        out.clear();
        graphics.clear(&mut out).expect("clear");
        assert_eq!(0, graphics.loaded());
        assert_eq!(0, graphics.placed());
        assert_eq!(1, count(&text(&out), "a=d,d=A"));
    }

    #[test]
    fn two_images_get_distinct_ids_and_placements() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let store = assets();
        let wanted = [
            ImagePlacement::new(Rect::new(0, 0, 8, 3), AssetId::new(0)),
            ImagePlacement::new(Rect::new(9, 0, 8, 3), AssetId::new(1)),
        ];
        let report = graphics
            .sync(&mut out, &wanted, &store, true)
            .expect("sync");
        assert_eq!(2, report.transmitted);
        assert_eq!(2, report.placed);
        assert_eq!(2, graphics.loaded());
        let sequence = text(&out);
        assert!(sequence.contains("i=1,"));
        assert!(sequence.contains("i=2,"));
        assert!(sequence.contains("p=1,"));
        assert!(sequence.contains("p=2,"));
    }

    #[test]
    fn a_missing_asset_is_skipped_rather_than_panicking() {
        let mut graphics = Graphics::new(GraphicsSupport::Confirmed);
        let mut out = Vec::new();
        let report = graphics
            .sync(
                &mut out,
                &[ImagePlacement::new(Rect::new(0, 0, 8, 3), AssetId::new(99))],
                &assets(),
                false,
            )
            .expect("sync");
        assert!(report.is_empty());
    }

    #[test]
    fn the_asset_used_is_the_one_requested() {
        let store = assets();
        let first: &Asset = store.get(AssetId::new(0)).expect("first");
        let second: &Asset = store.get(AssetId::new(1)).expect("second");
        assert_eq!("a", first.name());
        assert_eq!("b", second.name());
    }
}
