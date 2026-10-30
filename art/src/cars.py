# Kerb Racers' cars: three 1970s rally icons, lofted from superellipse
# cross-sections so they read as real cars from above.
#   blender -b -P art/src/cars.py
# Writes art/models/car-{0..5}.glb: nose along +X, wheels on the ground,
# about 3.4 m long so they match the game's collision shape.
import bpy, bmesh, math, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from kit import OUT, reset, material, link, export  # noqa: E402

LENGTH = 3.4


def smooth_step(e0, e1, x):
    t = min(max((x - e0) / (e1 - e0), 0.0), 1.0)
    return t * t * (3 - 2 * t)


def bump(x, centre, half):
    """1 at the centre, 0 beyond `half`, cosine shoulders."""
    d = abs(x - centre) / half
    return 0.5 * (1 + math.cos(math.pi * d)) if d < 1 else 0.0


def piecewise(points, x):
    """Smooth through (x, v) points, in any order, clamped at the ends."""
    points = sorted(points)
    if x <= points[0][0]:
        return points[0][1]
    for (x0, v0), (x1, v1) in zip(points, points[1:]):
        if x <= x1:
            t = (x - x0) / (x1 - x0)
            t = t * t * (3 - 2 * t)
            return v0 + (v1 - v0) * t
    return points[-1][1]


def section(hw, zb, zt, n, taper, ring):
    """A closed superellipse cross-section, narrower toward the top by `taper`."""
    zc, hz = (zb + zt) / 2, (zt - zb) / 2
    pts = []
    for k in range(ring):
        a = 2 * math.pi * k / ring
        c, s = math.cos(a), math.sin(a)
        y = math.copysign(abs(c) ** (2 / n), c)
        z = math.copysign(abs(s) ** (2 / n), s)
        up = (z + 1) / 2
        pts.append((y * hw * (1 + (taper - 1) * up), zc + z * hz))
    return pts


def loft(name, x0, x1, shape, mat, stations=30, ring=24):
    """Stations from x0 to x1; shape(x) -> (half width, bottom z, top z, exponent, taper)."""
    bm = bmesh.new()
    rings = []
    for i in range(stations):
        t = i / (stations - 1)
        # Denser near the ends, where the shape turns fastest.
        t = 0.5 - 0.5 * math.cos(math.pi * t)
        x = x0 + (x1 - x0) * t
        hw, zb, zt, n, taper = shape(x)
        rings.append([bm.verts.new((x, y, z)) for y, z in section(hw, zb, zt, n, taper, ring)])
    for a, b in zip(rings, rings[1:]):
        for k in range(ring):
            j = (k + 1) % ring
            bm.faces.new([a[k], a[j], b[j], b[k]])
    for r, flip in ((rings[0], True), (rings[-1], False)):
        centre = bm.verts.new(tuple(sum(v.co[i] for v in r) / ring for i in range(3)))
        for k in range(ring):
            j = (k + 1) % ring
            bm.faces.new([centre, r[j], r[k]] if flip else [centre, r[k], r[j]])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for p in me.polygons:
        p.use_smooth = True
    me.materials.append(mat)
    return link(bpy.data.objects.new(name, me))


def surface_z(shape, x, y):
    """Height of a loft's upper surface at (x, y), or None off its plan."""
    hw, zb, zt, n, taper = shape(x)
    w = hw * (1 + taper) / 2
    if abs(y) >= w:
        return None
    zc, hz = (zb + zt) / 2, (zt - zb) / 2
    return zc + hz * (1 - (abs(y) / w) ** n) ** (1 / n)


