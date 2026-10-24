"""Replay LoadMap's unmodified zone-table block (1041f33e..1041f4bc).

Also replays the radius-initialization stores 103d2c4d..103d2c7b.
No LoadMap callbacks: only the original 64-bit shift helper is called. Synthetic
fixtures contain no original instructions; --models also checks private map data.
"""
import argparse
import hashlib
import json
import random
import struct
from pathlib import Path
import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_XMM0, UC_X86_REG_XMM2


def expected(links):
    table = bytearray([255] * 4096)
    reach = links[:]
    for i in range(64):
        table[i * 64 + i] = 0
    for distance in range(1, 64):
        for i, mask in enumerate(reach):
            for j in range(64):
                if mask & (1 << j) and table[i * 64 + j] > distance:
                    table[i * 64 + j] = distance
        reach = [union(reach[j] for j in range(64) if mask & (1 << j)) for mask in reach]
    return table


def union(values):
    result = 0
    for value in values:
        result |= value
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-fixture', type=Path)
    parser.add_argument('--models', type=Path)
    args = parser.parse_args()
    binary = (args.game_data / 'System/engine.dll').read_bytes()
    digest = hashlib.sha256(binary).hexdigest()
    assert digest == 'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5'
    pe = pefile.PE(data=binary)
    machine = Uc(UC_ARCH_X86, UC_MODE_32)
    base = pe.OPTIONAL_HEADER.ImageBase
    machine.mem_map(base, (pe.OPTIONAL_HEADER.SizeOfImage + 4095) & ~4095)
    machine.mem_write(base, pe.get_memory_mapped_image())
    machine.mem_map(0x5000000, 0x40000)
    frame, engine, level, model = (0x5010000, 0x5020000, 0x5022000, 0x5025000)

    def put(at, fmt, *values):
        machine.mem_write(at, struct.pack('<' + fmt, *values))

    def replay(links):
        put(frame - 0x14, 'I', engine)
        put(engine + 0x118, 'I', level)
        put(level + 0x78, 'I', model)
        for i, mask in enumerate(links):
            put(model + 0xf0 + i * 24, 'Q', mask)
        machine.reg_write(UC_X86_REG_EBP, frame)
        machine.reg_write(UC_X86_REG_ESP, frame - 0x1000)
        machine.emu_start(0x1041f33e, 0x1041f4bc, count=200000000)
        assert machine.reg_read(UC_X86_REG_EIP) == 0x1041f4bc
        result = bytes(machine.mem_read(level + 0xf0, 4096))
        assert result == expected(links)
        return result.hex()

    graphs = [('empty', [0] * 64), ('self', [1 << i for i in range(64)]),
              ('full', [(1 << 64) - 1] * 64),
              ('directed-chain', [1 << (i + 1) if i < 63 else 0 for i in range(64)]),
              ('reflexive-chain', [(1 << i) | (1 << (i + 1) if i < 63 else 0) for i in range(64)]),
              ('ring', [1 << ((i + 1) % 64) for i in range(64)]),
              ('high-bit', [1 << 63] + [0] * 62 + [1])]
    rng = random.Random(641060)
    for size in [3, 5, 14, 32, 64]:
        graphs.append((f'directed-random-{size}', [rng.getrandbits(size) if i < size else 0 for i in range(64)]))
    cases = []
    for name, links in graphs:
        cases.append(dict(name=name, connectivity_hex=[f'{v:016x}' for v in links], distances_hex=replay(links)))
        print(f'original zone table: {name} passed', flush=True)
    initial = []
    source = 0x5026000
    for radius in [0., 0.1, 1., 10., 100.]:
        machine.reg_write(UC_X86_REG_ESI, source)
        machine.reg_write(UC_X86_REG_XMM0, struct.unpack('<I', struct.pack('<f', radius))[0])
        machine.reg_write(UC_X86_REG_XMM2, 0x3f800000)
        machine.emu_start(0x103d2c4d, 0x103d2c7b, count=100)
        assert machine.reg_read(UC_X86_REG_EIP) == 0x103d2c7b
        priority, current, target = struct.unpack('<3I',machine.mem_read(source+0x78,12))
        clock = struct.unpack('<Q',machine.mem_read(source+0x84,8))[0]
        initial.append(dict(radius=radius, radius_bits=[priority,current,target], changed_at_bits=clock))
    fixture = dict(source_initial=initial, binary_sha256=digest, scope='Unmodified LoadMap zone-table block, original __allshl and FAbstractSource::Init radius stores', cases=cases)
    if args.verify_fixture:
        assert json.loads(args.verify_fixture.read_text()) == fixture
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(fixture, separators=(',', ':')) + '\n')
    maps = []
    if args.models:
        for path in sorted(args.models.glob('geo_*.json')):
            m = json.loads(path.read_text())
            links = m['connectivity'] + [0] * (64 - len(m['connectivity']))
            actual = replay(links)
            assert actual == m['distances_hex']
            maps.append(dict(map=m['map'], zones=len(m['connectivity']), equal=True))
    report = dict(passed=True, synthetic_tables=len(cases), source_initial_cases=len(initial), bytes_per_table=4096, maps=maps)
    args.output.with_suffix('.validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
