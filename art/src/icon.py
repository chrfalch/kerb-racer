# Kerb Racers' app icon: the player's yellow #7 sliding sideways over the
# apex kerb, tyre smoke behind it, rendered from the game's own car model.
#   blender -b -P art/src/icon.py
# Writes assets/icon/icon-1024.png: the track on a rounded tile with transparent
# corners and margin (the macOS grid: an 824 px tile in 1024), the car and its
# smoke rendered again on their own and laid over it, so they spill past the
# tile's edge. Then make the smaller sizes with
#   sips -z 512 512 assets/icon/icon-1024.png --out assets/icon/icon-512.png
#   sips -z 192 192 assets/icon/icon-1024.png --out assets/icon/icon-192.png
import bpy, math, os, random
import numpy as np
from mathutils import Vector, Euler

HERE = os.path.dirname(os.path.abspath(__file__))
A = os.path.join(HERE, "..", "models") + "/"
OUT = os.path.join(HERE, "..", "..", "assets", "icon", "icon-1024.png")
RES = 1024
bpy.ops.wm.read_factory_settings(use_empty=True)
sc=bpy.context.scene
def hexc(h,a=1):
    c=[((h>>s)&255)/255 for s in (16,8,0)]
    return [x**2.2 for x in c]+[a]
def mat(name,h,rough=0.6,emit=0):
    m=bpy.data.materials.new(name); m.use_nodes=True
    b=m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value=hexc(h); b.inputs["Roughness"].default_value=rough
    return m
def box(name,loc,size,m,rot=0,bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(location=loc)
    o=bpy.context.object; o.name=name; o.scale=[s/2 for s in size]; o.rotation_euler=(0,0,rot)
    bpy.ops.object.transform_apply(scale=True)
    if bevel:
        md=o.modifiers.new("b","BEVEL"); md.width=bevel; md.segments=3
    o.data.materials.append(m); return o

# car group
car=bpy.data.objects.new("CarRoot",None); sc.collection.objects.link(car)
def imp(n):
    bpy.ops.import_scene.gltf(filepath=A+n+".glb")
    return bpy.context.selected_objects[0]
body=imp("car-4"); body.parent=car
for x in (0.908,-0.862):
    for s,side in ((1,"l"),(-1,"r")):
        w=imp(f"wheel-fuchs-{side}"); w.location=(x,0.72*s,0.36); w.parent=car
        if x>0: w.rotation_euler=(0,0,math.radians(18))  # counter-steer
for o in car.children:
    o.select_set(False)
YAW=math.radians(24)
car.rotation_euler=(0,0,YAW)

# ground: asphalt, kerb along a line in direction of travel (+X), grass beyond
KY=0.25
asphalt=mat("asphalt",0x55575C,0.9); grass=mat("grass",0x4E9A34,0.9)
red=mat("red",0xD83A2E,0.5); white=mat("white",0xF4F1EA,0.5)
box("road",(0,KY-15,-0.05),(60,30,0.1),asphalt)
box("grass",(0,KY+2.1+15,-0.06),(60,30,0.1),grass)
seg=0.85
for i in range(-30,30):
    box(f"k{i}",(i*seg+seg/2,KY+0.45,0.03),(seg,0.9,0.12),red if i%2 else white,bevel=0.03)
# runoff red band on grass edge like the game
box("band",(0,KY+0.9+0.6,-0.04),(60,1.2,0.08),mat("band",0xB8432A,0.9))
# smoke puffs from rear wheels
smoke=bpy.data.materials.new("smoke"); smoke.use_nodes=True
bs=smoke.node_tree.nodes["Principled BSDF"]; bs.inputs["Base Color"].default_value=(1,1,1,1); bs.inputs["Roughness"].default_value=1
random.seed(3)
rear=Euler((0,0,YAW)).to_matrix()
for s in (1,-1):
    base=rear@Vector((-0.862,0.72*s,0.2))
    for k in range(5):
        r=0.14+k*0.07
        p=base+Vector((-0.5*k-0.3, -0.16*k+random.uniform(-.07,.07), r*0.8+0.05*k))
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3,radius=r,location=p)
        o=bpy.context.object; bpy.ops.object.shade_smooth(); o.data.materials.append(smoke)
        o.scale=(1.25,1,0.85)