def decal(name, shapes, x0, x1, y0, y1, mat, lift=0.012, steps=36, across=3):
    """A thin band draped over the highest of several lofts: stripes, roof panels."""
    bm = bmesh.new()
    grid = []
    for i in range(steps + 1):
        x = x0 + (x1 - x0) * i / steps
        row = []
        for j in range(across + 1):
            y = y0 + (y1 - y0) * j / across
            zs = [z for z in (surface_z(s, x, y) for s in shapes) if z is not None]
            row.append(bm.verts.new((x, y, (max(zs) if zs else 0.5) + lift)))
        grid.append(row)
    for a, b in zip(grid, grid[1:]):
        for j in range(across):
            f = bm.faces.new([a[j], b[j], b[j + 1], a[j + 1]])
            if f.normal.z < 0:
                f.normal_flip()
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for p in me.polygons:
        p.use_smooth = True
    me.materials.append(mat)
    return link(bpy.data.objects.new(name, me))


def cyl(name, at, radius, depth, mat, axis="Y", verts=20):
    rot = {"Y": (math.pi / 2, 0, 0), "X": (0, math.pi / 2, 0), "Z": (0, 0, 0)}[axis]
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=radius, depth=depth, location=at, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.data.materials.append(mat)
    for p in o.data.polygons:
        p.use_smooth = len(p.vertices) == 4
    return o


def block(name, at, size, mat, bevel=0.0, rot=(0, 0, 0)):
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


def text_mesh(name, body, size, mat, rot_z=0.0, side=0):
    """Bold text as a thin mesh centred on the origin, lying flat (side=0) or
    standing on the car's flank (side=±1, reading front to back from outside)."""
    bpy.ops.object.text_add(location=(0, 0, 0))
    o = bpy.context.active_object
    o.name = name
    o.data.body = body
    o.data.size = size
    o.data.align_x = "CENTER"
    o.data.align_y = "CENTER"
    o.data.extrude = 0.0
    o.data.resolution_u = 3
    o.data.offset = size * 0.035
    o.data.space_character = 1.05
    bpy.ops.object.convert(target="MESH")
    o = bpy.context.active_object
    o.data.materials.clear()
    o.data.materials.append(mat)
    if side == 0:
        o.rotation_euler = (0, 0, rot_z)
    elif side > 0:
        o.rotation_euler = (math.pi / 2, 0, math.pi)
    else:
        o.rotation_euler = (math.pi / 2, 0, 0)
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=False)
    return o


def top_of(shapes, x, y):
    zs = [z for z in (surface_z(s, x, y) for s in shapes) if z is not None]
    return max(zs) if zs else 0.5


def flat_on(o, shapes, x, y, lift):
    """Seat a flat decal on the highest surface at (x, y), pitched to its slope."""
    z = top_of(shapes, x, y)
    pitch = math.atan2(top_of(shapes, x + 0.15, y) - top_of(shapes, x - 0.15, y), 0.3)
    o.location = (x, y, z + lift)
    o.rotation_euler = (0, -pitch, 0)


def livery(m, body, cabin, roof_x, bonnet_x, door_x, door_z, number, sponsor, ink, bands=()):
    """Sponsor bands, race numbers on roof and doors, sponsor names on bonnet and flanks."""
    ink_m = material("ink", ink, rough=0.4)
    for k, (y0, y1, colour) in enumerate(bands):
        decal(f"band{k}", [body, cabin], -2.1, 2.1, y0, y1, material(f"band{k}", colour, rough=0.32), lift=0.02 + 0.002 * k, across=2)
    # Roof roundel and number, readable from behind the car.
    disc = cyl("roof-disc", (0, 0, 0), 0.27, 0.01, m["white"], axis="Z", verts=28)
    flat_on(disc, [cabin], roof_x, 0, 0.03)
    num = text_mesh("roof-num", number, 0.34, ink_m, rot_z=-math.pi / 2)
    flat_on(num, [cabin], roof_x, 0, 0.037)
    # Sponsor across the bonnet.
    name = text_mesh("bonnet-name", sponsor, 0.2, ink_m, rot_z=-math.pi / 2)
    flat_on(name, [body], bonnet_x, 0, 0.03)
    for s in (-1, 1):
        hw = body(door_x)[0]
        num = text_mesh(f"door-num{s}", number, 0.22, ink_m, side=s)
        num.location = (door_x, s * (hw + 0.02), door_z)
        flank = text_mesh(f"flank{s}", sponsor, 0.13, ink_m, side=s)
        flank.location = (door_x - 0.95, s * (body(door_x - 0.95)[0] + 0.012), door_z - 0.12)


