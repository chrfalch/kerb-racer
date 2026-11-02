# Baked low-sun shadows for Kerb Racers' medium and low graphics.
#   blender -b -P art/src/shade.py
# Reads art/src/casters-{0,1,2,3}.json (written by the logic test
# `baked_shade_matches_the_woods` with KR_REBAKE=1) and paints each track's
# tree, bush and fence shadows into art/textures/shade-{id}.png: an RGBA
# overlay the game lays over the whole ground. Also paints art/textures/blob.png,
# the soft shadow under each car.
import bpy, json, math, os
import numpy as np

SRC = os.path.dirname(os.path.abspath(__file__))
TEXTURES = os.path.join(os.path.dirname(SRC), "textures")
N = 1024
# The game's sun sits at (-70, 24, -28) looking at the origin: its rays run
# along DIR over the ground and drop DROP metres per metre they travel.
DIR = np.array([70.0, 28.0]) / math.hypot(70.0, 28.0)
DROP = 24.0 / math.hypot(70.0, 28.0)
# How far (m) a ray steps while it looks for the hillside it lands on.
STEP = 0.1
SHADOW = np.array([0.07, 0.09, 0.18])  # sRGB, a cool blue shade
CONIFERS = {"pine-a": 8.5, "pine-b": 6.5, "pine-c": 10.5}


def save(name, rgb, alpha):
    h, w = alpha.shape
    img = bpy.data.images.new(name, w, h, alpha=True)
    px = np.concatenate([np.broadcast_to(rgb, (h, w, 3)), alpha[..., None]], axis=-1)[::-1]
    img.pixels.foreach_set(px.astype(np.float32).ravel())
    img.filepath_raw = os.path.join(TEXTURES, name + ".png")
    img.file_format = "PNG"
    img.save()


def blur(a, r):
    for axis in (0, 1):
        acc = np.zeros_like(a)
        for k in range(-r, r + 1):
            acc += np.roll(a, k, axis=axis)
        a = acc / (2 * r + 1)
    return a


