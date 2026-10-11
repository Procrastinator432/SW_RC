"""Replay the original SSE toss arithmetic with prepared pawn velocity/direction.
Unreal VM parameters and FRotator::Vector are explicit boundaries, not emulated.
"""
import hashlib
import json
import random
import re
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/mission4-detonator'
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

assert word(0x10673858) == bits(.4)
assembly = OUT / 'toss-instructions.txt'
code = {int(a, 16): (op, operands.split(','))
        for a, op, operands in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+) (.*)$', assembly.read_text(), re.M)}

def replay(direction, velocity, speed, toss_z):
    stack, actor, output = 0x20000000, 0x30000000, 0x40000000
    registers = dict(ESP=stack, EAX=actor)
    memory = {stack+0xc+4*i: bits(v) for i, v in enumerate(direction)}
    memory.update({stack+0x18+4*i: bits(v) for i, v in enumerate(velocity)})
    memory.update({stack+0x38: output, actor+0x284: bits(speed), actor+0x28c: bits(toss_z)})

    def address(operand):
        terms = operand[operand.index('[')+1:operand.index(']')].split(' + ')
        return sum(registers[t] if t in registers else int(t, 0) for t in terms)

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
        if not 0x104ed7e0 <= pc <= 0x104ed86c:
            continue
        op, operands = code[pc]
        if op == 'POP':
            continue  # saved nonvolatile register, irrelevant to the arithmetic
        left, right = operands
        if op in ['MOV', 'MOVSS', 'MOVAPS']:
            put(left, get(right))
        elif op in ['ADDSS', 'MULSS']:
            a, b = number(get(left)), number(get(right))
            put(left, bits(a+b if op == 'ADDSS' else a*b))
        else:
            raise AssertionError((hex(pc), op))
        count += 1
    return dict(velocity=[memory[output+4*i] for i in range(3)], instructions=count)

rng = random.Random(0x104ed730)
inputs = [([1.,0.,0.],[0.,0.,0.],2200.,200.),
          ([1.,0.,0.],[300.,800.,50.],2200.,200.),
          ([-1.,0.,0.],[300.,800.,50.],2200.,200.),
          ([0.,0.,1.],[100.,200.,300.],2200.,200.)]
for _ in range(2048):
    inputs.append(([rng.uniform(-1.,1.) for _ in range(3)],
                   [rng.uniform(-3000.,3000.) for _ in range(3)],
                   rng.choice([0.,2200.,3000.]), rng.choice([0.,200.,-100.])))
cases = []
for direction, velocity, speed, toss_z in inputs:
    direction = list(map(lambda v: number(bits(v)), direction))
    velocity = list(map(lambda v: number(bits(v)), velocity))
    cases.append(dict(direction=list(map(bits,direction)), pawn_velocity=list(map(bits,velocity)),
                      speed=bits(speed), toss_z=bits(toss_z), expected=replay(direction,velocity,speed,toss_z)))
result = dict(scope=__doc__, binary_sha256=hashlib.sha256(raw).hexdigest(),
              assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),
              inheritance_constant=dict(address='10673858', bits=word(0x10673858)), cases=cases)
(OUT/'toss-oracle.json').write_bytes((json.dumps(result,indent=2)+'\n').encode())
print(f'Recorded {len(cases)} original toss instruction cases')
