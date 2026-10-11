"""Bounded replay of exported original x86 decision instructions.

Only 1003a19a..1003a20c executes in this small interpreter. Capability bits,
offensive probability and MSVCRT rand results are supplied snapshots. No live
DLL execution, navigation, goal queue, animation, damage or Android claim.
"""
from pathlib import Path
import hashlib
import itertools
import json
import math
import re
import struct

root = Path(__file__).resolve().parent.parent
out = root / 'analysis/milestone3b'
out.mkdir(exist_ok=True)
assembly = root / 'analysis/decompiled/geonosian-attack.asm'
binary = Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/ctgame.dll')
raw = binary.read_bytes()
pe = struct.unpack_from('<I', raw, 0x3c)[0]
base = struct.unpack_from('<I', raw, pe + 52)[0]
optional = struct.unpack_from('<H', raw, pe + 20)[0]
sections = [struct.unpack_from('<IIII', raw, pe + 24 + optional + 40*i + 8)
            for i in range(struct.unpack_from('<H', raw, pe + 6)[0])]

def word(address):
    for size, va, count, offset in sections:
        if va <= address-base < va+count:
            return struct.unpack_from('<I', raw, offset+address-base-va)[0]
    raise AssertionError(hex(address))

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def floating(bits):
    return struct.unpack('<f', struct.pack('<I', bits))[0]

assert word(0x1004e130) == 0x38000100
assert word(0x1004ec74) == 0x3f4ccccd
code = {}
for address, operation, operands in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$', assembly.read_text(), re.M):
    a = int(address, 16)
    if 0x1003a19a <= a < 0x1003a20c:
        code[a] = (operation, operands.split(',') if operands else [])
addresses = sorted(code)
following = dict(zip(addresses, addresses[1:] + [0x1003a20c]))

def replay(mask, probability, random):
    registers = {'EBX': mask, 'EBP': 0, 'EDI': 0, 'EAX': 0}
    xmm = {'XMM0': 0., 'XMM1': 0.}
    zero = carry = False
    draws = 0
    pc = 0x1003a19a
    steps = 0
    def integer(operand):
        if operand == 'BH': return (registers['EBX'] >> 8) & 255
        if operand == 'BL': return registers['EBX'] & 255
        if operand in registers: return registers[operand]
        if operand == 'dword ptr [0x1004d870]': return word(0x1004d870)
        return int(operand, 0)
    def scalar(operand):
        if operand in xmm: return xmm[operand]
        if operand == 'dword ptr [ESI + 0x84]': return probability
        match = re.fullmatch(r'dword ptr \[(0x[0-9a-f]+)\]', operand)
        assert match, operand
        return floating(word(int(match[1], 16)))
    while pc != 0x1003a20c:
        steps += 1
        assert steps <= 40
        op, args = code[pc]
        next_pc = following[pc]
        if op == 'TEST':
            zero = integer(args[0]) & integer(args[1]) == 0
            carry = False
        elif op == 'MOV': registers[args[0]] = integer(args[1])
        elif op == 'MOVSS': xmm[args[0]] = scalar(args[1])
        elif op == 'CVTSI2SS': xmm[args[0]] = f32(integer(args[1]))
        elif op == 'MULSS': xmm[args[0]] = f32(xmm[args[0]] * scalar(args[1]))
        elif op == 'COMISS':
            a, b = scalar(args[0]), scalar(args[1])
            unordered = math.isnan(a) or math.isnan(b)
            zero, carry = unordered or a == b, unordered or a < b
        elif op == 'CALL':
            assert args == ['EDI'] and draws < len(random)
            registers['EAX'] = random[draws]
            draws += 1
        elif op in ('JMP', 'JZ', 'JBE'):
            if op == 'JMP' or (op == 'JZ' and zero) or (op == 'JBE' and (carry or zero)):
                next_pc = int(args[0], 16)
        else: raise AssertionError((hex(pc), op, args))
        pc = next_pc
    return registers['EBP'], draws

cases = []
probabilities = [0, 0x3f733333, 0x3f800000, 0x7fc00000, 0x7f800000, 0xbf800000]
draw_values = [0, 1, 26213, 26214, 31128, 31129, 32767]
for flags, bits, draws in itertools.product(range(32), probabilities, itertools.product(draw_values, repeat=2)):
    mask = sum(bit for i, bit in enumerate([8, 0x200, 0x400, 0x800, 0x1000]) if flags & (1 << i))
    maneuver, used = replay(mask, floating(bits), draws)
    cases.append(dict(mask=mask, probability_bits=bits, random=draws, maneuver=maneuver, draws=used))
provenance = dict(binary_sha256=hashlib.sha256(raw).hexdigest(),
                  assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),
                  instruction_range=['1003a19a', '1003a20c (exclusive)'],
                  boundary=__doc__)
(out / 'attack-oracle.json').write_text(json.dumps(dict(provenance=provenance, cases=cases)), encoding='utf-8')
print(f'Recorded {len(cases)} original branch cases')
