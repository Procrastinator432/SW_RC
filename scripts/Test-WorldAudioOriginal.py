"""Replay selected PC world-audio arithmetic against the installed engine.dll.

Requires unicorn==2.1.4 and pefile==2024.8.26. No original code is exported.
Geometry, listener lookup and duck/fade controllers are supplied inputs, not
claims of complete engine emulation. Every reported result comes from x86.
"""
import argparse
import hashlib
import itertools
import json
import random
import struct
from pathlib import Path

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import *


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-fixture', type=Path,
                        help='Fail if fresh original results differ from this frozen fixture')
    args = parser.parse_args()
    binary = (args.game_data / 'System/engine.dll').read_bytes()
    digest = hashlib.sha256(binary).hexdigest()
    if digest != 'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5':
        raise ValueError('Unsupported original engine.dll SHA-256')
    pe = pefile.PE(data=binary)
    u = Uc(UC_ARCH_X86, UC_MODE_32)
    u.mem_map(pe.OPTIONAL_HEADER.ImageBase, (pe.OPTIONAL_HEADER.SizeOfImage + 4095) & ~4095)
    u.mem_write(pe.OPTIONAL_HEADER.ImageBase, pe.get_memory_mapped_image())
    u.mem_map(0x4000000, 0x10000)
    u.mem_map(0x5000000, 0x10000)
    source, frame, stack = 0x5000000, 0x4008000, 0x4007000
    vtable, stub, result, global_radius = 0x5001000, 0x5002000, 0x5003000, 0x5004000

    def write(a, fmt, *values):
        u.mem_write(a, struct.pack('<' + fmt, *values))

    def read(a, fmt='I'):
        return struct.unpack('<' + fmt, u.mem_read(a, struct.calcsize('<' + fmt)))[0]

    def run(start, stop):
        u.reg_write(UC_X86_REG_FPCW, 0x37f)
        u.reg_write(UC_X86_REG_FPSW, 0)
        u.reg_write(UC_X86_REG_FPTAG, 0xffff)
        u.reg_write(UC_X86_REG_MXCSR, 0x1f80)
        u.emu_start(start, stop, count=2000)
        if u.reg_read(UC_X86_REG_EIP) != stop:
            raise RuntimeError('Original instruction region did not reach its exit')

    # Return trampoline stores the original x87 return as binary32.
    u.mem_write(stub, b'\xd9\x1d' + struct.pack('<I', result) + b'\x90')
    write(0x106503b8, 'I', global_radius)
    r = random.Random(59001)
    priority = []
    combinations = list(itertools.product([0, 4, 0x100, 0x104, 0x1804], range(8),
                                         [-1., 0., 0.0001, 0.25, 0.75, 1., 2.]))
    for i in range(1000):
        flags, slot, volume = combinations[i % len(combinations)]
        radius = r.choice([0., 1., 100., 1024., 100000.])
        multiplier = r.choice([0.25, 1., 2., 4.])
        listener = [r.uniform(-8192., 8192.) for _ in range(3)]
        location = listener[:] if i % 10 == 0 else [v + r.uniform(-radius * 2, radius * 2) for v in listener]
        if i % 10 in [1, 2, 3]:
            location = [listener[0] + radius * multiplier * [0.5, 1., 2.][i % 10 - 1], *listener[1:]]
        # All fixture input numbers are exactly the f32 values actually supplied.
        f32 = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
        listener, location = list(map(f32, listener)), list(map(f32, location))
        write(source, '3f', *location)
        write(source + 16, '3f', *listener)
        write(global_radius, 'f', multiplier)
        write(stack, 'IIIffII', stub, source, source + 16, volume, radius, flags, slot << 1)
        u.reg_write(UC_X86_REG_ESP, stack)
        run(0x103d64e0, stub + 6)
        priority.append(dict(location=location, listener=listener, volume=volume, radius=radius,
                             radius_multiplier=multiplier, flags=flags, slot=slot,
                             expected_bits=read(result)))

    radii = []
    for i in range(1000):
        base = r.choice([0., 15., 100., 1024., 1e6])
        scale = None if i % 3 == 0 else r.choice([0., 0.1, 0.5, 1.])
        current = r.choice([0., 10., 100., 2048.])
        previous = r.choice([base, 0., 35., 1024.])
        now = r.choice([0., 0.125, 1., 3., 30., 4096.])
        changed = r.choice([0., now, now - 0.5, now - 1., now - 2.])
        u.reg_write(UC_X86_REG_ESI, source)
        u.reg_write(UC_X86_REG_EBP, frame)
        write(frame - 0x14, 'f', base)
        write(source + 0x7c, 'ff', current, previous)
        write(source + 0x84, 'd', changed)
        write(0x1076390c, 'f', now)
        if scale is None:
            run(0x103d26a9, 0x103d26b3)
        else:
            write(frame - 0x18, 'f', scale)
            u.reg_write(UC_X86_REG_EAX, source + 0x500)
            run(0x103d267f, 0x103d26a7)
        # Skip profiling instructions only. Preserve XMM0 and the real local target.
        u.reg_write(UC_X86_REG_ECX, source + 0x500)
        run(0x103d26f6, 0x103d2771)
        radii.append(dict(base_radius=base, occlusion_scale=scale, current=current,
                          previous_target=previous, changed_at=changed, now=now,
                          expected=dict(priority_radius_bits=read(source + 0x78),
                                        current_bits=read(source + 0x7c),
                                        target_bits=read(source + 0x80),
                                        changed_at=read(source + 0x84, 'd'))))

    # Full UpdateVolume; the duck/fade/paused/stop virtual methods are explicit
    # controlled boundaries. The original chooses when to call/stop/multiply.
    duck_stub, fade_stub, pause_stub, stop_stub = [stub + n for n in [0x100, 0x200, 0x300, 0x400]]
    write(source, 'I', vtable)
    for off, address in [(0x30, duck_stub), (0x2c, fade_stub), (0x20, pause_stub), (0x10, stop_stub)]:
        write(vtable + off, 'I', address)
    u.mem_write(duck_stub, b'\xd9\x05' + struct.pack('<I', result + 4) + b'\xc3')
    # Fade callback can complete its mode before returning, as the real one does.
    u.mem_write(fade_stub, b'\xa1' + struct.pack('<I', result + 12) + b'\x89\x81\xa4\x00\x00\x00'
                + b'\xd9\x05' + struct.pack('<I', result + 8) + b'\xc2\x04\x00')
    u.mem_write(pause_stub, b'\xa1' + struct.pack('<I', result + 16) + b'\xc3')
    u.mem_write(stop_stub, b'\xc7\x05' + struct.pack('<I', result + 20) + b'\x01\x00\x00\x00\xc3')
    volumes = []
    for i in range(1000):
        before = r.randrange(3)
        after = r.choice([0, before])
        base = r.choice([0., 0.25, 1., 2.])
        duck = r.choice([0., 0.1, 0.5, 1.])
        fade = r.choice([0., 0.00001, 0.0001, 0.125, 1.])
        if before == 0:
            fade = 1.
        held = r.choice([0., 0.00001, 0.0001, 0.25, 1.])
        flags, paused = r.choice([0, 0x4000, 0x4100]), bool(i % 2)
        write(source + 0x20, 'I', flags)
        write(source + 0x90, 'f', base)
        write(source + 0xa4, 'If', before, held)
        write(result + 4, 'ffIII', duck, fade, after, int(paused), 0)
        write(stack, 'If', stub + 6, 1. / 60.)
        u.reg_write(UC_X86_REG_ECX, source)
        u.reg_write(UC_X86_REG_ESP, stack)
        run(0x103d0c60, stub + 6)
        volumes.append(dict(base_volume=base, duck=duck, fade=fade, held_fade=held,
                            mode_before=before, mode_after=after, flags=flags, paused=paused,
                            expected=dict(volume_bits=read(source + 0x94),
                                          held_fade_bits=read(source + 0xa8),
                                          stop=bool(read(result + 20)))))

    data = dict(scope='PC SoundPriority admission; UpdateRadii arithmetic after geometry; '
                      'UpdateVolume with controlled duck/fade/paused callbacks; no mixer integration',
                engine_sha256=digest,
                regions=dict(priority=['103d64e0', '103d6638'],
                             radii=['103d267f..103d26a7 or 103d26a9..103d26b3', '103d26f6..103d2771'],
                             volume=['103d0c60', '103d0cfc']),
                priority=priority, radii=radii, volumes=volumes)
    if args.verify_fixture and json.loads(args.verify_fixture.read_text(encoding='utf-8')) != data:
        raise ValueError('Fresh original results differ from the frozen fixture')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, separators=(',', ':')) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, priority=len(priority), radii=len(radii), volumes=len(volumes),
                         fixture_verified=bool(args.verify_fixture))))


if __name__ == '__main__':
    main()
