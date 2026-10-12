# Gameplay: the exploration RPG

This document describes the playable region: its world model, the rules that
govern it, the controls, and how progress is saved. It documents what exists
today, which is deliberately small.

> **Placeholder content.** The area names, the character called Ysolde, and every
> line of dialogue are provisional. The story, setting, and characters have not been
> decided and nothing here is canon.

## What you can do

Launch Grimmoire and you are standing in the **Ruined Chapel**. From there you can
walk out into the **Whispering Woods**, north to the **Hollow Village**, east to
the **Moonwell Shrine**, or down the stair to the **Sunken Cellar** and on into
the **Forgotten Catacombs**. Along the way you can examine what is worth
examining, talk to five characters, read the **journal** of what you have found,
and answer the moonwell.

Every place is reachable without solving anything. The moonwell opens a shortcut
between the shrine and the catacombs; it does not gate a place.

## Controls

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move (and face that way). Hold to keep walking. |
| `Shift` + a direction | Sprint |
| `e` | Interact with whatever you are facing, or whatever is adjacent |
| `j` | The journal of what you have found |
| `h`, `?`, `F1` | Help and renderer diagnostics |
| `F3` | The presentation study (a rendering prototype) |
| `F2` | The renderer showcase (a development screen) |
| `Esc` | Quit |
| `q`, `Ctrl`+`C` | Quit |

The world is drawn across the **whole screen**, and the camera follows you: the
areas are larger than a normal terminal, so walking scrolls the view.
Grimmoire draws its own background, so the terminal's theme never shows through.

The bottom two rows are an overlay, not part of the world. The upper row shows a
prompt for whatever is within reach, or the result of your last action, and says
`sprinting »` while a sprint is in progress. The lower row shows the area you are
in, your position, the direction you face, and the essential controls.

The world itself is **ASCII art**. Walls are drawn with a lit cap, a shadowed
base, a side face, or as a pillar depending on what surrounds them; floors, paths
and water vary by position; water rolls; candles gutter along the walls; and each
area carries an inlaid motif cut into its open floors.

Each area also has its own **material** and its own **air**, which is what makes
the wood, the village, the shrine and the catacombs recognisable before their
names are read: grass and pollen in the wood, ash and cinders in the village, pale
marble under cold silver motes in the shrine, bone dust in the catacombs.

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
- An **entity** is one of three things. An **NPC** speaks, from a fixed facing,
  with a *figure* naming the silhouette the interface should draw. An **object**
  is scenery that can be examined, with a *prop* naming its art and optionally a
  **discovery** it teaches. A **brazier** is the shrine's one stateful object.
  Entities block movement, so you cannot walk through a character or a well.
- A **portal** is a doorway: a position, a destination area, an arrival position,
  a label, and optionally the discovery that unseals it. Every doorway is drawn as
  a door tile and can be used from either side; a sealed one is drawn, reachable
  and interactive, and refuses with a message until the world state opens it.
- The **player** has a position, an area, and a facing direction.
- **Progression** is a set of discoveries, the journal that records them in order,
  and the braziers lit at the moonwell. It is small on purpose: it is the state a
  later inventory, spellbook or quest log would sit *beside*, not replace.

Areas are authored as fixed-width ASCII rows, which keeps the content readable
and lets the layout be reviewed like the game it produces:

```
#################+++++###################
#.................,,,,,..................#
#.................#####,,,,,#####........#
```

`#` is wall, `.` floor, `,` path, `~` water, `T` tree, and `+` a door. Rows are
padded or truncated to the first row's width, so a ragged row can never produce
an out-of-bounds grid.

Note the widths: the doorway is five cells and the nave is five, because the
character sprite is five cells wide. **Nothing the player walks through may be
narrower than three cells** — see [`map-design.md`](map-design.md).

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

## Progression and discoveries

A **discovery** is a stable, saveable id with its content attached: a title and a
journal entry (`game::discovery`). Recording one is what "finding something"
means, and it happens when the player examines a relic or answers the moonwell —
never because an area was loaded.

Everything a discovery unlocks reads from the same set:

- **Journal entries** (`j`), in the order they were found.
- **Dialogue variants**: a character's `Speech` is a default plus a list of
  variants, each unlocked by a discovery. The first matching variant wins, so the
  list reads most-specific-first and resolution is deterministic however the
  player got there.
- **Sealed doorways**, which open when the discovery they name is known.

The **moonwell** is the one puzzle. Three braziers, each marked with a phase of the
moon, and a dial cut into the floor listing waning, full, waxing — in that order.
Lighting them in that order answers the moon; lighting one out of turn puts *all*
of them out, which is the feedback that says the order mattered. A partial success
that stayed lit would let the player brute-force it, and resetting means it can
never soft-lock. The glow is persistent: lit braziers stay lit across a save.

## Persistence

Progress is saved automatically: on quitting, and immediately after travelling
to another area.

The save file lives at `$XDG_DATA_HOME/grimmoire/save.txt`, or
`$HOME/.local/share/grimmoire/save.txt` when `XDG_DATA_HOME` is not set. If
neither is available, persistence is skipped and the game simply starts fresh.

