# Map design

The rules a Grimmoire area has to satisfy, and why. Read this before drawing or
editing a map.

## 1. Every passage fits the character

**No tile the player can stand in may be narrower than three cells.**

The character sprite is **five cells wide and three tall**. A one-cell corridor
therefore has her shoulders through the wall on both sides, and a two-cell one
still clips. Three is the sprite width minus two cells of acceptable overlap.

```
 one cell        two cells       three cells     five cells
 #   #           ##   #          ##   ##         ##   ##
 # @ #     ->    # @  #    ->    #  @  #   ->    #  @  #
 #   #           ##   #          ##   ##         ##   ##
 clipping        clipping        acceptable      room to spare
```

The rule covers **anything the player passes through**: corridors, chamber
mouths, the gaps between pillars and between pews, and the approach to a doorway.

### What is drawn wider

The minimum is a floor, not a target. Routes a player uses constantly are drawn
to the character's **full width of five**:

| Route | Width |
| --- | --- |
| The chapel nave | 9 cells |
| The chapel crossings | 5 cells tall, edge to edge |
| Both stair doorways | 5 cells |
| The cellar's connecting corridors | 3 cells tall |
| The bridges over the flooded chamber | 3 cells |

### How it is enforced

Two places, so a hand edit cannot quietly regress it:

- `tools/generate-areas.py` widens any passage under the minimum after drawing a
  map, and refuses to emit one that is still too narrow (`widen`, `narrowest`,
  `MIN_CLEARANCE`). A *tree* in the way is carved out exactly like a wall is: a
  wood is made of trees, so that is how its paths are thinned rather than a
  special case for one area. Where a nook is pinched between water and a tree and
  cannot be widened, it is filled in instead — a gap the character cannot stand in
  is worse than no gap.
- `src/game/areas.rs` asserts it for every area on every test run
  (`every_passage_fits_the_character`, `every_doorway_is_a_full_character_wide`,
  `the_main_routes_are_wider_than_the_minimum`), using the same
  `MIN_PASSAGE` constant the rest of the code reads.

If you change the character sprite's size, change `MIN_PASSAGE` with it. The
constant is the *sprite width minus two*, not a number chosen for its own sake.

## 2. The other rules a map already has to satisfy

These were true before and stay true:

- **Every walkable tile is reachable** from the area's spawn. The generator seals
  off any pocket that is not, so a map cannot contain a region the player can see
  but never enter.
- **A place the map reaches is a place the trail reaches.** This is the rule that
  a cleared glade with its path stopping three cells short breaks: the clearing is
  drawn, the trail comes near, and `seal_islands` quietly walls the whole thing
  off. The generator resolves every named landmark against the *finished* map and
  refuses to continue if one landed more than a few cells from where it was asked
  for, which is what catches that class of mistake at generation time rather than
  in play.
- **Doorways sit on door tiles, and are reachable from both sides.** A portal's
  `arrive` position is on walkable ground in the destination. The generator prints
  every doorway's coordinates for exactly this reason: a portal on the wrong tile
  is not a door at all.
- **Every doorway runs both ways**, and the whole region is reachable from where
  the game starts, without solving anything.
- **Spawns, arrivals, entities and objects are on walkable ground**, and every
  entity can be walked up to from at least one neighbouring tile.
- **Areas are larger than the reference viewport** (120×40), so the camera
  scrolls rather than centring a small island.
- **Ragged rows are padded, never wrapped.** Areas are authored as fixed-width
  ASCII rows; a short row is padded with wall.

## 3. Drawing a map

```sh
tools/generate-areas.py          # print the maps, with every coordinate
tools/apply-areas.py             # write the maps into src/game/areas.rs
```

The maps are **generated, not hand-edited**. `tools/apply-areas.py` runs the
generator and writes each grid into its constructor in `src/game/areas.rs`; it is
idempotent, so it can be re-run at any time.

Everything *around* the grid — the cast, the props, the doorways, the spawns and
the landmarks — is authored by hand in `areas.rs`. The generator prints the
coordinates to paste into that part (until it, too, is worth generating):

```
// woods spawn=(64, 40)
// woods gate-south=(64, 43)
// woods glade=(18, 30) distance=56
...
```

To look at the result rather than read it:

```sh
cargo run --release --example preview -- chapel     # the chapel as ASCII
cargo run --release --example preview -- woods      # any word of an area's id
cargo run --release --example preview -- figures    # the whole cast, every way
```

## 4. Making the world look like a place

Beyond the rules, the two things that make an area read as architecture rather
than as graph paper are in the renderer, not the map:

- **Neighbour-aware walls.** A wall's glyph depends on what surrounds it — a lit
  cap where floor is above, a shadowed base where floor is below, a side face, a
  pillar when it stands alone. An isolated 1-cell wall is drawn as a *column*, so
  a colonnade is a row of lone `#` cells rather than a wall with holes in it.
- **Inlay.** `ui::scenery` cuts a motif into each area's open floor. If you give
  an area a large empty room, give it something to look at.

Both are described in [`renderer-design.md`](renderer-design.md).

### Give the place a material, an air and a name

Beyond the rules, an area should be recognisable **before its name is read**. Each
one carries three things in `ui::scenery`, and they are the cheapest identity the
game has:

| Knob | What it does |
| --- | --- |
| `Ground` | The glyphs, ink and grit of the open floor. Stone, damp, grass, ash, marble, bone. |
| `Air` | What drifts: dust, pollen and blossom, ash and cinders, silver motes, bone dust. |
| `Motif` | The inlay cut into its open floors. |

A new area wants all three. A wood floored in stone and breathing dust is a room
with trees in it.
