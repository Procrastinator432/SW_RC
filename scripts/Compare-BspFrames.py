"""Compare actual fitted render regions, excluding Android controls/status/time."""
from pathlib import Path
import json
import re
import xml.etree.ElementTree as ET
from PIL import Image

root = Path(__file__).resolve().parent.parent / "analysis" / "emulator"

def frame(name):
    node = ET.parse(root / f"{name}.xml").find(".//node[@class='android.widget.ImageView']")
    if node is None:
        raise AssertionError(f"Missing image view: {name}")
    left, top, right, bottom = map(int, re.findall(r"\d+", node.attrib["bounds"]))
    scale = min((right-left)/640, (bottom-top)/400)
    width, height = 640*scale, 400*scale
    x, y = (left+right-width)/2, (top+bottom-height)/2
    return Image.open(root / f"{name}.png").convert("RGB").crop(
        (round(x), round(y), round(x+width), round(y+height)))

def changed(first, second):
    a, b = frame(first), frame(second)
    assert a.size == b.size
    return sum(x != y for x, y in zip(a.get_flattened_data(), b.get_flattened_data()))

result = {
    "return_changed_pixels": changed("bsp-free", "bsp-returned"),
    "recovery_changed_pixels": changed("bsp-free", "bsp-recovery"),
}
assert all(value == 0 for value in result.values())
assert not (root / "bsp-crash-log.txt").read_text(encoding="utf-8-sig").strip()
result["crash_buffer_bytes"] = 0
result["states"] = ["outside/clear", "outside/blocked", "solid/blocked", "snapshot/no BSP"]
(root / "bsp-validation.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
print(json.dumps(result, indent=2))