def palette(paint, stripe, roof):
    return dict(
        paint=material("paint", paint, rough=0.28, metal=0.1),
        stripe=material("stripe", stripe, rough=0.32),
        roof=material("roof", roof, rough=0.32),
        glass=material("glass", 0x16202B, rough=0.06, metal=0.4),
        tyre=material("tyre", 0x161719, rough=0.92),
        rim=material("rim", 0xC9CCD2, rough=0.25, metal=0.85),
        hub=material("hub", 0x2B2D31, rough=0.6),
        chrome=material("chrome", 0xE2E4E8, rough=0.15, metal=1.0),
        lamp=material("lamp", 0xFFF4D0, rough=0.1, emit=0xFFE9B0),
        tail=material("tail", 0xB01018, rough=0.3, emit=0xFF2A20),
        black=material("black", 0x111214, rough=0.6),
        white=material("white", 0xF4F1EA, rough=0.4),
    )


def wheels(m, front, rear, track, radius, width, rim_style="disc"):
    for x in (front, rear):
        for s in (-1, 1):
            y = s * track
            cyl(f"tyre{x}{s}", (x, y, radius), radius, width, m["tyre"], verts=14)
            cyl(f"rim{x}{s}", (x, y + s * (width / 2 + 0.002), radius), radius * 0.66, 0.012, m["rim"], verts=12)
            cyl(f"hub{x}{s}", (x, y + s * (width / 2 + 0.008), radius), radius * 0.2, 0.012, m["hub"], verts=10)
            if rim_style == "fuchs":
                for k in range(5):
                    a = k * 2 * math.pi / 5
                    block(f"spoke{x}{s}{k}", (x + math.cos(a) * radius * 0.38, y + s * (width / 2 + 0.006), radius + math.sin(a) * radius * 0.38),
                          (radius * 0.5, 0.01, radius * 0.12), m["black"], rot=(0, -a, 0))


def mirrors(m, x, y, z):
    for s in (-1, 1):
        block(f"mirror{s}", (x, s * y, z), (0.1, 0.12, 0.07), m["chrome"], bevel=0.02)


def finish(name, length):
    scale = LENGTH / length
    for o in bpy.context.scene.objects:
        if o.type == "MESH":
            o.location *= scale
            o.scale *= scale
    export(name)


# --- Porsche 911 (1973 Carrera RS) -------------------------------------------

