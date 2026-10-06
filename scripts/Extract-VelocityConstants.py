"""Read velocity constants from the original PE32 engine.dll; optionally include Falling."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--falling", action="store_true", help="verify additional physFalling constants")
args = parser.parse_args()
data = args.binary.read_bytes()
if data[:2] != b"MZ":
    raise ValueError("not a PE file")
pe = struct.unpack_from("<I", data, 0x3C)[0]
if data[pe:pe + 4] != b"PE\0\0":
    raise ValueError("invalid PE signature")
count = struct.unpack_from("<H", data, pe + 6)[0]
optional = pe + 24
optional_size = struct.unpack_from("<H", data, pe + 20)[0]
if struct.unpack_from("<H", data, optional)[0] != 0x10B:
    raise ValueError("expected PE32")
base = struct.unpack_from("<I", data, optional + 28)[0]
sections = [struct.unpack_from("<IIII", data, optional + optional_size + i * 40 + 8)
            for i in range(count)]
constants = []
expected_constants = [(0x1065EFC8, 3.0), (0x1065EFC4, 0.5),
                      (0x106701B4, 0.125), (0x10653578, 1.0)]
if args.falling:
    expected_constants += [(0x10665BBC, 0.05), (0x10668E18, 10.0), (0x1065EFCC, 0.0)]
for address, expected in expected_constants:
    rva = address - base
    candidates = [raw + rva - start for _, start, size, raw in sections
                  if start <= rva and rva + 4 <= start + size]
    if len(candidates) != 1:
        raise ValueError(f"unmapped/ambiguous VA {address:x}")
    offset = candidates[0]
    value = struct.unpack_from("<f", data, offset)[0]
    if value != struct.unpack("<f", struct.pack("<f", expected))[0]:
        raise ValueError(f"unexpected value at {address:x}: {value}")
    constants.append({"va": f"{address:08x}", "file_offset": offset,
                      "bytes": data[offset:offset + 4].hex(), "f32": value})
report = {"binary": str(args.binary.resolve()), "sha256": hashlib.sha256(data).hexdigest(),
          "function": "10496dc0 APawn::physFalling" if args.falling else "1048f900 APawn::calcVelocity", "constants": constants,
          "scope": "Values read directly from PE sections; offsets apply to this binary only."}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(f"{len(constants)} native velocity constants verified")
