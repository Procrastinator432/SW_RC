"""Verify APawn exclusion identity and base controller predicate slots directly in PE32."""
from pathlib import Path
import hashlib
import json
import struct

root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes()
pe=struct.unpack_from("<I",data,60)[0]
assert data[:2]==b"MZ" and data[pe:pe+4]==b"PE\0\0"
optional=pe+24
assert struct.unpack_from("<H",data,optional)[0]==0x10b
base=struct.unpack_from("<I",data,optional+28)[0]
count=struct.unpack_from("<H",data,pe+6)[0]
size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+i*40+8) for i in range(count)]
def offset(address,length=4):
    rva=address-base
    found=[raw+rva-start for _,start,size,raw in sections if start<=rva and rva+length<=start+size]
    assert len(found)==1
    return found[0]
exports=json.loads((root/"analysis/reports/binaries/System__engine.dll.json").read_text())["exports"]
def export(name):
    found=[entry for entry in exports if entry["name"]==name]
    assert len(found)==1
    return int(found[0]["address"],16)
pawn_class=export("?PrivateStaticClass@APawn@@0VUClass@@A")
assert pawn_class==0x1075a270
asm=(root/"analysis/decompiled/pawn-hit-wall.asm").read_text()
assert "10490104 CMP EAX,0x1075a270" in asm and "1049010f MOV EAX,dword ptr [EAX + 0x30]" in asm
slots=[]
for table_name,function_name,expected_code,result in [
    ("??_7AController@@6B@","?IsAPlayerController@AActor@@UAEHXZ",bytes.fromhex("33c0c3"),False),
    ("??_7APlayerController@@6B@","?IsAPlayerController@APlayerController@@UAEHXZ",bytes.fromhex("b801000000c3"),True),
]:
    table=export(table_name)
    slot=table+0x1d0
    function=struct.unpack_from("<I",data,offset(slot))[0]
    assert function==export(function_name)
    code=data[offset(function,len(expected_code)):offset(function,len(expected_code))+len(expected_code)]
    assert code==expected_code
    slots.append({"vtable_symbol":table_name,"vtable_va":f"{table:08x}","slot":"0x1d0","slot_va":f"{slot:08x}","slot_file_offset":offset(slot),"function":function_name,"function_va":f"{function:08x}","bytes":code.hex(),"constant_return":result})
native=(root/"analysis/decompiled/pawn-human-controller.c").read_text()
assert "this + 0x41c" in native and "+ 0x1d0" in native
result={"date":"2026-10-06","engine_dll_sha256":hashlib.sha256(data).hexdigest(),"excluded_class":{"export":"?PrivateStaticClass@APawn@@0VUClass@@A","va":f"{pawn_class:08x}","predicate":"Actor class or ancestor equals APawn"},"human_function":"APawn.IsHumanControlled10480780","human_predicate":"Controller present and virtual IsAPlayerController result nonzero","verified_base_slots":slots,"scope":"Base exported controller vtables verified only. Derived runtime overrides and actor possession not reconstructed; human flag remains an explicit runtime snapshot."}
(root/"analysis/reports/hit-wall-class-controller-proof.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print("Verified APawn excluded-class symbol and two native controller predicate slots.")
