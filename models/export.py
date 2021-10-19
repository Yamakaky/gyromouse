import bpy
import sys
from pathlib import Path

argv = sys.argv
argv = argv[argv.index("--") + 1 :]  # get all args after "--"

filename = Path(argv[0])

bpy.ops.wm.open_mainfile(filepath=str(filename))
bpy.ops.export_scene.gltf(
    filepath=str(filename.with_suffix(".gltf")),
    export_format="GLTF_EMBEDDED",
    export_yup=True,
)
