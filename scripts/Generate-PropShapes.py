"""Freeze reviewed original AST shapes; this does not derive handler semantics."""
from pathlib import Path
import json
import subprocess
import sys

root=Path(__file__).resolve().parents[1]
report=json.loads((root/"analysis/reports/prop-state-dependencies.json").read_text())
assert report["errors"]==0
text="// Generated reviewed AST shapes. See scripts/Generate-PropShapes.py.\n"
def flatten(expressions):
    for e in expressions:
        yield e
        yield from flatten(e["children"])
def operand(o):
    kind=o["kind"]
    value=o.get("value")
    prefix="ExpectedOperand::"+kind
    if kind=="None":return prefix
    if kind=="Object":return prefix+f'({value["index"]},'+json.dumps(value["path"])+')'
    if kind=="Context":return prefix+f'({value["skip"]},{value["result_size"]})'
    return prefix+'('+json.dumps(value)+')'
anim_report=json.loads((root/"analysis/reports/anim-prop-dependencies.json").read_text())
assert anim_report["errors"]==0
for name,path,source in (("INVULNERABLE","Prop.Invulnerable.BeginState",report),("DAMAGABLE","Prop.Damagable.BeginState",report),("ANIM_INVULNERABLE","AnimProp.Invulnerable.BeginState",anim_report)):
    f=next(e["decoded"] for e in source["functions"] if e["path"]==path)
    text+=f"const {name}: &[Shape] = &[\n"
    for e in flatten(f["expressions"]):
        text+=(f'Shape {{opcode:{e["opcode"]},start:{e["logical_offset"]},end:{e["logical_end"]},'
               f'serialized:{e["serialized_offset"]},children:{len(e["children"])},operand:{operand(e["operand"])} }},\n')
    text+="];\n"
formatted=subprocess.run(["rustfmt","--edition","2021","--emit","stdout"],input=text,text=True,capture_output=True,check=True).stdout
path=root/"crates/rc-package/src/prop_begin_shapes.rs"
if sys.argv[1:]==["--write"]:
    path.write_text(formatted,encoding="utf-8")
else:
    assert not sys.argv[1:]
    assert path.read_text()==formatted,"generated AST shape differs"
print("Prop AST shapes verified")
