# /// script
# requires-python = ">=3.10"
# dependencies = ["UnityPy>=1.20", "Pillow"]
# ///
"""Export the UI kit sse needs (`--ui`) from a Project Sekai client you own.

The talk window sprites, the transition textures, the fonts and the `fx_transition_scenario`
particle prefab ship inside the app (not on the CDN), so SekaiStoryRipper cannot fetch them.
This script reads them from the client's `Data` and writes:

    <out>/<sprite>.png                  talk window and menu sprites
    <out>/tex_common_tri_01.png         transition triangle atlas
    <out>/tex_transition_{top,left}.png side fade edges
    <out>/holo.png                      hologram character shader scan lines
    <out>/FOT-RodinNTLGPro-{DB,EB}.otf  the fonts the talk window uses
    <out>/fx_transition_scenario.json   the transition particle systems (`sse-fx` v1)

Usage:
    uv run tools/ui-kit/extract.py <client> <out-dir>

`<client>` is an `.ipa`, a `.app` directory, its `Data` directory, or a `data.unity3d`.
Nothing is downloaded and nothing leaves your machine.
"""

import argparse
import json
import sys
import tempfile
import zipfile
from pathlib import Path

import UnityPy

# Sprites drawn by sse's native UI. When a name exists in several atlases the scenario screen
# uses the first matching tag (e.g. its menu icon comes from `ScenarioAtlas`).
SPRITES = [
    "bg_story_adv",
    "bg_base_half_r8_wh",
    "bg_base_round_h48_wh",
    "icon_triangle_h22_wh",
    "btn_circle_h80_wh",
    "icon_menu_story_wh",
    "btn_round_h80_wh",
]
ATLAS_PREFERENCE = ["ScenarioAtlas", "CommonAtlas"]
# Full textures (not sprites); the largest texture of each name is the one in use.
TEXTURES = ["tex_common_tri_01", "tex_transition_top", "tex_transition_left", "holo"]
FONTS = ["FOT-RodinNTLGPro-DB", "FOT-RodinNTLGPro-EB"]
FX_PREFAB = "fx_transition_scenario"


def load(client: Path, tmp: Path):
    """UnityPy environment for any of the accepted client layouts."""
    if client.suffix == ".ipa":
        with zipfile.ZipFile(client) as z:
            members = [m for m in z.namelist() if "/Data/" in m and m.startswith("Payload/")]
            if not members:
                sys.exit(f"{client}: no Payload/*.app/Data in this ipa")
            z.extractall(tmp, members)
        client = next(tmp.glob("Payload/*.app/Data"))
    if client.is_dir() and (client / "Data").is_dir():
        client = client / "Data"
    if client.is_dir() and (client / "data.unity3d").is_file():
        client = client / "data.unity3d"
    print(f"loading {client} ...", file=sys.stderr)
    return UnityPy.load(str(client))


def by_name(env, types):
    out = {}
    for o in env.objects:
        if o.type.name in types:
            out.setdefault(o.peek_name(), []).append(o)
    return out


def atlas_rank(obj):
    tags = obj.read_typetree().get("m_AtlasTags") or []
    for i, tag in enumerate(ATLAS_PREFERENCE):
        if tag in tags:
            return i
    return len(ATLAS_PREFERENCE)


def export_images(env, out: Path):
    found = by_name(env, {"Sprite", "Texture2D"})
    missing = []
    for name in SPRITES:
        sprites = [o for o in found.get(name, []) if o.type.name == "Sprite"]
        if not sprites:
            missing.append(name)
            continue
        best = min(sprites, key=lambda o: (atlas_rank(o), o.path_id))
        best.read().image.save(out / f"{name}.png")
    for name in TEXTURES:
        textures = [o.read() for o in found.get(name, []) if o.type.name == "Texture2D"]
        if not textures:
            missing.append(name)
            continue
        best = max(textures, key=lambda t: t.m_Width * t.m_Height)
        best.image.save(out / f"{name}.png")
    return missing