def bake(track):
    items = json.load(open(os.path.join(SRC, f"casters-{track}.json")))
    ground = items[0]
    x0, z0 = ground["x"] - ground["w"] / 2, ground["z"] - ground["h"] / 2
    sx, sz = N / ground["w"], N / ground["h"]
    land = np.array(ground["land"])  # corner heights, [x][z]
    cell = ground["w"] / (land.shape[0] - 1)
    cover = np.zeros((N, N))

    def ground_at(x, z):
        """The land under (x, z), bilinear between its grid corners."""
        fx = np.clip((x - x0) / cell, 0, land.shape[0] - 1.001)
        fz = np.clip((z - z0) / cell, 0, land.shape[1] - 1.001)
        i, j = fx.astype(int), fz.astype(int)
        u, v = fx - i, fz - j
        return (land[i, j] * (1 - u) * (1 - v) + land[i + 1, j] * u * (1 - v)
                + land[i, j + 1] * (1 - u) * v + land[i + 1, j + 1] * u * v)

    def cast(x, z, y):
        """Where the sun's rays through (x, y, z) for each height in `y` meet
        the land: down a slope a shadow reaches further, up one it falls short."""
        y = np.atleast_1d(np.asarray(y, dtype=float))
        reach = max((y.max() - land.min()) / DROP + 2 * STEP, 2 * STEP)
        s = np.arange(0.0, reach, STEP)
        px, pz = x + DIR[0] * s, z + DIR[1] * s
        gap = y[:, None] - DROP * s[None, :] - ground_at(px, pz)[None, :]
        under = gap <= 0
        k = np.where(under.any(axis=1), under.argmax(axis=1), len(s) - 1)
        # Between the last step above the land and the first below it.
        k0 = np.maximum(k - 1, 0)
        g0, g1 = gap[np.arange(len(y)), k0], gap[np.arange(len(y)), k]
        f = np.divide(g0, g0 - g1, out=np.zeros_like(g0), where=(k > 0) & (g0 != g1))
        d = s[k0] + (s[k] - s[k0]) * np.clip(f, 0, 1)
        return x + DIR[0] * d, z + DIR[1] * d

    def disc(x, z, r, strength=1.0):
        cx, cz, rx, rz = (x - x0) * sx, (z - z0) * sz, r * sx, r * sz
        i0, i1 = int(max(cz - rz - 1, 0)), int(min(cz + rz + 2, N))
        j0, j1 = int(max(cx - rx - 1, 0)), int(min(cx + rx + 2, N))
        if i0 >= i1 or j0 >= j1:
            return
        ii, jj = np.mgrid[i0:i1, j0:j1]
        d = ((jj + 0.5 - cx) / rx) ** 2 + ((ii + 0.5 - cz) / rz) ** 2
        patch = cover[i0:i1, j0:j1]
        np.maximum(patch, np.clip((1.0 - d) * 3.0, 0, 1) * strength, out=patch)

    def discs(x, z, y, hs, rs, strength=1.0):
        """Shade at each height in `hs` above (x, y, z), `rs` metres across."""
        cx, cz = cast(x, z, y + np.asarray(hs, dtype=float))
        for px, pz, r in zip(cx, cz, np.broadcast_to(rs, cx.shape)):
            if r > 0:
                disc(px, pz, r, strength)

    def stroke(x, z, y, h0, h1, r, strength=1.0):
        """A vertical pole from h0 to h1 metres: its shadow, a smear along the sun."""
        steps = max(2, int((h1 - h0) / DROP * sx / 2))
        discs(x, z, y, np.linspace(h0, h1, steps + 1), r, strength)

    for it in items[1:]:
        x, y, z, s, kind = it["x"], it["y"], it["z"], it["scale"], it["kind"]
        if kind in CONIFERS:
            height = CONIFERS[kind] * s
            stroke(x, z, y, 0, height * 0.25, 0.18 * s)
            t = np.linspace(0, 1, 28)
            hs = height * (0.2 + 0.78 * t)
            rs = ((1 - t) ** 0.95 * height * 0.34 + 0.45) * 0.85
            discs(x, z, y, hs, rs, 0.9)
        elif kind == "tree-a":
            size = 4.2 * s
            stroke(x, z, y, 0, size * 0.4, 0.2 * s)
            centre, radius = size * 0.6, size * 0.55
            hs = centre - radius + 2 * radius * np.linspace(0, 1, 18)
            rs = np.sqrt(np.maximum(radius ** 2 - (hs - centre) ** 2, 0))
            discs(x, z, y, hs, rs * 0.9, 0.85)
        elif kind == "rock":
            # A squashed ico sphere: ~0.9 m by 0.7 across, 0.6 tall, per unit scale.
            height = 0.6 * it["h"]
            k = np.arange(6)
            discs(x, z, y, height * k / 6, 0.75 * s * (1 - 0.5 * k / 6), 0.85)
        elif kind == "bush":
            discs(x, z, y, [0.4], 0.8 * s, 0.7)
        elif kind == "fence":
            dx, dz = it["dx"] * 3, it["dz"] * 3
            stroke(x, z, y, 0, 1.2, 0.1, 0.8)
            # The rails follow the land along the section.
            lift = y - ground_at(x, z)
            for t in np.linspace(0, 1, 16):
                rx, rz = x + dx * t, z + dz * t
                discs(rx, rz, ground_at(rx, rz) + lift, [0.25, 1.05], 0.08, 0.7)
    # Light through the needles: break the solid shapes up a little.
    rng = np.random.default_rng(track)
    holes = blur((rng.random((N, N)) > 0.86).astype(float), 2)
    cover = cover * (1 - 0.45 * np.clip(holes * 3, 0, 1))
    alpha = blur(cover, 1) * 0.55
    save(f"shade-{track}", SHADOW, alpha)
    print(f"shade-{track}: {len(items) - 1} casters, {100 * (alpha > 0.05).mean():.0f}% shaded")


def blob():
    w, h = 128, 64
    yy, xx = np.mgrid[0:h, 0:w]
    u = (xx + 0.5) / w * 2 - 1
    v = (yy + 0.5) / h * 2 - 1
    d = (np.abs(u) ** 4 + np.abs(v) ** 4) ** 0.25
    alpha = np.clip((1 - d) / 0.45, 0, 1) ** 1.5 * 0.6
    save("blob", SHADOW, alpha)


for track in range(4):
    bake(track)
blob()
