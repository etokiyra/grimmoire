# Gameplay: the exploration prototype

This document describes the playable exploration slice: its world model, the
rules that govern it, the controls, and how progress is saved. It documents what
exists today, which is deliberately small.

> **Placeholder content.** The area names, the character called Ysolde, and every
> line of dialogue are provisional. The story, setting, and characters have not been
> decided and nothing here is canon.

## What you can do

Launch Grimmoire and you are standing in the **Ruined Chapel**. You can walk
around it, examine a cracked basin, speak to a hooded figure, and take the stair
down to the **Sunken Cellar** — and come back up again. That is the whole slice.

## Controls

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move (and face that way). Hold to keep walking. |
| `Shift` + a direction | Sprint |
| `e` | Interact with whatever you are facing, or whatever is adjacent |
| `h`, `?`, `F1` | Help and renderer diagnostics |
| `F3` | The presentation study (a rendering prototype) |
| `F2` | The renderer showcase (a development screen) |
| `Esc` | Quit |
| `q`, `Ctrl`+`C` | Quit |

The world is drawn across the **whole screen**, and the camera follows you: the
two areas are larger than a normal terminal, so walking scrolls the view.
Grimmoire draws its own background, so the terminal's theme never shows through.

The bottom two rows are an overlay, not part of the world. The upper row shows a
prompt for whatever is within reach, or the result of your last action, and says
`sprinting »` while a sprint is in progress. The lower row shows the area you are
in, your position, the direction you face, and the essential controls.

The world itself is **ASCII art**. Walls are drawn with a lit cap, a shadowed
base, a side face, or as a pillar depending on what surrounds them; floors, paths,
rubble and moss vary by position; water rolls; candles gutter along the walls;
motes drift; and each area carries an inlaid motif cut into its open floors.

The player and the people they meet are small **figures**, not single letters:
several cells tall, drawn in several colours at once (hair, skin, cloth, trim and
eyes), with four facing directions, a walk cycle that keeps pace with movement, a
sprint of their own, and an idle breath that blinks now and then.

Play needs a viewport of at least 80×24. Below that the game suspends to a short
explanation, ignores input, keeps all of its state, and resumes by itself when the
window grows.

## The world model

The world is a set of **areas**. An area is a bounded grid of tiles plus a list
of things placed on it. There is no entity-component system, no scripting, and no
scene graph: the slice does not need them, and adding them now would be
speculative.

- A **tile** is `Floor`, `Path`, `Wall`, `Water`, `Tree`, or `Door`. Only floor
  and path can be walked on. A `Door` blocks movement but can be used.
- An **entity** is an NPC (which speaks) or an object (which can be examined).
  Entities block movement, so you cannot walk through the figure or the basin.
- A **portal** is a doorway: a position, a destination area, an arrival position,
  and a label. Every doorway is drawn as a door tile and can be used from either
  side.
- The **player** has a position, an area, and a facing direction.

Areas are authored as fixed-width ASCII rows, which keeps the content readable
and lets the layout be reviewed like the game it produces:

```
#################+################
#...............,,,..............#
#...............######,,,#####...#
```

`#` is wall, `.` floor, `,` path, `~` water, `T` tree, and `+` a door. Rows are
padded or truncated to the first row's width, so a ragged row can never produce
an out-of-bounds grid.

## Rules

- **Movement.** Pressing a direction always turns you that way, and steps if the
  destination is inside the area, is walkable, and holds no entity. Turning
  without moving is what lets you face a wall or a doorway.
- **Collision.** Terrain, area boundaries, and entities all block movement.
  Out-of-bounds coordinates read as wall, so boundaries need no special case.
- **Interaction.** `e` resolves the adjacent target, preferring the tile you
  face and otherwise scanning north, east, south, west in that fixed order, so
  the choice is deterministic when several things are within reach.
  - An object produces a message in the feedback line.
  - An NPC opens a dialogue panel.
  - A doorway moves you to the other area, which is saved immediately.
  - Nothing within reach produces a message saying so. This is not an error.
- **Dialogue.** The panel is a screen pushed on top of the game, so input goes
  to it and you cannot walk while talking. Its dismissal plays out on its own
  before the panel is removed, which is why a screen can finish something without
  another keypress. It is an overlay: the world stays
  visible around it, and a small ASCII bust of the speaker is drawn beside their
  words. The panel rises into place when it opens and slides away when it closes.
  It closes on `Enter`, `Space`, or
  `Esc` — deliberately *not* on `e`, the key that opens it, because a terminal
  that reports auto-repeats as ordinary presses would otherwise close it the
  instant it appeared.
