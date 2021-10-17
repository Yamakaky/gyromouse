import bpy
import sys
from pathlib import Path

argv = sys.argv
argv = argv[argv.index("--") + 1 :]  # get all args after "--"

filename = Path(argv[0])

bpy.ops.wm.open_mainfile(filepath=str(filename))
bpy.ops.export_scene.obj(
    filepath=str(filename.with_suffix(".obj")),
    axis_forward="-Z",
    axis_up="Y",
    path_mode="COPY",
)
