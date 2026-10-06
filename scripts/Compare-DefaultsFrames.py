"""Verify the newly resolved entry.ctm default anchor in the existing Android viewer."""
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
    "anchor_changed_pixels": changed("defaults-overview", "defaults-anchor"),
    "move_changed_pixels": changed("defaults-anchor", "defaults-moved"),
    "return_changed_pixels": changed("defaults-anchor", "defaults-returned"),
    "reset_changed_pixels": changed("defaults-overview", "defaults-reset"),
    "recovery_changed_pixels": changed("defaults-overview", "defaults-recovery"),
}
assert result["anchor_changed_pixels"] > 1000 and result["move_changed_pixels"] > 1000
assert all(result[k] == 0 for k in ("return_changed_pixels", "reset_changed_pixels", "recovery_changed_pixels"))
assert not (root / "defaults-crash-log.txt").read_text(encoding="utf-8-sig").strip()
result["crash_buffer_bytes"] = 0
(root / "defaults-validation.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
frame("defaults-anchor").save(root / "defaults-render.png")
print(json.dumps(result, indent=2))