def porsche(name, paint, stripe, roof, number, sponsor, ink, bands=()):
    reset()
    m = palette(paint, stripe, roof)
    front, rear = 1.18, -1.12
    r = 0.31

    def body(x):
        hw = 0.78 + 0.05 * bump(x, rear, 0.55) + 0.025 * bump(x, front, 0.45)
        hw *= (1 - smooth_step(1.75, 2.1, x) * 0.32) * (1 - smooth_step(-1.8, -2.1, x) * 0.22)
        zt = piecewise([(-2.08, 0.66), (-1.7, 0.82), (-1.1, 0.9), (0.6, 0.86), (1.25, 0.78), (2.08, 0.58)], x)
        # Front wings stand proud of the bonnet: the round headlamps sit in them.
        zb = 0.24 + 0.40 * max(bump(x, front, 0.42), bump(x, rear, 0.42))
        zb = max(zb, piecewise([(-2.1, 0.36), (-1.9, 0.24), (1.9, 0.24), (2.1, 0.34)], x))
        return hw, zb, zt, 2.6, 0.96

    def cabin(x):
        # Fastback: the roof runs in one curve from the windscreen into the engine lid.
        zt = piecewise([(0.62, 0.84), (0.0, 1.27), (-0.45, 1.29), (-1.05, 1.08), (-1.75, 0.86)], x)
        hw = piecewise([(0.62, 0.66), (0.0, 0.7), (-0.6, 0.7), (-1.75, 0.6)], x)
        return hw, 0.78, zt, 2.4, 0.72

    loft("body", -2.08, 2.08, body, m["paint"])
    loft("cabin", -1.75, 0.62, cabin, m["glass"])
    decal("roof", [cabin], -0.62, 0.0, -0.45, 0.45, m["roof"], lift=0.008)
    decal("lid", [body, cabin], -1.95, -1.05, -0.5, 0.5, m["paint"], lift=0.006)
    for y in (-0.16, 0.16):
        decal(f"stripe{y}", [body, cabin], -2.0, 2.02, y - 0.07, y + 0.07, m["stripe"], lift=0.016, across=2)
    # Ducktail.
    block("ducktail", (-1.95, 0, 0.9), (0.28, 1.2, 0.05), m["paint"], bevel=0.02, rot=(0, math.radians(-14), 0))
    # Round headlamps in the wings, chrome rings.
    for s in (-1, 1):
        cyl(f"lamp{s}", (1.83, s * 0.56, 0.7), 0.11, 0.06, m["lamp"], axis="X", verts=18)
        cyl(f"ring{s}", (1.8, s * 0.56, 0.7), 0.125, 0.05, m["chrome"], axis="X", verts=18)
        block(f"tail{s}", (-2.06, s * 0.5, 0.7), (0.04, 0.32, 0.08), m["tail"])
        # A side stripe and a door roundel.
        block(f"sidestripe{s}", (-0.2, s * 0.835, 0.5), (2.6, 0.01, 0.06), m["stripe"])
        cyl(f"roundel{s}", (0.05, s * 0.84, 0.6), 0.16, 0.01, m["white"], verts=20)
    block("bumper-f", (2.05, 0, 0.38), (0.08, 1.3, 0.08), m["black"], bevel=0.02)
    block("bumper-r", (-2.08, 0, 0.42), (0.06, 1.3, 0.08), m["black"], bevel=0.02)
    livery(m, body, cabin, -0.3, 1.45, 0.05, 0.6, number, sponsor, ink, bands)
    mirrors(m, 0.45, 0.82, 0.88)
    wheels(m, front, rear, 0.72, r, 0.24, rim_style="fuchs")
    finish(name, 4.16)


# --- Lancia Stratos HF ---------------------------------------------------------

