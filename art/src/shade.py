# Baked low-sun shadows for Kerb Racers' medium and low graphics.
#   blender -b -P art/src/shade.py
# Reads art/src/casters-{0,1,2}.json (written by the logic test
# `baked_shade_matches_the_woods` with KR_REBAKE=1) and paints each track's
# tree, bush and fence shadows into art/textures/shade-{id}.png: an RGBA
# overlay the game lays over the whole ground. Also paints art/textures/blob.png,
# the soft shadow under each car.
import bpy, json, math, os
import numpy as np

SRC = os.path.dirname(os.path.abspath(__file__))
TEXTURES = os.path.join(os.path.dirname(SRC), "textures")
N = 1024
# The game's sun sits at (-70, 24, -28) looking at the origin: a point h metres
# up throws its shadow this far along the ground, per metre of height.
SUN = np.array([70.0, 28.0]) / 24.0
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
    cover = np.zeros((N, N))

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

    def stroke(x, z, h0, h1, r, strength=1.0):
        """A vertical pole from h0 to h1 metres: its shadow, a smear along the sun."""
        steps = max(2, int((h1 - h0) * SUN[0] * sx / 2))
        for k in range(steps + 1):
            h = h0 + (h1 - h0) * k / steps
            disc(x + SUN[0] * h, z + SUN[1] * h, r, strength)

    for it in items[1:]:
        x, z, s, kind = it["x"], it["z"], it["scale"], it["kind"]
        if kind in CONIFERS:
            height = CONIFERS[kind] * s
            stroke(x, z, 0, height * 0.25, 0.18 * s)
            for k in range(28):
                t = k / 27
                h = height * (0.2 + 0.78 * t)
                r = ((1 - t) ** 0.95 * height * 0.34 + 0.45) * 0.85
                disc(x + SUN[0] * h, z + SUN[1] * h, r, 0.9)
        elif kind == "tree-a":
            size = 4.2 * s
            stroke(x, z, 0, size * 0.4, 0.2 * s)
            centre, radius = size * 0.6, size * 0.55
            for k in range(18):
                h = centre - radius + 2 * radius * k / 17
                r = math.sqrt(max(radius ** 2 - (h - centre) ** 2, 0))
                disc(x + SUN[0] * h, z + SUN[1] * h, r * 0.9, 0.85)
        elif kind == "bush":
            disc(x + SUN[0] * 0.4, z + SUN[1] * 0.4, 0.8 * s, 0.7)
        elif kind == "fence":
            dx, dz = it["dx"] * 3, it["dz"] * 3
            stroke(x, z, 0, 1.2, 0.1, 0.8)
            for h in (0.25, 1.05):
                for k in range(16):
                    t = k / 15
                    disc(x + dx * t + SUN[0] * h, z + dz * t + SUN[1] * h, 0.08, 0.7)
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


for track in range(3):
    bake(track)
blob()