Only state that cannot be reconstructed is stored — the current area, the player's
position, the areas visited, the journal, and the braziers lit:

```
grimmoire-save 2
area=sunken-cellar
pos=20 12
visited=ruined-chapel sunken-cellar
journal=chapel-basin village-well
braziers=waning
```

The format is versioned and parsed strictly. Anything unexpected — an unknown
version, a missing field, a malformed number, an unknown key, a *field the file's
own version does not define*, a position that is not valid standing ground, or an
area that no longer exists — causes the whole save to be ignored and a new game to
start. Saving writes to a temporary file and renames it over the target, so an
interrupted write cannot corrupt an existing save.

### Versions and migration

| Version | Fields | Read by |
| --- | --- | --- |
| 1 | `area`, `pos`, `visited` | every version |
| 2 | the above, plus `journal` and `braziers` | 2 and later |

Files are always *written* at the current version and older files are still
*read*. A version 1 save loads with an empty journal and no braziers lit, which is
exactly the state that player was in when it was written: migration is a widening
rather than a conversion, nothing is reinterpreted, and **no valid save is ever
discarded because the schema grew**.

Progression ids this build does not recognise are dropped *individually* — a
discovery that was renamed or removed costs only itself, and the rest of the
journal and the player's position survive. That is the same rule the visited set
already used.

A version 1 file that contains a version 2 field is rejected: it is not a file the
game ever wrote, and guessing what it meant is worse than starting fresh.

The format stays intentionally narrow. Equipment, quests, and story flags will
need more design, and inventing it now would be premature.

## Where the code lives

| Path | Responsibility |
| --- | --- |
| `src/game/area.rs` | Tiles, positions, directions, doorways, and the ASCII area parser |
| `src/game/entity.rs` | NPCs, props and braziers; the figures and props they name |
| `src/game/dialogue.rs` | Dialogue, and the speech variants a discovery unlocks |
| `src/game/discovery.rs` | What can be learned, and the journal text for each |
| `src/game/progress.rs` | What has been learned, and the moonwell's state |
| `src/game/world.rs` | The player's place in the world: movement, collision, interaction, travel, progression, snapshots |
| `src/game/areas.rs` | The six areas, the cast, and every landmark (content) |
| `src/save.rs` | The versioned save format and the file store |
| `src/ui/motion.rs` | The movement model: press, cadence, release, sprint, stall guard |
| `src/ui/scenery.rs` | The ASCII world renderer: terrain glyphs, ground, inlay, props, ambience, figures |
| `src/ui/cast.rs` | The cast table: each figure's art, inks and dialogue bust |
| `src/ui/animation.rs` | The character animation model: stride, sprint, idle, blink |
| `src/ui/effects.rs` | Transient feedback pulses |
| `src/ui/exploration.rs` | The exploration screen: key mapping, camera, and the HUD overlay |
| `src/ui/dialogue.rs` | The dialogue overlay and the speaker's bust |
| `src/ui/journal.rs` | The journal panel |

The `game` and `save` modules never draw and never touch a terminal. The UI maps
tiles and entity kinds onto glyphs and colours; the game model never decides how
anything looks.

## Adding an area

1. Draw it in `tools/generate-areas.py` and run `tools/apply-areas.py`, which
   writes the grid into `src/game/areas.rs` and prints the coordinates.
2. Add a constructor with a stable `AreaId`, and declare its entities and portals
   using the printed coordinates. Give each portal an `arrive` position on
   walkable ground, and every area at least one landmark worth examining.
3. Add it to `areas::all()` and point an existing portal at it — both ways, since
   `every_doorway_runs_both_ways` checks.
4. Give it a `Ground`, an `Air` and a `Motif` in `ui::scenery`, or it will look
   like the chapel with different walls.

**Every passage must fit the character.** The sprite is five cells wide, so no
tile the player can stand in may be narrower than three cells, and the routes
walked constantly are drawn wider still. This covers anything passed *through*:
corridors, chamber mouths, the gaps between pillars and pews, and the approach to a
doorway. Read [`map-design.md`](map-design.md) before drawing a map — it lists the
rule, the widths in use, and the other rules a layout already has to satisfy.

`tools/generate-areas.py` draws the maps; `tools/apply-areas.py` writes them into
`areas.rs`. The generator widens a passage under the minimum, seals off any pocket
that is not reachable from the main space — so a map cannot contain a region the
player can see but never enter — and refuses to emit a map that is still too
narrow. `cargo run --example preview -- woods` prints a rendered area, which is the
quickest way to check how a layout will look.

The tests in `areas.rs` check that every spawn and arrival is on walkable ground,
that every portal sits on a door tile and is reachable from both sides, that every
doorway is a full character wide and runs both ways, that no walkable tile is in a
passage the character does not fit through, that every entity can be walked up to
and every area has something to examine, that the whole region is reachable from
the chapel, that every discovery can actually be made, that most of each area is
one connected space, and that each area is at least as large as the reference
viewport — so most authoring mistakes are caught without launching the game.

## Not implemented

No combat, spells, inventory, quests, skill trees, or story. Items cannot be
picked up. Dialogue branches only on what has been discovered. There is one puzzle
and five characters, and the content is placeholder throughout.