def export_fonts(env, out: Path):
    found = by_name(env, {"Font"})
    missing = []
    for name in FONTS:
        if name not in found:
            missing.append(name)
            continue
        data = bytes(found[name][0].read_typetree()["m_FontData"])
        (out / f"{name}.otf").write_bytes(data)
    return missing


# ---- fx_transition_scenario -> sse-fx v1 -------------------------------------------------


def minmax_curve(m):
    """`MinMaxCurve`; curve modes carry their multiplier in `scalar`."""
    state = m["minMaxState"]
    if state == 0:
        return {"Const": m["scalar"]}
    if state == 3:
        return {"RandConst": [m["minScalar"], m["scalar"]]}
    if state == 1:
        return {"Curve": keys(m["maxCurve"], m["scalar"])}
    if state == 2:
        return {"TwoCurves": [keys(m["minCurve"], m["scalar"]), keys(m["maxCurve"], m["scalar"])]}
    sys.exit(f"unsupported MinMaxCurve state {state}")


def keys(curve, scale):
    """Hermite keys as [time, value, inSlope, outSlope]."""
    out = []
    for k in curve["m_Curve"]:
        if k.get("weightedMode", 0) != 0:
            sys.exit("weighted tangents are not supported")
        out.append([k["time"], k["value"] * scale, k["inSlope"] * scale, k["outSlope"] * scale])
    return out


def gradient(g):
    colors = [
        [g[f"ctime{i}"] / 65535, g[f"key{i}"]["r"], g[f"key{i}"]["g"], g[f"key{i}"]["b"]]
        for i in range(g["m_NumColorKeys"])
    ]
    alphas = [[g[f"atime{i}"] / 65535, g[f"key{i}"]["a"]] for i in range(g["m_NumAlphaKeys"])]
    return {"colors": colors, "alphas": alphas}


def xyz(v):
    return [v["x"], v["y"], v["z"]]


class Prefab:
    def __init__(self, env, root):
        self.file = root.assets_file
        self.env = env

    def obj(self, ptr):
        if ptr["m_FileID"] != 0:
            return None
        return self.file.objects.get(ptr["m_PathID"])

    def components(self, go):
        return {c.type.name: c for c in (self.obj(p["component"]) for p in go.read_typetree()["m_Component"]) if c}

    def walk(self, go, path):
        """Pre-order over the hierarchy, children in `m_Children` order."""
        comps = self.components(go)
        yield path, comps
        transform = comps.get("Transform") or comps.get("RectTransform")
        for child in transform.read_typetree()["m_Children"]:
            child_go = self.obj(self.obj(child).read_typetree()["m_GameObject"])
            yield from self.walk(child_go, f"{path}/{child_go.peek_name()}")

    def material(self, renderer):
        mat = self.obj(renderer["m_Materials"][0])
        shader = mat.read_typetree()["m_Shader"]
        name = shader_name(self.env, mat, shader)
        return "Additive" if "Additive" in name else "AlphaBlended"


def shader_name(env, mat, ptr):
    file = mat.assets_file
    if ptr["m_FileID"] == 0:
        obj = file.objects[ptr["m_PathID"]]
    else:
        ext = file.externals[ptr["m_FileID"] - 1].path.split("/")[-1].lower()
        obj = next(
            f.objects[ptr["m_PathID"]]
            for f in all_files(env)
            if f.name.lower() == ext and ptr["m_PathID"] in f.objects
        )
    return obj.read().m_ParsedForm.m_Name


def all_files(env):
    stack = list(env.files.values())
    while stack:
        f = stack.pop()
        if hasattr(f, "objects"):
            yield f
        if hasattr(f, "files"):
            stack.extend(f.files.values())