for nm,loc,sc_ in (("daisies",(-0.6,3.0,0),1.2),("daisies",(4.6,3.2,0),1.0)):
    t=imp(nm); t.location=loc; t.scale=(sc_,)*3
root=bpy.data.objects.new("Root",None); sc.collection.objects.link(root)
for o in list(sc.objects):
    if o.parent is None and o is not root: o.parent=root
root.rotation_euler=(0,0,math.radians(22))
# light
bpy.ops.object.light_add(type='SUN',rotation=(math.radians(35),math.radians(-20),math.radians(-40)))
sun=bpy.context.object; sun.data.energy=4.2; sun.data.angle=math.radians(4)
w=bpy.data.worlds.new("w"); sc.world=w; w.use_nodes=True
w.node_tree.nodes["Background"].inputs[0].default_value=(0.75,0.85,1,1); w.node_tree.nodes["Background"].inputs[1].default_value=0.7
# camera
cd=bpy.data.cameras.new("cam"); cd.lens=80
cam=bpy.data.objects.new("cam",cd); sc.collection.objects.link(cam); sc.camera=cam
target=Vector((-0.75,-0.1,0.4))
cam.location=target+Vector((-2.5,-8.5,8.5))
d=target-cam.location; cam.rotation_euler=d.to_track_quat('-Z','Y').to_euler()
sc.render.engine='CYCLES'; sc.cycles.device='CPU'; sc.cycles.samples=256; sc.cycles.use_denoising=True
sc.render.resolution_x=sc.render.resolution_y=RES
sc.view_settings.view_transform='Standard'
sc.render.image_settings.color_mode='RGBA'
TMP = os.path.join(bpy.app.tempdir or "/tmp", "kr-icon-")

def render(name):
    sc.render.filepath = TMP + name + ".png"
    bpy.ops.render.render(write_still=True)
    img = bpy.data.images.load(sc.render.filepath)
    px = np.array(img.pixels[:], dtype=np.float32).reshape(RES, RES, 4)
    bpy.data.images.remove(img)
    return px

# The whole scene for the tile, then the car and smoke alone: the ground is a
# holdout, so only what stands on it is left, on transparency.
tile = render("tile")
sc.render.film_transparent = True
for o in sc.objects:
    if o.name.startswith(("road", "grass", "band", "k", "daisies")):
        o.is_holdout = True
car = render("car")

# The tile's mask: a rounded square, antialiased over a pixel, and a soft
# shadow under it (rows run bottom-up, so "down" is a smaller row).
def rounded(size, radius, cx, cy, soft):
    y, x = np.mgrid[0:RES, 0:RES].astype(np.float32) + 0.5
    qx = np.maximum(np.abs(x - cx) - (size / 2 - radius), 0)
    qy = np.maximum(np.abs(y - cy) - (size / 2 - radius), 0)
    d = np.hypot(qx, qy) - radius
    return np.clip(0.5 - d / soft, 0, 1)

S, R, C = 824, 185, RES / 2
mask = rounded(S, R, C, C, 1.0)
shadow = rounded(S, R, C, C - 10, 36.0) * 0.32

def over(top, a, bottom, b):
    out_a = a + b * (1 - a)
    out = (top * a[..., None] + bottom * (b * (1 - a))[..., None]) / np.maximum(out_a, 1e-6)[..., None]
    return out, out_a

rgb, a = over(tile[..., :3], mask, np.zeros((RES, RES, 3), np.float32), shadow)
rgb, a = over(car[..., :3], car[..., 3], rgb, a)

out = bpy.data.images.new("icon", RES, RES, alpha=True)
out.pixels[:] = np.dstack([rgb, a]).ravel()
out.filepath_raw = OUT
out.file_format = "PNG"
out.save()
