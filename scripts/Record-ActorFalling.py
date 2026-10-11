"""Replay AActor::physFalling SSE body with prepared GetNetBuoyancy/volume values.
No MoveActor callbacks, volume lookup, landing or complete-engine execution claim.
"""
import hashlib
import json
import random
import re
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission4-flight'
BINARY = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/engine.dll')
raw = BINARY.read_bytes()
pe = struct.unpack_from('<I', raw, 60)[0]
base = struct.unpack_from('<I', raw, pe + 52)[0]
optional = struct.unpack_from('<H', raw, pe + 20)[0]
sections = [struct.unpack_from('<IIII', raw, pe + 24 + optional + 40*i + 8)
            for i in range(struct.unpack_from('<H', raw, pe + 6)[0])]

def word(address):
    for _, va, count, offset in sections:
        if va <= address-base < va+count:
            return struct.unpack_from('<I', raw, offset+address-base-va)[0]
    raise AssertionError(hex(address))

def bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]

def number(value):
    return struct.unpack('<f', struct.pack('<I', value))[0]

assert word(0x10653578) == bits(1.)
assert word(0x10665bbc) == bits(.05)
assert word(0x1065efc4) == bits(.5)
assembly = OUT / 'falling-instructions.txt'
code = {int(a, 16): (op, operands.split(','))
        for a, op, operands in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+) (.*)$', assembly.read_text(), re.M)}

def replay(c):
    stack, actor, volume, acceleration = 0x20000000, 0x30000000, 0x40000000, 0x50000000
    registers = dict(EBP=stack, ESI=actor, EDI=actor+0x150)
    memory = {actor+0x134: volume, stack-0xb0: acceleration}
    for start, values in [(stack-0x2c, c['velocity']), (acceleration, c['acceleration']),
                          (volume+0x308, c['gravity']), (volume+0x2fc, c['zone_velocity'])]:
        memory.update({start+4*i: v for i, v in enumerate(values)})
    memory.update({stack-0xa4: c['buoyancy'], stack-0xac: c['fluid_friction'],
                   actor+0x200: c['mass'], stack+8: c['dt']})

    def address(operand):
        terms = operand[operand.index('[')+1:operand.index(']')].split(' + ')
        return sum(registers[t] if t in registers else int(t, 0) for t in terms) & 0xffffffff

    def get(operand):
        if '[' not in operand:
            return registers[operand]
        at = address(operand)
        return word(at) if base <= at < base+0x1000000 else memory[at]

    def put(operand, value):
        if '[' in operand:
            memory[address(operand)] = value
        else:
            registers[operand] = value

    count = 0
    for pc in sorted(code):
        if not 0x10490c63 <= pc <= 0x10490d71:
            continue
        op, (left, right) = code[pc]
        if op in ['MOV', 'MOVSS', 'MOVAPS']:
            put(left, get(right))
        elif op in ['ADDSS', 'SUBSS', 'MULSS', 'DIVSS']:
            a, b = number(get(left)), number(get(right))
            value = {'ADDSS': lambda: a+b, 'SUBSS': lambda: a-b,
                     'MULSS': lambda: a*b, 'DIVSS': lambda: a/b}[op]()
            put(left, bits(value))
        else:
            raise AssertionError((hex(pc), op))
        count += 1
    return dict(velocity=[memory[actor+0x150+4*i] for i in range(3)],
                displacement=[registers[r] for r in ['XMM3', 'XMM2', 'XMM1']], instructions=count)

rng = random.Random(0x10490a30)
cases = []
for i in range(4096):
    c = dict(velocity=[bits(rng.uniform(-8000.,8000.)) for _ in range(3)],
             acceleration=[bits(rng.uniform(-2000.,2000.)) for _ in range(3)],
             gravity=[bits(rng.uniform(-1500.,1500.)) for _ in range(3)],
             zone_velocity=[bits(rng.uniform(-300.,300.)) for _ in range(3)],
             buoyancy=bits(rng.choice([0.,50.,100.,200.])),
             mass=bits(rng.choice([10.,100.,150.,500.])),
             fluid_friction=bits(rng.choice([0.,.3,1.,2.])),
             dt=bits(rng.choice([1./60.,.05,.04,.01,0.])))
    c['expected'] = replay(c)
    cases.append(c)
result = dict(scope=__doc__, binary_sha256=hashlib.sha256(raw).hexdigest(),
              assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(), cases=cases)
(OUT/'falling-oracle.json').write_bytes((json.dumps(result,indent=2)+'\n').encode())
print(f'Recorded {len(cases)} original actor falling instruction cases')
