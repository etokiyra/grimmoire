#!/usr/bin/env python3
"""Generate the two prototype areas as rectangular ASCII maps.

# Passage width

The character sprite is **five cells wide and three tall**. A one-cell corridor
therefore has her shoulders through the wall on both sides, which is exactly what
it used to look like.

Every tile the player can stand in must therefore be **at least three cells
across** (five minus two, the most the sprite can overlap without looking wrong),
and the routes a player uses constantly are drawn wider: the chapel nave is nine
cells, its crossings five, and both stair doorways five. `widen` enforces the
three-cell floor automatically, and `MIN_CLEARANCE` records the rule for anyone
editing a map by hand.

The same rule applies to anything the player has to pass *through* - doorways,
chamber mouths, corridors, and the gaps between pillars and pews.
"""

#: The narrowest a passage may be, in cells. See the module docstring.
MIN_CLEARANCE = 3

def blank(w, h, ch='.'): return [[ch]*w for _ in range(h)]
def box(g,x0,y0,x1,y1,ch='#'):
    for x in range(x0,x1+1): g[y0][x]=ch; g[y1][x]=ch
    for y in range(y0,y1+1): g[y][x0]=ch; g[y][x1]=ch
def rect(g,x0,y0,x1,y1,ch):
    for y in range(y0,y1+1):
        for x in range(x0,x1+1): g[y][x]=ch
def hline(g,y,x0,x1,ch):
    for x in range(x0,x1+1): g[y][x]=ch
def vline(g,x,y0,y1,ch):
    for y in range(y0,y1+1): g[y][x]=ch
def niches(g,x0,y0,x1,y1,tile='#',step=5,inset=1):
    """Small alcoves set into a chamber's walls."""
    for x in range(x0+2, x1-1, step):
        g[y0+inset][x]=tile; g[y0+inset][x+1]=tile
        g[y1-inset][x]=tile; g[y1-inset][x+1]=tile

def chapel():
    W,H = 140,52
    g = blank(W,H)
    box(g,0,0,W-1,H-1)
    for x in range(8,W-8,12):
        g[0][x]=g[1][x]=g[2][x]='#'
        g[H-1][x]=g[H-2][x]=g[H-3][x]='#'
    aisle = W//2
    for y in range(3,H-2):
        for x in range(aisle-2,aisle+3): g[y][x]=','
    for y in (11, H-11): hline(g,y,3,W-4,',')
    # Colonnade.
    for y in range(6,H-7,8):
        for x in (aisle-16,aisle-13,aisle+13,aisle+16):
            for dy in range(3):
                if 0 < y+dy < H: g[y+dy][x]='#'
    # Choir: dense seating close to the apse.
    for y in range(14,26,3):
        for side in (-1,1):
            for dx in range(4,11,3):
                x = aisle + side*dx
                g[y][x]='#'; g[y][x+side]='#'
    # Nave pews.
    for y in range(28,H-15,4):
        for side in (-1,1):
            for dx in range(5,13,4):
                x = aisle + side*dx
                if g[y][x]=='.': g[y][x]='#'
    # Apse.
    box(g,aisle-12,3,aisle+12,9)
    rect(g,aisle-11,4,aisle+11,8,',')
    box(g,aisle-3,5,aisle+3,7)
    rect(g,aisle-2,6,aisle+2,6,',')
    hline(g,9,aisle-2,aisle+2,',')
    # Side chapels.
    for sx in (16, W-28):
        box(g,sx,19,sx+13,31)
        rect(g,sx+1,20,sx+12,30,'.')
        niches(g,sx,19,sx+13,31)
        g[25][sx]=g[25][sx+13]='.'
    # Vault niches along the outer walls.
    niches(g,2,2,W-3,H-3,step=7,inset=1)
    # Collapsed corners.
    for (x0,y0) in [(3,H-15),(W-10,3),(3,3),(W-10,H-15)]:
        for dy in range(7):
            for dx in range(7):
                if (dx*3+dy*5)%4!=0: g[y0+dy][x0+dx]='#'
    # A standing pool, as a solid block: scattered single cells would leave
    # one-cell gaps of floor between them, which the character cannot fit in.
    for y in range(H-9, H-5):
        for x in range(6, 13):
            g[y][x]='~'
    for (x,y) in [(14,46),(15,47),(20,44),(24,47),(W-16,45),(W-17,44),(W-22,47),(30,47),
                  (95,46),(110,45),(W-40,47),(50,46),(60,45),(80,47)]:
        g[y][x]='T'
    # The stair down: five cells wide, the width of the character.
    wide_doors(g, aisle, H-1, 5)
    return g,aisle

