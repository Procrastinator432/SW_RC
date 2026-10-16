"""Verify decisive original x86 instructions without Ghidra or third-party Python modules."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('engine',type=Path)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
data=a.engine.read_bytes()
assert data[:2]==b'MZ'
pe=struct.unpack_from('<I',data,60)[0]
assert data[pe:pe+4]==b'PE\0\0'
assert struct.unpack_from('<H',data,pe+24)[0]==0x10b
base=struct.unpack_from('<I',data,pe+24+28)[0]
count=struct.unpack_from('<H',data,pe+6)[0]
sections=pe+24+struct.unpack_from('<H',data,pe+20)[0]

def read(address,size):
    rva=address-base
    for i in range(count):
        vs,va,rs,rp=struct.unpack_from('<IIII',data,sections+i*40+8)
        if va<=rva and rva+size<=va+rs:
            return data[rp+rva-va:rp+rva-va+size]
    raise AssertionError(f'Address outside stored section: {address:x}')

checks=[]
for address,expected,meaning in [
    (0x104ea69c,'85d275050d00200000','execPlayOwnedSound: TEST bNoOverride; JNZ skips OR flags,0x2000'),
    (0x103d0da1,'8b48248b522433d1f7c2feffffff7518f6c10174ba',
     'FindSource: existing ID at [EAX+0x24]; XOR incoming; ignore bit0; TEST existing CL,1; protected existing source rejects incoming'),
]:
    expected=bytes.fromhex(expected)
    assert read(address,len(expected))==expected, f'Original instruction mismatch at {address:x}'
    checks.append(dict(address=f'{address:08x}',bytes=expected.hex(),meaning=meaning))
report=dict(passed=True,engine_sha256=hashlib.sha256(data).hexdigest(),image_base=f'{base:08x}',checks=checks,
            scope='Decisive original instruction operands/branches, not full audio-driver parity.')
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report))
