# Kerb Racers' woods: fluffy conifers, leafy bushes and ferns made of
# alpha-cut foliage cards with soft, rounded normals, plus mushrooms, logs,
# stumps, rocks and an X-braced wooden fence.
#   blender -b -P art/src/nature.py
# Paints its own foliage textures (art/src/foliage/*.png, embedded in each
# GLB as alpha-masked materials) and writes art/models/*.glb.
import bpy, bmesh, math, os, random, sys
import numpy as np
from mathutils import Vector, Matrix

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from kit import OUT, reset, material, link, export, srgb  # noqa: E402

FOLIAGE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "foliage")


# --- Painted foliage ---------------------------------------------------------

def stroke(rgb, alpha, p0, p1, width, color):
    """A tapered line from p0 to p1 into an RGBA canvas (rows = v, cols = u)."""
    h, w = alpha.shape
    steps = int(max(abs(p1[0] - p0[0]), abs(p1[1] - p0[1])) * 1.5) + 2
    for k in range(steps):
        t = k / (steps - 1)
        x = p0[0] + (p1[0] - p0[0]) * t
        y = p0[1] + (p1[1] - p0[1]) * t
        r = max(width * (1 - 0.7 * t), 0.6)
        x0, x1 = int(max(x - r, 0)), int(min(x + r + 1, w))
        y0, y1 = int(max(y - r, 0)), int(min(y + r + 1, h))
        if x0 >= x1 or y0 >= y1:
            continue
        yy, xx = np.mgrid[y0:y1, x0:x1]
        mask = (xx - x) ** 2 + (yy - y) ** 2 <= r * r
        shade = color * (0.85 + 0.3 * t)
        rgb[y0:y1, x0:x1][mask] = shade
        alpha[y0:y1, x0:x1][mask] = 1.0


def bleed(rgb, alpha):
    """Fill transparent texels with the mean colour so mips have no dark fringe."""
    mean = rgb[alpha > 0.5].mean(axis=0)
    rgb[alpha <= 0.5] = mean
    return rgb


def save(name, rgb, alpha):
    h, w = alpha.shape
    img = bpy.data.images.new(name, w, h, alpha=True)
    px = np.concatenate([np.clip(rgb, 0, 1), alpha[..., None]], axis=-1)[::-1]
    img.pixels.foreach_set(px.astype(np.float32).ravel())
    path = os.path.join(FOLIAGE, name + ".png")
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    return path


def paint_needles(name, base, tip, seed):
    """A conifer frond pointing up the texture: a twig with needles fanning off it."""
    rng = np.random.default_rng(seed)
    h, w = 512, 256
    rgb = np.zeros((h, w, 3))
    alpha = np.zeros((h, w))
    base, tip = np.array(base), np.array(tip)
    stroke(rgb, alpha, (w / 2, h - 4), (w / 2, 10), 3, np.array([0.28, 0.22, 0.12]))
    for side_twig in range(7):
        t = 0.15 + side_twig * 0.11
        y = h - 4 - t * (h - 20)
        dirx = 1 if side_twig % 2 else -1
        stroke(rgb, alpha, (w / 2, y), (w / 2 + dirx * (1 - t) * 90, y - 50), 2, np.array([0.28, 0.22, 0.12]))
    for _ in range(2600):
        t = rng.random() ** 0.8
        y = h - 6 - t * (h - 24)
        spread = math.sin(math.pi * min(t * 1.1, 1.0)) ** 0.7 * (1 - 0.35 * t) * (w * 0.46)
        x = w / 2 + rng.uniform(-0.85, 0.85) * spread * 0.7
        ang = rng.uniform(0.35, 1.1) * (1 if rng.random() < 0.5 else -1)
        length = rng.uniform(14, 30) * (1 - 0.4 * t)
        end = (x + math.sin(ang) * length, y - math.cos(ang) * length)
        if abs(end[0] - w / 2) > spread:
            continue
        mix = min(1, t * 0.6 + rng.random() * 0.6)
        stroke(rgb, alpha, (x, y), end, rng.uniform(1.8, 3.0), base * (1 - mix) + tip * mix)
    return save(name, bleed(rgb, alpha), alpha)


