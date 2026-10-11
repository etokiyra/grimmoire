# Gameplay: the exploration prototype

This document describes the playable exploration slice: its world model, the
rules that govern it, the controls, and how progress is saved. It documents what
exists today, which is deliberately small.

> **Placeholder content.** The area names, the Hooded Figure, and every line of
> dialogue are provisional. The story, setting, and characters have not been
> decided and nothing here is canon.

## What you can do

Launch Grimmoire and you are standing in the **Ruined Chapel**. You can walk
around it, examine a cracked basin, speak to a hooded figure, and take the stair
down to the **Sunken Cellar** — and come back up again. That is the whole slice.

## Controls

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move one tile (and face that way) |
| `e` | Interact with whatever you are facing, or whatever is adjacent |
| `h`, `?`, `F1` | Help and renderer diagnostics |
| `F2` | The renderer showcase (a development screen) |
| `Esc` | Quit |
| `q`, `Ctrl`+`C` | Quit |

The bottom of the screen always shows your position, the direction you face, and
the essential controls. The line above it shows the result of your last action,
or a prompt for whatever is within reach.

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
  to it and you cannot walk while talking. It closes on `Enter`, `Space`, or
  `Esc` — deliberately *not* on `e`, the key that opens it, because a terminal
  that reports auto-repeats as ordinary presses would otherwise close it the
  instant it appeared.
- **Auto-repeat.** Movement is paced by a fixed step interval, so held keys and
  rapid taps walk at the same predictable speed on every terminal. Discrete
  actions (`e`, help, the showcase) ignore repeats entirely.

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
| `src/ui/exploration.rs` | The exploration screen: key mapping, camera, and drawing |
| `src/ui/dialogue.rs` | The dialogue panel |

The `game` and `save` modules never draw and never touch a terminal. The UI maps
tiles and entity kinds onto glyphs and colours; the game model never decides how
anything looks.

## Adding an area

1. Author the rows (all the same width) and add a constructor in
   `src/game/areas.rs` with a stable `AreaId`.
2. Declare its entities and portals. Give each portal an `arrive` position on
   walkable ground.
3. Add it to `areas::all()` and point an existing portal at it.

The tests in `areas.rs` check that every spawn and arrival is on walkable ground,
that every portal sits on a door tile, and that entity glyphs are printable
ASCII, so most authoring mistakes are caught without launching the game.

## Not implemented

No combat, spells, inventory, quests, skill trees, or story. Items cannot be
picked up. Areas are hand-authored and tiny. There is exactly one NPC. Save data
covers position and discovery only.