- **Movement and repeat.** Movement is **event-driven**, not timer-driven. A press
  steps immediately, with no delay. While a direction is held, the terminal's
  auto-repeats step again, but no faster than the cadence, so holding cannot
  sprint and the pace is Grimmoire's rather than the terminal's. Releasing stops at
  once. Because steps come only from key events, a tap is always exactly one step,
  and no lost or missing event can leave you walking on your own.
- **Both axes move at the same screen speed.** Terminal cells are taller than they
  are wide (about 9 by 20 pixels), so a step north crosses more than twice the
  screen distance of a step east. The two axes are therefore paced from a single
  speed — **150 pixels a second**, doubling while sprinting — rather than from one
  step rate. North and south step every ~133 ms, east and west every ~60 ms, and
  the cell size comes from the terminal, so this adapts to the font.
- **Sprint.** Hold `Shift` while moving (or send an upper-case movement key) and
  the pace doubles, the pose changes, and a short wake trails behind. A sprint is
  not a different kind of movement: it steps, collides, and stops exactly like a
  walk.
- **Feedback.** A step that is refused pulses and underlines the tile that refused
  it; an interaction flashes whatever answered; arriving in a new area marks the
  spot. Nothing changes the glyph under a pulse, so feedback never hides a
  character or an exit.
- **Releases.** Grimmoire asks Kitty for the keyboard protocol's event types, so it
  sees press, repeat, and release separately. Releasing a key stops the walk
  immediately. If a release is ever missed, the walk stops anyway once events stop
  arriving.
- **Discrete actions.** `e`, help, and screen changes fire only on a genuine press:
  releases are not activations and auto-repeat is ignored, so holding `e` cannot
  interact over and over.

## Persistence

Progress is saved automatically: on quitting, and immediately after travelling
to another area.

The save file lives at `$XDG_DATA_HOME/grimmoire/save.txt`, or
`$HOME/.local/share/grimmoire/save.txt` when `XDG_DATA_HOME` is not set. If
neither is available, persistence is skipped and the game simply starts fresh.

Only essential exploration state is stored — the current area, the player's
position, and the areas visited:

```
grimmoire-save 1
area=sunken-cellar
pos=17 1
visited=ruined-chapel sunken-cellar
```

The format is versioned and parsed strictly. Anything unexpected — a different
version, a missing field, a malformed number, an unknown key, a position that is
not valid standing ground, or an area that no longer exists — causes the whole
save to be ignored and a new game to start. Saving writes to a temporary file and
renames it over the target, so an interrupted write cannot corrupt an existing
save.

The save format is intentionally narrow. Equipment, quests, and story flags will
need a different design, and inventing one now would be premature.

## Where the code lives

| Path | Responsibility |
| --- | --- |
| `src/game/area.rs` | Tiles, positions, directions, doorways, and the ASCII area parser |
| `src/game/entity.rs` | NPCs and objects |
| `src/game/dialogue.rs` | The minimal dialogue content model |
| `src/game/world.rs` | The player's place in the world: movement, collision, interaction, travel, snapshots |
| `src/game/areas.rs` | The two prototype areas (temporary content) |
| `src/save.rs` | The versioned save format and the file store |
| `src/ui/motion.rs` | The movement model: press, cadence, release, sprint, stall guard |
| `src/ui/scenery.rs` | The ASCII world renderer: terrain glyphs, inlay, props, ambience, figures |
| `src/ui/animation.rs` | The character animation model: stride, sprint, idle, blink |
| `src/ui/effects.rs` | Transient feedback pulses |
| `src/ui/exploration.rs` | The exploration screen: key mapping, camera, and the HUD overlay |
| `src/ui/dialogue.rs` | The dialogue overlay and its ASCII portrait |

The `game` and `save` modules never draw and never touch a terminal. The UI maps
tiles and entity kinds onto glyphs and colours; the game model never decides how
anything looks.

## Adding an area

1. Author the rows (all the same width) and add a constructor in
   `src/game/areas.rs` with a stable `AreaId`.
2. Declare its entities and portals. Give each portal an `arrive` position on
   walkable ground.
3. Add it to `areas::all()` and point an existing portal at it.

`tools/generate-areas.py` draws the two areas and prints them; it seals off any
pocket that is not reachable from the main space, so a map cannot contain a region
the player can see but never enter. `cargo run --example preview -- chapel` prints
a rendered area, which is the quickest way to check how a layout will look.

The tests in `areas.rs` check that every spawn and arrival is on walkable ground,
that every portal sits on a door tile and is reachable from both sides, that every
entity can be walked up to, that most of each area is one connected space, that
each area is at least as large as the reference viewport, and that entity glyphs
are printable ASCII — so most authoring mistakes are caught without launching the
game.

## Not implemented

No combat, spells, inventory, quests, skill trees, or story. Items cannot be
picked up. There are two hand-authored areas and exactly one NPC, with no dialogue
branching. Save data covers position and discovery only.