def paint_leaves(name, base, tip, count, seed):
    """A round cluster of small oval leaves."""
    rng = np.random.default_rng(seed)
    n = 384
    rgb = np.zeros((n, n, 3))
    alpha = np.zeros((n, n))
    for _ in range(count):
        a = rng.uniform(0, math.tau)
        d = (rng.random() ** 0.45) * n * 0.44
        x, y = n / 2 + math.cos(a) * d, n / 2 + math.sin(a) * d
        ang = rng.uniform(0, math.tau)
        length = rng.uniform(18, 32)
        end = (x + math.cos(ang) * length, y + math.sin(ang) * length)
        mix = rng.random()
        # Leaves near the top of the texture catch more light.
        mix = min(1, mix * 0.7 + (1 - y / n) * 0.5)
        stroke(rgb, alpha, (x, y), end, rng.uniform(5, 9), base * (1 - mix) + tip * mix)
    return save(name, bleed(rgb, alpha), alpha)


def paint_fern():
    rng = np.random.default_rng(9)
    h, w = 512, 192
    rgb = np.zeros((h, w, 3))
    alpha = np.zeros((h, w))
    stroke(rgb, alpha, (w / 2, h - 2), (w / 2, 8), 2.5, np.array([0.3, 0.45, 0.18]))
    for k in range(34):
        t = k / 34
        y = h - 12 - t * (h - 30)
        span = (1 - t) ** 0.6 * w * 0.47
        for s in (-1, 1):
            col = np.array([0.30, 0.55, 0.18]) * (0.85 + 0.3 * rng.random())
            stroke(rgb, alpha, (w / 2, y), (w / 2 + s * span, y - 14 - 10 * t), 4.5 * (1 - t * 0.6) + 1, col)
    return save("fern", bleed(rgb, alpha), alpha)


def paint_daisies():
    rng = np.random.default_rng(13)
    n = 256
    rgb = np.zeros((n, n, 3))
    alpha = np.zeros((n, n))
    for _ in range(9):
        cx, cy = rng.uniform(40, n - 40), rng.uniform(40, n - 40)
        for k in range(10):
            a = k * math.tau / 10
            stroke(rgb, alpha, (cx, cy), (cx + math.cos(a) * 16, cy + math.sin(a) * 16), 4, np.array([0.97, 0.96, 0.92]))
        stroke(rgb, alpha, (cx - 2, cy), (cx + 2, cy), 5, np.array([0.95, 0.75, 0.15]))
    return save("daisies", bleed(rgb, alpha), alpha)