def emitter(name, comps, material):
    ps = comps["ParticleSystem"].read_typetree()
    r = comps["ParticleSystemRenderer"].read_typetree()
    t = (comps.get("Transform") or comps.get("RectTransform")).read_typetree()
    init, shape, em = ps["InitialModule"], ps["ShapeModule"], ps["EmissionModule"]
    vel, noise, clamp, rot = ps["VelocityModule"], ps["NoiseModule"], ps["ClampVelocityModule"], ps["RotationModule"]
    return {
        "name": name,
        "material": material,
        "sorting_order": r["m_SortingOrder"],
        "tile": ps["UVModule"]["startFrame"]["scalar"] * 16.0,
        "position": xyz(t["m_LocalPosition"]),
        "lifetime": minmax_curve(init["startLifetime"]),
        "speed": minmax_curve(init["startSpeed"]),
        "size": minmax_curve(init["startSize"]),
        "rotation": minmax_curve(init["startRotation"]),
        "rotation_x": minmax_curve(init["startRotationX"]),
        "rotation_y": minmax_curve(init["startRotationY"]),
        "color": gradient(ps["ColorModule"]["gradient"]["maxGradient"]),
        "gravity": minmax_curve(init["gravityModifier"]),
        "max_particles": init["maxNumParticles"],
        "shape": {
            "angle": shape["angle"],
            "radius": shape["radius"]["value"],
            "rotation": xyz(shape["m_Rotation"]),
            "scale": xyz(shape["m_Scale"]),
            "position": xyz(shape["m_Position"]),
        },
        "bursts": [
            {
                "time": b["time"],
                "count": minmax_curve(b["countCurve"]),
                "cycles": b["cycleCount"],
                "interval": b["repeatInterval"],
            }
            for b in em["m_Bursts"]
        ],
        "velocity": {
            "x": minmax_curve(vel["x"]),
            "y": minmax_curve(vel["y"]),
            "z": minmax_curve(vel["z"]),
            "speed_modifier": minmax_curve(vel["speedModifier"]),
        },
        "angular_velocity": [
            {"RandConst": [rot["x"]["minScalar"], rot["x"]["scalar"]]},
            {"RandConst": [rot["y"]["minScalar"], rot["y"]["scalar"]]},
            {"RandConst": [rot["curve"]["minScalar"], rot["curve"]["scalar"]]},
        ],
        "noise": {
            "strength": noise["strength"]["scalar"],
            "frequency": noise["frequency"],
            "octaves": noise["octaves"],
            "damping": bool(noise["damping"]),
        },
        "clamp": (
            {"limit": minmax_curve(clamp["magnitude"]), "dampen": clamp["dampen"]}
            if clamp["enabled"]
            else None
        ),
        "length": ps["lengthInSec"],
        "auto_seed": bool(ps["autoRandomSeed"]),
        "random_seed": ps["randomSeed"],
    }


def export_fx(env, out: Path):
    root = next(
        (o for o in env.objects if o.type.name == "GameObject" and o.peek_name() == FX_PREFAB),
        None,
    )
    if root is None:
        return [FX_PREFAB]
    prefab = Prefab(env, root)
    emitters = []
    for path, comps in prefab.walk(root, f"/{FX_PREFAB}"):
        if "ParticleSystem" not in comps:
            continue
        if not comps["ParticleSystem"].read_typetree()["EmissionModule"]["enabled"]:
            continue
        name = path.split(f"/{FX_PREFAB}/root/")[-1]
        material = prefab.material(comps["ParticleSystemRenderer"].read_typetree())
        emitters.append(emitter(name, comps, material))
    doc = {"format": "sse-fx", "version": 1, "emitters": emitters}
    (out / f"{FX_PREFAB}.json").write_text(json.dumps(doc, indent=1) + "\n")
    return []


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("client", type=Path, help=".ipa, .app, Data directory or data.unity3d")
    ap.add_argument("out", type=Path, help="output directory (pass it to sse as --ui)")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        env = load(args.client, Path(tmp))
        missing = export_images(env, args.out) + export_fonts(env, args.out) + export_fx(env, args.out)
    for name in missing:
        print(f"warning: {name} not found in this client", file=sys.stderr)
    print(f"wrote {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