def stratos(name, paint, stripe, roof, number, sponsor, ink, bands=()):
    reset()
    m = palette(paint, stripe, roof)
    front, rear = 1.1, -1.05
    r = 0.32

    def body(x):
        hw = 0.84 + 0.04 * bump(x, rear, 0.5) + 0.04 * bump(x, front, 0.45)
        hw *= (1 - smooth_step(1.55, 1.86, x) * 0.25) * (1 - smooth_step(-1.6, -1.86, x) * 0.12)
        # The wedge: a sharp low nose rising straight to the screen.
        zt = piecewise([(-1.86, 0.84), (-0.5, 0.86), (0.35, 0.8), (1.86, 0.5)], x)
        zb = 0.24 + 0.36 * max(bump(x, front, 0.42), bump(x, rear, 0.42))
        zb = max(zb, piecewise([(-1.9, 0.32), (-1.7, 0.24), (1.6, 0.22), (1.86, 0.3)], x))
        return hw, zb, zt, 3.0, 0.97

    def cabin(x):
        # The wraparound visor: a near-hemispherical screen and a short roof.
        zt = piecewise([(0.62, 0.78), (0.1, 1.08), (-0.4, 1.11), (-0.75, 0.9)], x)
        hw = piecewise([(0.62, 0.55), (0.1, 0.74), (-0.4, 0.74), (-0.75, 0.7)], x)
        return hw, 0.74, zt, 2.2, 0.8

    loft("body", -1.86, 1.86, body, m["paint"])
    loft("cabin", -0.75, 0.62, cabin, m["glass"])
    decal("roof", [cabin], -0.62, -0.05, -0.5, 0.5, m["roof"], lift=0.008)
    # Alitalia-style sweep: one broad and one thin band over the car.
    decal("band", [body, cabin], -1.84, 1.84, -0.3, 0.05, m["stripe"], lift=0.016, across=3)
    decal("pin", [body, cabin], -1.84, 1.84, 0.12, 0.2, m["stripe"], lift=0.016, across=2)
    # Engine-cover louvres.
    for k in range(6):
        block(f"louvre{k}", (-1.0 - k * 0.11, 0, 0.87), (0.05, 1.0, 0.025), m["black"])
    # Roof and tail spoilers.
    block("roofwing", (-0.72, 0, 1.0), (0.12, 1.2, 0.03), m["roof"], rot=(0, math.radians(-12), 0))
    block("tailwing", (-1.8, 0, 0.94), (0.2, 1.6, 0.03), m["black"], rot=(0, math.radians(-8), 0))
    for s in (-1, 1):
        block(f"strut{s}", (-1.78, s * 0.6, 0.88), (0.12, 0.04, 0.12), m["black"])
        cyl(f"tail{s}", (-1.86, s * 0.6, 0.66), 0.08, 0.03, m["tail"], axis="X", verts=14)
        cyl(f"roundel{s}", (0.0, s * 0.87, 0.55), 0.15, 0.01, m["white"], verts=20)
        block(f"skirt{s}", (0, s * 0.85, 0.28), (1.4, 0.03, 0.06), m["black"])
    # The rally light pod on the nose.
    block("pod", (1.12, 0, 0.72), (0.12, 0.9, 0.16), m["black"], bevel=0.03)
    for y in (-0.33, -0.11, 0.11, 0.33):
        cyl(f"spot{y}", (1.19, y, 0.72), 0.075, 0.03, m["lamp"], axis="X", verts=16)
    livery(m, body, cabin, -0.33, 0.85, 0.0, 0.55, number, sponsor, ink, bands)
    mirrors(m, 0.38, 0.84, 0.86)
    wheels(m, front, rear, 0.76, r, 0.26)
    finish(name, 3.71)


# --- Ford Escort Mk1 RS ------------------------------------------------------