def cutout(name, path, tint=(1, 1, 1)):
    """An alpha-masked, double-sided material over a painted texture."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    m.use_backface_culling = False
    nodes, links = m.node_tree.nodes, m.node_tree.links
    p = nodes["Principled BSDF"]
    p.inputs["Roughness"].default_value = 0.85
    p.inputs["Metallic"].default_value = 0.0
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(path, check_existing=True)
    links.new(tex.outputs["Color"], p.inputs["Base Color"])
    clip = nodes.new("ShaderNodeMath")
    clip.operation = "GREATER_THAN"
    clip.inputs[1].default_value = 0.5
    links.new(tex.outputs["Alpha"], clip.inputs[0])
    links.new(clip.outputs[0], p.inputs["Alpha"])
    return m


# --- Card geometry -----------------------------------------------------------

class Cards:
    """Foliage cards in one mesh, with normals pointing out of a soft volume."""

    def __init__(self, name, mat, centre, stretch=(1, 1, 1)):
        self.bm = bmesh.new()
        # Named as the primitives name theirs, so joining keeps one UV map.
        self.uv = self.bm.loops.layers.uv.new("UVMap")
        self.name, self.mat = name, mat
        self.centre, self.stretch = Vector(centre), Vector(stretch)
        # Per-vertex shading centre: a lobe's own centre, or the crown's.
        self.centres = {}

    def card(self, root, direction, up, length, width, droop=0.0, segments=3, centre=None):
        """A bent strip from `root` along `direction`, the texture's v running root to tip.
        `centre` shades it as part of a lobe around that point instead of the whole crown."""
        direction = direction.normalized()
        side = direction.cross(up).normalized() * (width / 2)
        rows = []
        for k in range(segments + 1):
            t = k / segments
            p = root + direction * (length * t) - Vector((0, 0, 1)) * droop * t * t
            a, b = self.bm.verts.new(p - side), self.bm.verts.new(p + side)
            if centre is not None:
                self.centres[a] = self.centres[b] = (Vector(centre), 1.0)
            rows.append((a, b, t))
        for (a0, a1, t0), (b0, b1, t1) in zip(rows, rows[1:]):
            f = self.bm.faces.new([a0, a1, b1, b0])
            for loop, uv in zip(f.loops, [(0, t0), (1, t0), (1, t1), (0, t1)]):
                loop[self.uv].uv = uv

    def build(self):
        me = bpy.data.meshes.new(self.name)
        self.bm.verts.index_update()
        lobes = {v.index: c for v, c in self.centres.items()}
        self.bm.to_mesh(me)
        self.bm.free()
        me.materials.append(self.mat)
        for p in me.polygons:
            p.use_smooth = True
        normals = []
        for v in me.vertices:
            # Round, soft shading: each leaf takes the normal of the blob it sits on.
            lobe = lobes.get(v.index)
            d = v.co - (lobe[0] if lobe else self.centre)
            if not lobe:
                d = Vector((d.x / self.stretch.x, d.y / self.stretch.y, d.z / self.stretch.z))
            normals.append((d.normalized() + Vector((0, 0, 0.45))).normalized())
        me.normals_split_custom_set_from_vertices(normals)
        return link(bpy.data.objects.new(self.name, me))


def trunk(name, height, r0, r1, mat, verts=8):
    bpy.ops.mesh.primitive_cone_add(vertices=verts, radius1=r0, radius2=r1, depth=height, location=(0, 0, height / 2))
    o = bpy.context.active_object
    o.name = name
    o.data.materials.append(mat)
    for p in o.data.polygons:
        p.use_smooth = True
    return o


def conifer(name, seed, height, needles_path):
    reset()
    rng = random.Random(seed)
    bark = material("bark", 0x4A3220, rough=0.95)
    trunk("trunk", height * 0.75, 0.22 * height / 8, 0.05, bark)
    mat = cutout("needles", needles_path)
    # An opaque core inside the branches: it hides the cards behind it, so the
    # GPU rejects them by depth instead of shading layer after layer of needles.
    # Dark, so the gaps between branches read as the shade inside the tree.
    core = material("core", 0x18301F, rough=0.95)
    bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=height * 0.2, radius2=0.0, depth=height * 0.6,
                                    location=(0, 0, height * 0.15 + height * 0.3))
    bpy.context.active_object.data.materials.append(core)
    cards = Cards("crown", mat, (0, 0, height * 0.45), (1, 1, 1.6))
    # Stacked tiers, each a round skirt of broad, overlapping branches: from above
    # the tree reads as a few soft rings, darker underneath each one.
    tiers = max(4, int(height * 0.6))
    for k in range(tiers):
        t = k / (tiers - 1)
        z = height * (0.18 + 0.72 * t)
        reach = (1 - t) ** 0.9 * height * 0.36 + 0.55
        tier = Vector((0, 0, z - reach * 0.15))
        n = max(5, int(9 - 4 * t))
        twist = rng.uniform(0, math.tau)
        for j in range(n):
            a = twist + j * math.tau / n + rng.uniform(-0.2, 0.2)
            out = Vector((math.cos(a), math.sin(a), rng.uniform(0.0, 0.2)))
            # Mostly flat to the camera, a few tilted for the silhouette.
            tilt = 0.6 if (j + k) % 3 == 0 else 0.05
            up = Vector((0, 0, 1)).lerp(Vector((-math.sin(a), math.cos(a), 0)), tilt).normalized()
            cards.card(Vector((0, 0, z)), out, up, reach * rng.uniform(0.95, 1.1), reach * 1.5,
                       droop=reach * 0.45, segments=2, centre=tier)
    # The leader: crossed vertical cards.
    for a in (0, math.pi / 2):
        d = Vector((math.cos(a), math.sin(a), 0))
        cards.card(Vector((0, 0, height * 0.86)), Vector((0, 0, 1)), d.cross(Vector((0, 0, 1))), height * 0.2, 0.7, segments=1)
    cards.build()
    export(name)


def broadleaf_bush(name, seed, leaves_path, size, lobes, core_hex, trunk_height=0.0):
    """Round clumps of leaves: each lobe an opaque dark ball wrapped in leaf cards
    that lie on its surface, shaded as one soft sphere."""
    reset()
    rng = random.Random(seed)
    mat = cutout("leaves", leaves_path)
    core = material("core", core_hex, rough=0.95)
    if trunk_height:
        bark = material("bark", 0x4A3220, rough=0.95)
        trunk("trunk", trunk_height + size * 0.3, size * 0.07, size * 0.035, bark)
    base = trunk_height
    cards = Cards("leaves", mat, (0, 0, base + size * 0.5), (1, 1, 0.8))
    for k in range(lobes):
        a = k * math.tau / lobes + rng.uniform(-0.3, 0.3)
        d = 0.0 if k == 0 else size * rng.uniform(0.22, 0.38)
        r = size * (0.42 if k == 0 else rng.uniform(0.26, 0.34))
        c = Vector((math.cos(a) * d, math.sin(a) * d, base + r * 0.9 + (size * 0.25 if k == 0 else rng.uniform(0, size * 0.15))))
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=r * 0.8, location=c)
        ball = bpy.context.active_object
        ball.data.materials.append(core)
        for p in ball.data.polygons:
            p.use_smooth = True
        cards_on = 14 if k == 0 else 10
        for j in range(cards_on):
            # Spread over the upper hemisphere, where the camera looks.
            u = (j + rng.random()) / cards_on
            phi = math.acos(1 - 1.4 * u)
            th = rng.uniform(0, math.tau)
            nrm = Vector((math.sin(phi) * math.cos(th), math.sin(phi) * math.sin(th), math.cos(phi)))
            tang = nrm.orthogonal().normalized()
            s = r * rng.uniform(1.1, 1.35)
            centre = c + nrm * r * 0.85
            cards.card(centre - tang * s * 0.5, tang, nrm, s, s, segments=1, centre=c)
    cards.build()
    export(name)


def fern(name, path):
    reset()
    rng = random.Random(21)
    mat = cutout("fern", path)
    cards = Cards("fronds", mat, (0, 0, 0.0), (1, 1, 0.6))
    for k in range(9):
        a = k * math.tau / 9 + rng.uniform(-0.2, 0.2)
        out = Vector((math.cos(a), math.sin(a), 0.9 + rng.uniform(0, 0.4))).normalized()
        cards.card(Vector((0, 0, 0.02)), out, Vector((0, 0, 1)), rng.uniform(0.9, 1.2), 0.45, droop=0.5, segments=4)
    cards.build()
    export(name)


def daisies(name, path):
    reset()
    mat = cutout("daisies", path)
    cards = Cards("daisies", mat, (0, 0, -3), (1, 1, 1))
    cards.card(Vector((0, -0.6, 0.04)), Vector((0, 1, 0)), Vector((0, 0, 1)), 1.2, 1.2, segments=1)
    cards.build()
    export(name)


# --- Props -------------------------------------------------------------------

def jitter(obj, amount, seed):
    r = random.Random(seed)
    for v in obj.data.vertices:
        v.co += Vector((r.uniform(-amount, amount), r.uniform(-amount, amount), r.uniform(-amount * 0.5, amount * 0.5)))


def box(name, at, size, mat, rot=(0, 0, 0), bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=at, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.scale = size
    o.data.materials.append(mat)
    if bevel:
        m = o.modifiers.new("bevel", "BEVEL")
        m.width = bevel
        m.segments = 2
    return o


def cylinder(name, at, r, depth, mat, rot=(0, 0, 0), verts=12):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=r, depth=depth, location=at, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.data.materials.append(mat)
    for p in o.data.polygons:
        p.use_smooth = len(p.vertices) == 4
    return o


def mushrooms(name):
    reset()
    cap = material("cap", 0xC8282A, rough=0.5)
    stem = material("stem", 0xEFE6D2, rough=0.8)
    for k, (x, y, s) in enumerate([(0, 0, 1.0), (0.22, 0.12, 0.7), (-0.15, 0.2, 0.55)]):
        cylinder(f"stem{k}", (x, y, 0.09 * s), 0.035 * s, 0.18 * s, stem, verts=6)
        bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=0.1 * s, location=(x, y, 0.18 * s))
        c = bpy.context.active_object
        c.scale.z = 0.6
        c.data.materials.append(cap)
        for p in c.data.polygons:
            p.use_smooth = True
    export(name)


def logs(name):
    reset()
    bark = material("bark", 0x5C3B22, rough=0.95)
    wood = material("wood", 0xC8955A, rough=0.8)
    for k, (y, z) in enumerate([(-0.3, 0.2), (0.3, 0.2), (0.0, 0.52)]):
        cylinder(f"log{k}", (0, y, z), 0.2, 1.8, bark, rot=(0, math.pi / 2, 0), verts=12)
        for s in (-1, 1):
            cylinder(f"end{k}{s}", (s * 0.905, y, z), 0.185, 0.02, wood, rot=(0, math.pi / 2, 0), verts=12)
    export(name)


def stump(name):
    reset()
    bark = material("bark", 0x5C3B22, rough=0.95)
    wood = material("wood", 0xC8955A, rough=0.8)
    cylinder("stump", (0, 0, 0.2), 0.32, 0.4, bark, verts=11)
    cylinder("top", (0, 0, 0.405), 0.29, 0.02, wood, verts=11)
    export(name)


def rock(name, seed):
    reset()
    stone = material("stone", 0x9A958A, rough=0.95)
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.7, location=(0, 0, 0.2))
    o = bpy.context.active_object
    o.scale = (1.3, 1.0, 0.6)
    o.data.materials.append(stone)
    jitter(o, 0.14, seed)
    export(name)


def fence(name):
    """A three-metre section from x=0 to x=3: a capped post, a half post, a chunky
    top rail and a rail low down, and a lattice of X braces, one per half bay."""
    reset()
    post = material("post", 0x5A361D, rough=0.9)
    rail = material("rail", 0x8E5730, rough=0.85)
    box("post", (0, 0, 0.65), (0.22, 0.22, 1.3), post, bevel=0.03)
    box("cap", (0, 0, 1.32), (0.28, 0.28, 0.07), post, bevel=0.02)
    box("mid", (1.5, 0, 0.55), (0.14, 0.14, 1.1), post, bevel=0.02)
    box("top", (1.5, 0, 1.12), (3.04, 0.16, 0.16), rail, bevel=0.03)
    box("bottom", (1.5, 0, 0.22), (3.04, 0.11, 0.11), rail, bevel=0.02)
    angle = math.atan2(0.82, 1.42)
    for k, x in enumerate((0.75, 2.25)):
        for s in (-1, 1):
            box(f"brace{k}{s}", (x, 0.025 * s, 0.67), (1.62, 0.07, 0.1), rail, rot=(0, s * angle, 0))
    export(name)


def barrier(name):
    """A low trackside hoarding in front of the fence, three metres long: boards
    in alternating red and white on a wooden frame, leaning back a little."""
    reset()
    frame = material("frame", 0x5A361D, rough=0.9)
    red = material("red", 0xC9372B, rough=0.7)
    white = material("white", 0xEDE8DC, rough=0.7)
    for x in (0.0, 1.5):
        box(f"stake{x}", (x + 0.08, 0.12, 0.3), (0.1, 0.1, 0.6), frame)
    for k in range(6):
        box(f"board{k}", (k * 0.5 + 0.25, 0, 0.32), (0.48, 0.06, 0.5), red if k % 2 == 0 else white,
            rot=(math.radians(-8), 0, 0), bevel=0.012)
    box("rail", (1.5, 0.06, 0.6), (3.0, 0.1, 0.07), frame, rot=(math.radians(-8), 0, 0))
    export(name)


def poppies(name):
    """A little clump of red poppies on thin stems."""
    reset()
    stem = material("stem", 0x4D7A2A, rough=0.8)
    petal = material("petal", 0xD8281E, rough=0.6)
    heart = material("heart", 0x1E1A16, rough=0.8)
    rng = random.Random(17)
    for k in range(5):
        x, y = rng.uniform(-0.25, 0.25), rng.uniform(-0.25, 0.25)
        h = rng.uniform(0.22, 0.38)
        cylinder(f"stem{k}", (x, y, h / 2), 0.012, h, stem, verts=4)
        bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=0.075, location=(x, y, h))
        c = bpy.context.active_object
        c.scale.z = 0.45
        c.data.materials.append(petal)
        cylinder(f"heart{k}", (x, y, h + 0.032), 0.022, 0.012, heart, verts=6)
    export(name)


os.makedirs(FOLIAGE, exist_ok=True)
os.makedirs(OUT, exist_ok=True)
reset()
# Deep blue-greens in the shade, warm yellow only where the low sun catches the tips.
needles = paint_needles("needles", [0.10, 0.25, 0.18], [0.38, 0.54, 0.24], 3)
needles_dark = paint_needles("needles-dark", [0.07, 0.19, 0.15], [0.26, 0.42, 0.22], 4)
needles_warm = paint_needles("needles-warm", [0.13, 0.29, 0.16], [0.50, 0.60, 0.25], 5)
leaves = paint_leaves("leaves", np.array([0.12, 0.29, 0.14]), np.array([0.50, 0.64, 0.24]), 1500, 5)
shrub = paint_leaves("shrub", np.array([0.15, 0.33, 0.15]), np.array([0.58, 0.70, 0.28]), 1200, 6)
fern_tex = paint_fern()
daisy_tex = paint_daisies()
conifer("pine-a", 1, 8.5, needles)
conifer("pine-b", 2, 6.5, needles_dark)
conifer("pine-c", 3, 10.5, needles_warm)
broadleaf_bush("tree-a", 4, leaves, 4.6, 6, 0x1C3A1E, trunk_height=2.2)
broadleaf_bush("bush", 5, shrub, 1.5, 4, 0x21401F)
fern("fern", fern_tex)
daisies("daisies", daisy_tex)
mushrooms("mushrooms")
logs("logs")
stump("stump")
rock("rock", 6)
fence("fence")
barrier("barrier")
poppies("poppies")
print("nature written to", OUT)
