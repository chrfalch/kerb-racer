# Tileable ground textures for Kerb Racers, painted with numpy inside Blender:
#   blender -b -P art/src/textures.py
# Writes art/textures/{asphalt,grass,clay,dirt,gravel,scree}.png (sRGB, 512², seamless).
import bpy, numpy as np, os
OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "textures")
N = 512
rng = np.random.default_rng(7)

def value_noise(cells, seed):
    g = np.random.default_rng(seed).random((cells, cells))
    t = np.arange(N) * cells / N
    i0 = np.floor(t).astype(int); f = t - i0; i1 = (i0 + 1) % cells
    f = f * f * (3 - 2 * f)
    a = g[np.ix_(i0, i0)]; b = g[np.ix_(i0, i1)]; c = g[np.ix_(i1, i0)]; d = g[np.ix_(i1, i1)]
    fy = f[:, None]; fx = f[None, :]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy

def fbm(base, octaves, seed, gain=0.5):
    total = np.zeros((N, N)); amp = 1.0; norm = 0
    for o in range(octaves):
        total += amp * value_noise(base * 2 ** o, seed + o); norm += amp; amp *= gain
    return total / norm

def blur(img, r):
    out = img.copy()
    for axis in (0, 1):
        acc = np.zeros_like(out)
        for k in range(-r, r + 1):
            acc += np.roll(out, k, axis=axis)
        out = acc / (2 * r + 1)
    return out

def save(name, rgb):
    rgb = np.clip(rgb, 0, 1)
    img = bpy.data.images.new(name, N, N, alpha=False)
    px = np.ones((N, N, 4), dtype=np.float32)
    # Blender stores rows bottom-up; a byte image keeps these sRGB values as written.
    px[..., :3] = rgb[::-1]
    img.pixels.foreach_set(px.ravel())
    img.filepath_raw = os.path.join(OUT, name + ".png")
    img.file_format = "PNG"
    img.save()

# Asphalt: a darker, warm grey aggregate, fine speckle, soft patches and a few tar seams.
broad = fbm(4, 4, 11)
fine = rng.random((N, N))
speck = (fine > 0.985) * 0.12 - (fine < 0.02) * 0.08
grain = blur(rng.random((N, N)), 1) - 0.5
v = 0.30 + 0.05 * (broad - 0.5) + 0.045 * grain + speck
seams = np.abs(fbm(2, 3, 21) - 0.5) < 0.002
v = np.where(seams, v * 0.88, v)
save("asphalt", np.stack([v * 1.03, v * 1.0, v * 0.97], -1))

# Grass: a smooth, mown meadow: broad soft clumps, only a whisper of blade noise.
clumps = fbm(3, 4, 31)
blades = blur(rng.random((N, N)), 2)
dry = np.clip((fbm(3, 3, 41) - 0.72) * 2, 0, 1)
g = 0.56 + 0.16 * (clumps - 0.5) + 0.08 * (blades - 0.5)
# A soft sage rather than a saturated lawn green.
r = 0.38 * g + 0.12 * dry
gg = 0.58 * g + 0.06 * dry
b = 0.24 * g + 0.02 * dry
save("grass", np.stack([r, gg, b], -1))

# Clay run-off: rust red earth with pebbles.
earth = fbm(5, 5, 51)
peb = blur((rng.random((N, N)) > 0.992).astype(float), 1) * 2.2
v = 0.8 + 0.25 * (earth - 0.5) + 0.1 * (blur(rng.random((N, N)), 1) - 0.5)
save("clay", np.stack([0.62 * v + 0.15 * peb, 0.26 * v + 0.12 * peb, 0.17 * v + 0.1 * peb], -1))

# The dirt track, with its own generator so the textures above never change.
rng = np.random.default_rng(97)

def stones(density, r, seed):
    """Round, domed stones with a lit top-left and a shadowed bottom-right:
    (coverage, brightness), both seamless."""
    g = np.random.default_rng(seed)
    seeds = g.random((N, N)) > 1 - density
    size = np.where(seeds, g.uniform(0.6, 1.0, (N, N)), 0)
    tone = g.random((N, N))
    cover = np.zeros((N, N))
    light = np.zeros((N, N))
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            d = np.hypot(dx, dy) / (r + 0.5)
            inside = np.clip((size - d) * 4, 0, 1)
            shifted = np.roll(np.roll(inside, dy, 0), dx, 1)
            lit = np.roll(np.roll(tone, dy, 0), dx, 1) + 0.25 * (-dx - dy) / (r + 0.5)
            take = shifted > cover
            light = np.where(take, lit, light)
            cover = np.maximum(cover, shifted)
    return cover, np.clip(light, 0, 1)

# Dirt: packed, sun-baked ochre earth; soft darker damp patches, fine grit and
# the odd embedded stone, so it reads as a road without any paint.
earth = fbm(4, 5, 61)
damp = np.clip((fbm(2, 3, 71) - 0.45) * 2.5, 0, 1)
grit = blur(rng.random((N, N)), 1) - 0.5
cover, tone = stones(0.0012, 3, 81)
v = 0.8 + 0.18 * (earth - 0.5) + 0.08 * grit - 0.1 * damp
v = v * (1 - 0.6 * cover) + 0.6 * cover * (0.6 + 0.5 * tone)
save("dirt", np.stack([0.70 * v, 0.53 * v, 0.36 * v], -1))

# Gravel: loose grey-beige stones, packed tight, on sandy earth.
sand = fbm(6, 4, 91)
small, ts = stones(0.035, 2, 101)
big, tb = stones(0.006, 4, 111)
v = 0.62 + 0.1 * (sand - 0.5)
v = v * (1 - small) + small * (0.62 + 0.4 * ts)
v = v * (1 - big) + big * (0.6 + 0.42 * tb)
# Dark gaps between stones.
both = np.maximum(small, big)
v -= 0.18 * np.clip(blur(both, 1) - both, 0, 1)
warm = 0.08 * (fbm(3, 3, 121) - 0.5)
save("gravel", np.stack([0.80 * v + warm, 0.74 * v + warm * 0.6, 0.62 * v], -1))

# Scree: the dirt track's surroundings. Pale, dusty rock and grit, broken by
# sparse tufts of dry grass and scattered stones.
ground = fbm(3, 5, 131)
tufts = np.clip((fbm(6, 3, 141) - 0.58) * 4, 0, 1) * np.clip(blur(rng.random((N, N)), 1) * 2.2 - 0.6, 0, 1)
rocks, tr = stones(0.004, 4, 151)
pebbles, tp = stones(0.02, 1, 161)
v = 0.68 + 0.2 * (ground - 0.5) + 0.06 * (blur(rng.random((N, N)), 1) - 0.5)
rgb = np.stack([0.80 * v, 0.72 * v, 0.60 * v], -1)
grass = np.stack([0.52 + 0.1 * tufts, 0.50 + 0.08 * tufts, 0.26 + 0 * tufts], -1)
rgb = rgb * (1 - tufts[..., None] * 0.8) + grass * tufts[..., None] * 0.8
for cover, tone, k in ((pebbles, tp, 0.85), (rocks, tr, 0.95)):
    stone = (0.45 + 0.45 * tone)[..., None] * np.array([0.82, 0.79, 0.74])
    rgb = rgb * (1 - cover[..., None] * k) + stone * cover[..., None] * k
save("scree", rgb)
print("textures written to", OUT)