def escort(name, paint, stripe, roof, number, sponsor, ink, bands=()):
    reset()
    m = palette(paint, stripe, roof)
    front, rear = 1.22, -1.18
    r = 0.3

    def body(x):
        hw = 0.76 + 0.07 * max(bump(x, rear, 0.42), bump(x, front, 0.42))
        hw *= (1 - smooth_step(1.8, 2.0, x) * 0.1) * (1 - smooth_step(-1.8, -2.0, x) * 0.08)
        # Three boxes: flat bonnet, upright cabin, flat boot.
        zt = piecewise([(-2.0, 0.84), (-1.2, 0.88), (0.7, 0.86), (1.75, 0.82), (2.0, 0.72)], x)
        zb = 0.26 + 0.36 * max(bump(x, front, 0.4), bump(x, rear, 0.4))
        zb = max(zb, piecewise([(-2.02, 0.34), (-1.85, 0.26), (1.85, 0.26), (2.02, 0.34)], x))
        return hw, zb, zt, 4.5, 0.98

    def cabin(x):
        zt = piecewise([(0.78, 0.84), (0.3, 1.33), (-0.75, 1.35), (-1.25, 0.88)], x)
        hw = piecewise([(0.78, 0.7), (0.3, 0.7), (-0.75, 0.7), (-1.25, 0.7)], x)
        return hw, 0.8, zt, 3.6, 0.8

    loft("body", -2.0, 2.0, body, m["paint"])
    loft("cabin", -1.25, 0.78, cabin, m["glass"])
    decal("roof", [cabin], -0.75, 0.25, -0.5, 0.5, m["roof"], lift=0.008)
    # A single broad stripe over bonnet, roof and boot, and side flashes.
    decal("stripe", [body, cabin], -1.98, 1.98, -0.22, 0.22, m["stripe"], lift=0.016, across=3)
    for s in (-1, 1):
        block(f"flash{s}", (0.1, s * 0.84, 0.62), (2.8, 0.01, 0.1), m["stripe"])
        cyl(f"roundel{s}", (-0.1, s * 0.845, 0.52), 0.15, 0.01, m["white"], verts=20)
        # Twin round headlamps per side in a black grille.
        for y in (0.28, 0.55):
            cyl(f"lamp{s}{y}", (2.0, s * y, 0.66), 0.08, 0.04, m["lamp"], axis="X", verts=16)
        block(f"tail{s}", (-2.0, s * 0.55, 0.68), (0.04, 0.22, 0.12), m["tail"])
        block(f"flap{s}", (-1.55, s * 0.78, 0.18), (0.02, 0.2, 0.22), m["black"])
    block("grille", (2.0, 0, 0.62), (0.04, 0.9, 0.16), m["black"])
    block("bumper-f", (2.04, 0, 0.42), (0.06, 1.5, 0.06), m["chrome"], bevel=0.02)
    block("bumper-r", (-2.03, 0, 0.44), (0.06, 1.5, 0.06), m["chrome"], bevel=0.02)
    # Four spotlights on a bar and a roof vent.
    block("bar", (2.07, 0, 0.5), (0.03, 1.2, 0.03), m["black"])
    for y in (-0.48, -0.16, 0.16, 0.48):
        cyl(f"spot{y}", (2.12, y, 0.56), 0.085, 0.07, m["black"], axis="X", verts=16)
        cyl(f"glass{y}", (2.16, y, 0.56), 0.07, 0.02, m["lamp"], axis="X", verts=16)
    block("vent", (-0.1, 0, 1.36), (0.3, 0.3, 0.05), m["black"], bevel=0.02)
    livery(m, body, cabin, -0.25, 1.35, -0.1, 0.52, number, sponsor, ink, bands)
    mirrors(m, 0.55, 0.8, 0.9)
    wheels(m, front, rear, 0.7, r, 0.24)
    finish(name, 4.0)


os.makedirs(OUT, exist_ok=True)
# Grid order (game's DRIVERS): Juno, Mika, Rook, Ayla, You, Vex. The sponsors are invented.
porsche("car-0", 0x22252B, 0xD4A93A, 0x22252B, "1", "AURUM", 0xD4A93A)            # Juno: black and gold
stratos("car-1", 0x7B45C8, 0xF4F1EA, 0xF4F1EA, "4", "NOVA", 0x22252B,
        bands=[(0.32, 0.4, 0xF2C230)])                                           # Mika: purple, white sweep
escort("car-2", 0xF4F1EA, 0xE8742A, 0xF4F1EA, "22", "CASTELLO", 0x1F7A3A,
       bands=[(0.26, 0.34, 0xD83A2E), (0.36, 0.42, 0x1F7A3A)])                  # Rook: white, red/green/orange
escort("car-3", 0x3FA7D6, 0xF4F1EA, 0x1E4F80, "9", "MARINA", 0x1E2A44,
       bands=[(-0.42, -0.34, 0x1E4F80), (-0.32, -0.26, 0xD83A2E)])              # Ayla: Martini-ish blues and red
porsche("car-4", 0xF2C230, 0x22252B, 0xF2C230, "7", "VELOX", 0x22252B,
        bands=[(-0.45, -0.38, 0xD83A2E), (0.38, 0.45, 0xD83A2E)])               # You: yellow, black and red
stratos("car-5", 0xF4F1EA, 0xD83A2E, 0xF4F1EA, "12", "ALTIVA", 0x1F7A3A,
        bands=[(-0.36, -0.31, 0x1F7A3A)])                                         # Vex: Alitalia-style
print("cars written to", OUT)