def crypt():
    W,H = 136,44
    g = blank(W,H)
    box(g,0,0,W-1,H-1)
    # Four chambers in a grid, joined by corridors.
    chambers = [(6,6,60,20),(74,6,129,20),(6,24,60,38),(74,24,129,38)]
    for (x0,y0,x1,y1) in chambers:
        box(g,x0,y0,x1,y1)
        rect(g,x0+1,y0+1,x1-1,y1-1,'.')
        niches(g,x0,y0,x1,y1,step=6,inset=1)
    # Doorways between the chambers, three cells tall so the character fits.
    for (x0,y0,x1,y1) in chambers:
        cy=(y0+y1)//2
        for dy in (-1, 0, 1):
            g[cy+dy][x0]='.'; g[cy+dy][x0+1]='.'
    for mx in range(66, 74):
        for dy in (-1, 0, 1):
            g[13+dy][mx]=','; g[31+dy][mx]=','
    for y in (22, 23, 24):
        for x in range(66, 74): g[y][x]=','
    # The flooded south-west chamber, drawn before the walkways so that the
    # walkways cross it as bridges.
    (sx0, sy0, sx1, sy1) = chambers[2]
    for y in range(sy0+1, sy1):
        for x in range(sx0+1, sx1):
            g[y][x] = '~'
    # Three-cell bridges over the water. A one-cell bridge would have the
    # character's shoulders over the water on both sides.
    cx, cy = (sx0+sx1)//2, (sy0+sy1)//2
    for y in range(cy-1, cy+2):
        hline(g, y, sx0+1, sx1-1, ',')
    for x in range(cx-1, cx+2):
        vline(g, x, sy0+1, sy1-1, ',')
    # Walkways in the other chambers, which stand on dry floor.
    for (x0,y0,x1,y1) in chambers:
        if (x0,y0,x1,y1) == (sx0,sy0,sx1,sy1):
            continue
        hline(g,(y0+y1)//2,x0+1,x1-1,',')
        vline(g,(x0+x1)//2,y0+1,y1-1,',')
    # The dais with the sigil, in the north-east chamber. Its mouth faces south
    # onto the chamber floor and is five cells wide, like every other doorway.
    dx,dy = 118,13
    box(g,dx-6,dy-3,dx+6,dy+3)
    rect(g,dx-5,dy-2,dx+5,dy+2,',')
    hline(g,dy+3,dx-2,dx+2,',')
    # The stair up: a doorway in the north wall with a short corridor down onto
    # the floor of the north-west chamber, so the way back is short and obvious.
    sx = 20
    wide_doors(g, sx, 0, 5)
    for y in range(1, 11):
        for x in range(sx - 1, sx + 2):
            g[y][x] = ','
    # Debris, kept off the water and the walkways.
    for i in range(60):
        x = 4 + ((i*37)%(W-8)); y = 4 + ((i*53)%(H-8))
        if g[y][x]=='.' and (x*7+y*3)%5!=0: g[y][x]='#'
    # The stair down to the catacombs, at the far end of the south-east chamber,
    # so the descent is the far side of the cellar from the way back up.
    low = 118
    for step in (-1, 0, 1):
        g[38][low + step] = '.'
    for y in range(39, H - 1):
        for step in (-1, 0, 1):
            g[y][low + step] = ','
    wide_doors(g, low, H - 1, 5)
    return g,(dx,dy),(sx,10)

def set_cell(g, x, y, ch=','):
    """Sets a cell, never touching the outer wall."""
    if 1 <= x < len(g[0]) - 1 and 1 <= y < len(g) - 1:
        g[y][x] = ch


def band(g, x0, y0, x1, y1, width=3, ch=','):
    """Carves an axis-aligned corridor `width` cells across."""
    half = width // 2
    if y0 == y1:
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for d in range(-half, half + 1):
                set_cell(g, x, y0 + d, ch)
    elif x0 == x1:
        for y in range(min(y0, y1), max(y0, y1) + 1):
            for d in range(-half, half + 1):
                set_cell(g, x0 + d, y, ch)
    else:
        raise ValueError("band must be axis-aligned")


def trail(g, waypoints, width=3, ch=','):
    """Carves a trail through axis-aligned waypoints.

    Consecutive waypoints are joined by an elbow when they differ on both axes,
    which is what makes a path wander rather than run dead straight.
    """
    for (x0, y0), (x1, y1) in zip(waypoints, waypoints[1:]):
        if y0 == y1 or x0 == x1:
            band(g, x0, y0, x1, y1, width, ch)
        else:
            band(g, x0, y0, x1, y0, width, ch)
            band(g, x1, y0, x1, y1, width, ch)


def clearing(g, cx, cy, rx, ry, ch=','):
    """Opens an elliptical clearing."""
    for y in range(cy - ry, cy + ry + 1):
        for x in range(cx - rx, cx + rx + 1):
            if ((x - cx) / float(rx)) ** 2 + ((y - cy) / float(ry)) ** 2 <= 1.0:
                set_cell(g, x, y, ch)


def scatter(g, n, ch, on=',', keep=lambda x, y: True, seed=0):
    """Scatters `ch` over cells that currently hold `on`."""
    W, H = len(g[0]), len(g)
    placed = 0
    i = 0
    while placed < n and i < n * 20:
        x = 3 + ((i * 31 + seed * 17) % (W - 6))
        y = 3 + ((i * 47 + seed * 29) % (H - 6))
        if g[y][x] == on and keep(x, y):
            g[y][x] = ch
            placed += 1
        i += 1
    return placed


def woods():
    """The Whispering Woods: a trodden trail, a hidden glade, and a trail east."""
    W, H = 128, 44
    g = blank(W, H, 'T')
    box(g, 0, 0, W - 1, H - 1)
    # The trail from the chapel in the south up to the village in the north.
    trail(g, [(64, 43), (64, 34), (52, 34), (52, 26), (70, 26), (70, 14), (62, 14), (62, 1)],
          width=5)
    # A branch west that ends in the glade: an optional path, not a through route.
    # It runs *into* the clearing rather than stopping short of it, because a
    # clearing the trail does not quite reach is an island, and an island gets
    # walled off rather than explored.
    trail(g, [(52, 34), (22, 34), (22, 30)], width=3)
    clearing(g, 18, 30, 8, 5)
    # A branch east toward the shrine.
    trail(g, [(70, 26), (96, 26), (96, 20), (126, 20)], width=3)
    # Clearings where the ways meet, so the trail opens out rather than merely
    # continuing.
    clearing(g, 64, 26, 6, 3)
    clearing(g, 70, 14, 5, 3)
    # Trees standing inside the clearings, kept off the trail itself.
    scatter(g, 30, 'T', on=',', keep=lambda x, y: (x * 5 + y * 7) % 11 == 0, seed=1)
    # Gates: south to the chapel, north to the village, east to the shrine.
    wide_doors(g, 64, H - 1, 5)
    wide_doors(g, 62, 0, 5)
    wide_doors(g, W - 1, 20, 3, horizontal=False)
    return g


def village():
    """The Hollow Village: a crossroad of ruined houses around a dead well."""
    W, H = 124, 42
    g = blank(W, H)
    box(g, 0, 0, W - 1, H - 1)
    # The crossroad. Wide, because it is the only route through.
    trail(g, [(62, 40), (62, 2)], width=5)
    trail(g, [(2, 20), (121, 20)], width=5)
    # Houses: three walls standing, a doorway onto the street, the rest fallen in.
    houses = [
        (14, 4, 40, 14), (48, 4, 74, 14), (82, 4, 110, 14),
        (14, 26, 40, 36), (48, 26, 74, 36), (82, 26, 110, 36),
    ]
    for (x0, y0, x1, y1) in houses:
        box(g, x0, y0, x1, y1)
        rect(g, x0 + 1, y0 + 1, x1 - 1, y1 - 1, '.')
        niches(g, x0, y0, x1, y1, step=5, inset=1)
        door = (x0 + x1) // 2
        wall = y1 if y0 < 20 else y0
        for dx in (-1, 0, 1):
            g[wall][door + dx] = '.'
    # Rubble heaps where the roofs came down.
    for (cx, cy) in [(26, 9), (60, 9), (96, 9), (26, 32), (60, 32), (96, 32)]:
        for dy in range(-1, 2):
            for dx in range(-1, 2):
                if (dx + dy) % 2 == 0:
                    set_cell(g, cx + dx, cy + dy, '#')
    # Gates: south to the woods, west to the shrine.
    wide_doors(g, 62, H - 1, 5)
    wide_doors(g, 0, 20, 5, horizontal=False)
    return g


def shrine():
    """The Moonwell Shrine: a pale hall, a pool at its heart, three braziers."""
    W, H = 120, 40
    g = blank(W, H)
    box(g, 0, 0, W - 1, H - 1)
    rect(g, 3, 3, W - 4, H - 4, '.')
    # The moonwell: a pool the player walks around, never across.
    for y in range(11, 30):
        for x in range(41, 72):
            if ((x - 56) / 13.0) ** 2 + ((y - 20) / 7.0) ** 2 <= 1.0:
                g[y][x] = '~'
    # A colonnade of free-standing pillars, six cells apart.
    for y in range(6, H - 5, 6):
        for x in range(10, W - 9, 6):
            if 0 <= y < H and 0 <= x < W and g[y][x] == '.':
                g[y][x] = '#'
    # Gates. The north and west ways are open; the east one is sealed until the
    # moonwell is answered, and is carved as a door so it can be seen and tried.
    wide_doors(g, 56, 0, 5)
    wide_doors(g, 0, 20, 5, horizontal=False)
    wide_doors(g, W - 1, 20, 5, horizontal=False)
    for y in range(4, 20):
        for x in range(54, 59):
            if g[y][x] == '.': g[y][x] = ','
    for y in range(20, 21):
        for x in range(2, 41):
            if g[y][x] == '.': g[y][x] = ','
    return g


def catacombs():
    """The Forgotten Catacombs: a long gallery, side passages, and a tomb."""
    W, H = 132, 44
    g = blank(W, H)
    box(g, 0, 0, W - 1, H - 1)
    # The main gallery, and four side passages off it.
    trail(g, [(2, 21), (129, 21)], width=5)
    for x in (20, 44, 96, 118):
        trail(g, [(x, 6), (x, 37)], width=3)
    # The scribe's chamber at the middle of the gallery.
    box(g, 58, 12, 84, 31)
    rect(g, 59, 13, 83, 30, ',')
    for dx in (-1, 0, 1):
        g[12][71 + dx] = ','
        g[31][71 + dx] = ','
    # Niches cut into the gallery walls: shallow, and never on the route.
    for x in range(6, W - 6, 9):
        for y in (8, 34):
            if g[y][x] == '.':
                g[y][x] = '#'
                g[y][x + 1] = '#'
        for y in (16, 26):
            if g[y][x] == '.':
                g[y][x] = '#'
    # Gates: west to the cellar, east to the shrine (sealed).
    wide_doors(g, 0, 21, 5, horizontal=False)
    wide_doors(g, W - 1, 21, 5, horizontal=False)
    return g


def run_length(g, x, y, dx, dy):
    """How many walkable cells lie in a straight line through (x, y)."""
    W, H = len(g[0]), len(g)
    run = 1
    for sign in (1, -1):
        cx, cy = x + dx*sign, y + dy*sign
        while 0 <= cx < W and 0 <= cy < H and g[cy][cx] in '.,':
            run += 1
            cx += dx*sign; cy += dy*sign
    return run


def clearance(g, x, y):
    """The narrowest cross-section through this tile, in cells."""
    return min(run_length(g, x, y, 1, 0), run_length(g, x, y, 0, 1))


def widen_axis(g, x, y, dx, dy, target):
    """Carves the walls at the ends of (x, y)'s run until it reaches target."""
    W, H = len(g[0]), len(g)
    carved = 0
    for _ in range(target * 2):
        if run_length(g, x, y, dx, dy) >= target:
            break
        for sign in (1, -1):
            if run_length(g, x, y, dx, dy) >= target:
                break
            cx, cy = x, y
            while 0 <= cx + dx*sign < W and 0 <= cy + dy*sign < H and g[cy + dy*sign][cx + dx*sign] in '.,':
                cx += dx*sign; cy += dy*sign
            nx, ny = cx + dx*sign, cy + dy*sign
            # Never touch the outer wall: that is the edge of the world.
            #
            # A tree in the way is carved out exactly like a wall is. A wood is
            # made of trees, so this is how the woods are thinned along a path
            # rather than a special case for one area: whatever stands in a
            # passage the character cannot fit through is removed.
            if 1 <= nx < W - 1 and 1 <= ny < H - 1 and g[ny][nx] in '#T':
                g[ny][nx] = ','
                carved += 1
    return carved


def widen(g, target=MIN_CLEARANCE, limit=40):
    """Widens every passage narrower than `target` cells.

    The narrow axis is the one that matters: a one-cell corridor twenty cells
    long is still one cell wide, however far it runs.

    Carving is tried first. If nothing can be carved - a tile pinched between
    water and a tree, say - the nook is filled in instead, because a gap the
    character cannot stand in is worse than no gap at all.
    """
    W, H = len(g[0]), len(g)
    for _ in range(limit):
        changed = 0
        for y in range(1, H - 1):
            for x in range(1, W - 1):
                if g[y][x] not in '.,' or clearance(g, x, y) >= target:
                    continue
                if run_length(g, x, y, 1, 0) < target:
                    changed += widen_axis(g, x, y, 1, 0, target)
                if run_length(g, x, y, 0, 1) < target:
                    changed += widen_axis(g, x, y, 0, 1, target)
        if changed:
            continue

        nooks = [(x, y) for y in range(1, H - 1) for x in range(1, W - 1)
                 if g[y][x] in '.,' and clearance(g, x, y) < target]
        if not nooks:
            break
        for (x, y) in nooks:
            water = any(g[y+dy][x+dx] == '~'
                        for dx, dy in ((1,0),(-1,0),(0,1),(0,-1)))
            g[y][x] = '~' if water else '#'
    return g


def narrowest(g):
    """The narrowest cross-section anywhere a player can stand."""
    W, H = len(g[0]), len(g)
    return min((clearance(g, x, y)
                for y in range(H) for x in range(W) if g[y][x] in '.,'),
               default=99)


def wide_doors(g, x, y, length, horizontal=True):
    """Cuts a doorway `length` cells wide, centred on (x, y)."""
    half = length // 2
    for step in range(-half, half + 1):
        if horizontal:
            g[y][x + step] = '+'
        else:
            g[y + step][x] = '+'


def reachable(g, start):
    from collections import deque
    W=len(g[0]); H=len(g)
    seen={start}; q=deque([start]); n=0
    while q:
        x,y=q.popleft(); n+=1
        for dx,dy in ((1,0),(-1,0),(0,1),(0,-1)):
            nx,ny=x+dx,y+dy
            if 0<=nx<W and 0<=ny<H and (nx,ny) not in seen and g[ny][nx] in '.,':
                seen.add((nx,ny)); q.append((nx,ny))
    walkable=sum(1 for row in g for c in row if c in '.,')
    return seen, n, walkable

def show(name,g):
    W=len(g[0]); H=len(g)
    assert all(len(r)==W for r in g), f"{name} ragged"
    print(f"// {name}: {W} x {H}")
    for row in g: print(''.join(row))
def components(g):
    from collections import deque
    W, H = len(g[0]), len(g)
    seen = [[False]*W for _ in range(H)]
    found = []
    for y in range(H):
        for x in range(W):
            if g[y][x] in '.,' and not seen[y][x]:
                q = deque([(x, y)]); seen[y][x] = True; comp = []
                while q:
                    cx, cy = q.popleft(); comp.append((cx, cy))
                    for dx, dy in ((1,0),(-1,0),(0,1),(0,-1)):
                        nx, ny = cx+dx, cy+dy
                        if 0<=nx<W and 0<=ny<H and not seen[ny][nx] and g[ny][nx] in '.,':
                            seen[ny][nx] = True; q.append((nx, ny))
                found.append(comp)
    return found

def seal_islands(g):
    """Wall off everything not in the largest walkable component."""
    comps = components(g)
    keep = set(max(comps, key=len)) if comps else set()
    for y in range(len(g)):
        for x in range(len(g[0])):
            if g[y][x] in '.,' and (x, y) not in keep:
                g[y][x] = '#'
    return keep

def degree(g, x, y, keep):
    return sum(1 for dx, dy in ((1,0),(-1,0),(0,1),(0,-1))
               if (x+dx, y+dy) in keep)

def pick(g, keep, x, y, min_degree=2, limit=60):
    """The nearest open tile that is well connected."""
    best = None
    for r in range(limit):
        for dy in range(-r, r+1):
            for dx in range(-r, r+1):
                nx, ny = x+dx, y+dy
                if (nx, ny) in keep and degree(g, nx, ny, keep) >= min_degree:
                    d = abs(dx)+abs(dy)
                    if best is None or d < best[0]:
                        best = (d, nx, ny)
        if best:
            return best[1], best[2]
    raise SystemExit(f"no suitable tile near {(x, y)}")

def carve_to(g, keep, x, y):
    """Carve a walkable path from (x, y) to the nearest connected tile."""
    W, H = len(g[0]), len(g)
    best = None
    for ty in range(H):
        for tx in range(W):
            if (tx, ty) in keep:
                d = abs(tx-x) + abs(ty-y)
                if best is None or d < best[0]:
                    best = (d, tx, ty)
    _, tx, ty = best
    cx, cy = x, y
    while cy != ty:
        cy += 1 if ty > cy else -1
        g[cy][cx] = ','; keep.add((cx, cy))
    while cx != tx:
        cx += 1 if tx > cx else -1
        g[cy][cx] = ','; keep.add((cx, cy))
    return keep

def path_length(g, start, goal):
    from collections import deque
    W, H = len(g[0]), len(g)
    q = deque([(start, 0)]); seen = {start}
    while q:
        (x, y), d = q.popleft()
        if (x, y) == goal: return d
        for dx, dy in ((1,0),(-1,0),(0,1),(0,-1)):
            n = (x+dx, y+dy)
            if 0 <= n[0] < W and 0 <= n[1] < H and n not in seen and g[n[1]][n[0]] in '.,':
                seen.add(n); q.append((n, d+1))
    return -1

def show(name, g):
    W = len(g[0]); H = len(g)
    assert all(len(r) == W for r in g), f"{name} ragged"
    print(f"// {name}: {W} x {H}")
    for row in g: print(''.join(row))

def gate(g, edge):
    """The centre of a doorway run on one edge: 'north', 'south', 'west', 'east'."""
    W, H = len(g[0]), len(g)
    if edge in ('north', 'south'):
        y = 0 if edge == 'north' else H - 1
        run = [x for x in range(W) if g[y][x] == '+']
        return (run[len(run) // 2], y) if run else (0, 0)
    x = 0 if edge == 'west' else W - 1
    run = [y for y in range(H) if g[y][x] == '+']
    return (x, run[len(run) // 2]) if run else (0, 0)


def settle(name, g, spawn_hint, marks):
    """Widens, seals, and reports one area.

    Everything a hand-edit would otherwise have to keep in step - the spawn, the
    arrival of every doorway, the piece of scenery each landmark refers to - is
    resolved here against the *finished* map, so a coordinate can never drift out
    of step with the terrain it names.
    """
    widen(g)
    keep = seal_islands(g)
    spawn = pick(g, keep, *spawn_hint)
    assert abs(spawn[0] - spawn_hint[0]) + abs(spawn[1] - spawn_hint[1]) <= 6, \
        f"{name}: the spawn hint {spawn_hint} landed at {spawn}"
    # A landmark the map does not quite reach is the failure mode this check
    # exists for: the clearing or chamber is drawn, the trail stops short, and the
    # whole thing is quietly sealed off as an island. Resolving every mark and
    # insisting it landed where it was asked for catches that at generation time.
    resolved = {}
    for label, hint in marks.items():
        at = pick(g, keep, *hint)
        assert abs(at[0] - hint[0]) + abs(at[1] - hint[1]) <= 6, \
            f"{name}: {label} asked for {hint} but landed at {at}"
        resolved[label] = at
    worst = narrowest(g)
    assert worst >= MIN_CLEARANCE, f"{name} has a {worst}-cell passage"
    walkable = sum(1 for row in g for c in row if c in '.,')
    percent = len(keep) * 100 // max(walkable, 1)
    assert percent >= 99, f"{name}: only {percent}% of the map is reachable"
    print(f"// {name} narrowest={worst} walkable={len(keep)}")
    print(f"// {name} spawn={spawn}")
    # The doorway positions, which are what a Portal has to name. A portal on the
    # wrong tile is not a door at all, so these are printed rather than guessed.
    for edge in ("north", "south", "west", "east"):
        at = gate(g, edge)
        if at != (0, 0):
            print(f"// {name} gate-{edge}={at}")
    for label, at in resolved.items():
        print(f"// {name} {label}={at} distance={path_length(g, spawn, at)}")
    show(name, g)
    print()
    return g, spawn, resolved


# -- the chapel --------------------------------------------------------------

c, aisle = chapel()
wide_doors(c, 0, 25, 5, horizontal=False)
c, c_spawn, c_marks = settle(
    "chapel", c, (aisle, 44),
    {"ysolde": (56, 33), "basin": (76, 6), "south-stair": (aisle, 50),
     "west-door": (5, 25)})

# -- the cellar --------------------------------------------------------------

d, dais, stair = crypt()
d, d_spawn, d_marks = settle(
    "cellar", d, (20, 12),
    {"sigil": (dais[0], dais[1]), "up-stair": (20, 1), "down-stair": (118, 40)})

# -- the woods ---------------------------------------------------------------

w = woods()
w, w_spawn, w_marks = settle(
    "woods", w, (64, 40),
    {"chapel-gate": (64, 41), "village-gate": (62, 3), "shrine-gate": (124, 20),
     "glade": (18, 30), "cairn": (52, 26), "rin": (62, 20)})

# -- the village -------------------------------------------------------------

v = village()
v, v_spawn, v_marks = settle(
    "village", v, (62, 38),
    {"woods-gate": (62, 39), "shrine-gate": (3, 20), "well": (62, 20),
     "letters": (20, 10), "cassen": (40, 20)})

# -- the shrine --------------------------------------------------------------

s = shrine()
s, s_spawn, s_marks = settle(
    "shrine", s, (56, 5),
    {"village-gate": (56, 3), "woods-gate": (3, 20), "catacomb-gate": (115, 20),
     "dial": (40, 20), "ioreth": (30, 14),
     "brazier-west": (78, 16), "brazier-north": (78, 20), "brazier-east": (78, 24)})

# -- the catacombs -----------------------------------------------------------

k = catacombs()
k, k_spawn, k_marks = settle(
    "catacombs", k, (8, 21),
    {"cellar-gate": (3, 21), "shrine-gate": (128, 21), "scribe": (71, 22),
     "vesper": (52, 21), "niche": (20, 30)})
