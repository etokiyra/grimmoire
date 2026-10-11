#!/usr/bin/env python3
"""Generate the two prototype areas as rectangular ASCII maps."""

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
    for (x,y) in [(7,H-8),(8,H-8),(9,H-8),(7,H-7),(8,H-7),(10,H-9),(11,H-8),(6,H-6)]:
        g[y][x]='~'
    for (x,y) in [(14,46),(15,47),(20,44),(24,47),(W-16,45),(W-17,44),(W-22,47),(30,47),
                  (95,46),(110,45),(W-40,47),(50,46),(60,45),(80,47)]:
        g[y][x]='T'
    g[H-1][aisle]='+'
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
    # Doorways between the chambers.
    for (x0,y0,x1,y1) in chambers:
        cy=(y0+y1)//2
        g[cy][x0]='.'; g[cy][x0+1]='.'
    for (mx) in (66,67,68,69,70,71,72,73):
        g[13][mx]=','; g[31][mx]=','
    for y in (22,23):
        for x in (66,67,68,69,70,71,72,73): g[y][x]=','
    # The flooded south-west chamber, drawn before the walkways so that the
    # walkways cross it as bridges.
    for y in range(25,38):
        for x in range(7,60):
            if (x*3+y*5)%11!=0: g[y][x]='~'
    # Walkways inside the chambers: laid over the water as bridges.
    for (x0,y0,x1,y1) in chambers:
        hline(g,(y0+y1)//2,x0+1,x1-1,',')
        vline(g,(x0+x1)//2,y0+1,y1-1,',')
    # The dais with the sigil, in the north-east chamber.
    dx,dy = 118,13
    box(g,dx-6,dy-3,dx+6,dy+3)
    rect(g,dx-5,dy-2,dx+5,dy+2,',')
    hline(g,dy+3,dx-2,dx+2,',')
    # The stair up: a doorway in the north wall with a short corridor down onto
    # the floor of the north-west chamber, so the way back is short and obvious.
    sx = 20
    g[0][sx] = '+'
    for y in range(1, 9):
        g[y][sx] = ','
    for y in (9, 10):
        for x in range(sx - 1, sx + 2):
            g[y][x] = ','
    # Debris, kept off the water and the walkways.
    for i in range(60):
        x = 4 + ((i*37)%(W-8)); y = 4 + ((i*53)%(H-8))
        if g[y][x]=='.' and (x*7+y*3)%5!=0: g[y][x]='#'
    return g,(dx,dy),(sx,10)

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

c, a = chapel()
keep_c = seal_islands(c)
keep_c = carve_to(c, keep_c, a, 51)
chapel_spawn = pick(c, keep_c, a, 44)
chapel_npc = pick(c, keep_c, 56, 33)
chapel_basin = pick(c, keep_c, 76, 6)
chapel_arrive = pick(c, keep_c, a, 49)
print(f"// chapel keep={len(keep_c)} walkable=", sum(1 for r in c for ch in r if ch in '.,'))
print(f"// chapel spawn={chapel_spawn} npc={chapel_npc} basin={chapel_basin} arrive={chapel_arrive}")
show("chapel", c); print()

d, dais, stair = crypt()
keep_d = seal_islands(d)
door = next((x for x in range(len(d[0])) if d[0][x] == '+'), 0)
keep_d = carve_to(d, keep_d, door, 0)
cellar_spawn = pick(d, keep_d, door, 12)
cellar_sigil = pick(d, keep_d, dais[0], dais[1])
cellar_arrive = pick(d, keep_d, door, 12)
print(f"// cellar keep={len(keep_d)} walkable=", sum(1 for r in d for ch in r if ch in '.,'))
print(f"// cellar door={door} spawn={cellar_spawn} sigil={cellar_sigil} arrive={cellar_arrive} "
      f"door->spawn={path_length(d, (door, 0), cellar_spawn)} spawn->sigil={path_length(d, cellar_spawn, cellar_sigil)}")
print(f"// chapel spawn->npc={path_length(c, chapel_spawn, chapel_npc)} spawn->arrive={path_length(c, chapel_spawn, chapel_arrive)} spawn->basin={path_length(c, chapel_spawn, chapel_basin)}")
show("cellar", d)
