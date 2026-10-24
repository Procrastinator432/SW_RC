"""Execute original curve/fade/duck/priority/pool kernels on synthetic inputs.

Original CRT is mapped for _CIasin/_CIpow. Listener, source-list, level and
stop/attach methods are controlled boundaries. No original instructions/assets
are exported. Requires the pinned config/analysis-requirements.txt environment.
"""
import argparse
import hashlib
import json
import random
import struct
from pathlib import Path
import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


class Original:
    def __init__(self, game_data):
        self.u = Uc(UC_ARCH_X86, UC_MODE_32)
        self.hashes = {}
        images = {}
        for name in ['engine', 'msvcr71']:
            raw = (game_data / 'System' / (name + '.dll')).read_bytes()
            self.hashes[name] = hashlib.sha256(raw).hexdigest()
            pe = pefile.PE(data=raw)
            images[name] = pe
            self.u.mem_map(pe.OPTIONAL_HEADER.ImageBase, (pe.OPTIONAL_HEADER.SizeOfImage + 4095) & ~4095)
            self.u.mem_write(pe.OPTIONAL_HEADER.ImageBase, pe.get_memory_mapped_image())
        assert self.hashes == {
            'engine': 'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5',
            'msvcr71': '8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe'}
        exports = {s.name: images['msvcr71'].OPTIONAL_HEADER.ImageBase + s.address
                   for s in images['msvcr71'].DIRECTORY_ENTRY_EXPORT.symbols}
        for library in images['engine'].DIRECTORY_ENTRY_IMPORT:
            for imp in library.imports:
                if imp.name in [b'_CIasin', b'_CIpow']:
                    self.put(imp.address, 'I', exports[imp.name])
        self.u.mem_map(0, 4096)
        self.u.mem_map(0x5000000, 0x100000)
        self.source, self.audio, self.listener = 0x5000000, 0x5001000, 0x5002000
        self.params, self.sound, self.engine, self.actor, self.level = [0x5003000 + i * 0x1000 for i in range(5)]
        self.vtables, self.stub, self.result, self.stack = 0x5010000, 0x5020000, 0x5030000, 0x5080000
        self.pool, self.pool_array = 0x5040000, 0x5050000
        self.u.mem_write(self.stub, b'\xd9\x1d' + struct.pack('<I', self.result) + b'\x90')
        # Immutable code; changing callback VALUES never invalidates Unicorn's code cache.
        for index, (value, pop) in enumerate([(self.listener, 4), (self.params, 0), (self.actor, 0), (0, 0), (self.source, 0), (0, 0)]):
            address = self.stub + 0x100 + index * 0x20
            self.put(self.result + 0x100 + index * 4, 'I', value)
            self.u.mem_write(address, b'\xa1' + struct.pack('<I', self.result + 0x100 + index * 4)
                             + (b'\xc2' + struct.pack('<H', pop) if pop else b'\xc3'))
        self.get_pool = self.stub + 0x200
        self.stop = self.stub + 0x220
        self.attach = self.stub + 0x240
        self.u.mem_write(self.get_pool, b'\x8b\x44\x24\x04\x8b\x04\x85' + struct.pack('<I', self.pool_array) + b'\xc2\x04\x00')
        self.u.mem_write(self.stop, b'\xc3')
        self.u.mem_write(self.attach, b'\xc2\x04\x00')
        self.put(self.source, 'I', self.vtables)
        self.put(self.audio, 'I', self.vtables + 0x1000)
        self.put(self.sound, 'I', self.vtables + 0x2000)
        self.put(self.engine + 0x30, 'I', self.actor)
        self.put(self.actor, 'I', self.vtables + 0x3000)
        self.put(self.actor + 0x98, 'I', self.level)
        self.put(self.level, 'I', self.vtables + 0x4000)
        self.put(self.audio + 0x2c, 'I', self.engine)
        self.put(self.source + 4, 'I', self.audio)
        self.put(self.source + 0xc, 'I', self.sound)
        self.put(self.source + 0x2c, 'i', -1)
        for base, off, address in [
            (self.vtables + 0x1000, 0xc4, self.stub + 0x100),
            (self.vtables + 0x2000, 0xa4, self.stub + 0x120),
            (self.vtables + 0x3000, 0x1f4, self.stub + 0x140),
            (self.vtables + 0x4000, 0x100, self.stub + 0x160),
            (self.vtables + 0x1000, 0xc0, self.stub + 0x180),
            (self.vtables + 0x1000, 0xb8, self.stub + 0x1a0),
            (self.vtables + 0x1000, 0xbc, self.get_pool),
            (self.vtables + 0x1000, 0xd0, self.attach)]:
            self.put(base + off, 'I', address)
        self.put(self.vtables + 0x10, 'I', self.stop)
        self.put(0x106503b8, 'I', self.result + 0x200)
        self.trace_math = False
        self.pending_math = []
        self.calls = []
        self.seed = None
        self.u.hook_add(UC_HOOK_CODE, self.hook)

    def put(self, at, fmt, *values):
        self.u.mem_write(at, struct.pack('<' + fmt, *values))

    def get(self, at, fmt='I'):
        return struct.unpack('<' + fmt, self.u.mem_read(at, struct.calcsize('<' + fmt)))[0]

    def words(self, at, count):
        return list(struct.unpack('<' + 'I' * count, self.u.mem_read(at, count * 4)))

    def hook(self, machine, at, size, data):
        helpers = {0x103d1b30: (1, True), 0x103d1b70: (1, False),
                   0x103d1bb0: (2, True), 0x103d1c50: (2, False),
                   0x103d1c00: (3, True), 0x103d1ca0: (3, False)}
        sp = machine.reg_read(UC_X86_REG_ESP)
        if self.trace_math and at in helpers and 0x103d1d00 <= self.get(sp) <= 0x103d1e43:
            kind, encode = helpers[at]
            self.pending_math.append((at, kind, encode, self.get(sp + 4)))
        if at == 0x103d51e1:
            self.seed = machine.reg_read(UC_X86_REG_XMM0) & 0xffffffff
        if at == self.stop:
            self.calls.append(['stop', (machine.reg_read(UC_X86_REG_ECX) - self.pool) // 0x100])
        if at == self.attach:
            self.calls.append(['attach', (self.get(sp + 4) - self.pool) // 0x100])

    def call(self, address, context, arguments=(), floating=False):
        self.put(self.stack, 'I' + 'I' * len(arguments), self.stub, *arguments)
        self.u.reg_write(UC_X86_REG_ESP, self.stack)
        self.u.reg_write(UC_X86_REG_ECX, context)
        self.u.reg_write(UC_X86_REG_FPCW, 0x37f)
        self.u.reg_write(UC_X86_REG_FPSW, 0)
        self.u.reg_write(UC_X86_REG_FPTAG, 0xffff)
        self.u.reg_write(UC_X86_REG_MXCSR, 0x1f80)
        stop = self.stub + 6 if floating else self.stub
        self.u.emu_start(address, stop, count=100000)
        assert self.u.reg_read(UC_X86_REG_EIP) == stop, hex(self.u.reg_read(UC_X86_REG_EIP))
        return self.get(self.result) if floating else self.u.reg_read(UC_X86_REG_EAX)

    def math_receipts(self):
        pending, self.pending_math = self.pending_math, []
        self.trace_math = False
        return [dict(kind=k, encode=e, input_bits=v,
                     output_bits=self.call(at, 0, [v], True)) for at, k, e, v in pending]

    def initialize_curve(self, value):
        self.put(self.source + 0x40, '6IB', 0, 0, 0, 0, 0, 0, 0)
        self.call(0x103d1cf0, self.source + 0x40, [bits(value), 0, 0])
        self.put(self.source + 0x9c, 'ffIf', 0., 0., 0, value)

    def curve_snapshot(self):
        return dict(words=self.words(self.source + 0x40, 6), kind=self.get(self.source + 0x58, 'B'))

    def fade_snapshot(self):
        return dict(curve=self.curve_snapshot(), duration_bits=self.get(self.source + 0x9c),
                    elapsed_bits=self.get(self.source + 0xa0), mode=self.get(self.source + 0xa4))


def curves(h, rng):
    cases = []
    for i in range(48):
        initial = [0., 1., .25, .8][i % 4]
        h.initialize_curve(initial)
        h.put(h.source + 0x9c, 'ffI', 1e6, 0., 1)
        steps = []
        for tick in range(36):
            h.pending_math = []
            h.trace_math = True
            if tick in [0, 9, 17, 26]:
                kind = (i + tick) % 4
                target = f32(rng.choice([0., 1., .00001, .25, .5, .75, rng.random()]))
                duration = f32(rng.choice([0., .1, .3, 1., 3.]))
                h.call(0x103d1cf0, h.source + 0x40, [bits(target), bits(duration), kind])
                actual = h.call(0x103d1dc0, h.source + 0x40, floating=True)
                op = dict(op='set', target=target, duration=duration, kind=kind)
            else:
                dt = f32(rng.choice([0., 1 / 60, .03, .1, .75]))
                actual = h.call(0x103d2810, h.source, [bits(dt)], True)
                op = dict(op='step', delta=dt)
            state = h.curve_snapshot()
            math = h.math_receipts()
            steps.append(dict(**op, expected=state, value_bits=actual, math=math))
        cases.append(dict(initial=initial, steps=steps))
    return cases


def fades(h, rng):
    cases = []
    for i in range(48):
        initial = [0., 1., .5][i % 3]
        h.initialize_curve(initial)
        steps = []
        for tick in range(32):
            h.pending_math = []
            h.trace_math = True
            if tick in [0, 8, 15, 22]:
                mode = 1 + (i + tick) % 2
                duration = f32(rng.choice([0., .05, .3, 1.]))
                kind = (i + tick) % 4
                h.put(h.source + 0x9c, 'ffI', duration, 0., mode)
                h.call(0x103d1cf0, h.source + 0x40, [bits(1. if mode == 1 else 0.), bits(duration), kind])
                op = dict(op='begin', mode=mode, duration=duration, kind=kind)
                value = None
            else:
                delta = f32(rng.choice([0., 1 / 60, .05, .5, 1., 2.]))
                value = h.call(0x103d2810, h.source, [bits(delta)], True)
                op = dict(op='step', delta=delta)
            state = h.fade_snapshot()
            math = h.math_receipts()
            steps.append(dict(**op, expected=state, value_bits=value, math=math))
        cases.append(dict(initial=initial, steps=steps))
    return cases


def stop_dispatch(h):
    cases = []
    getter = h.stub + 0x280
    h.u.mem_write(getter, b'\xa1' + struct.pack('<I', h.result + 0x340) + b'\xc3')
    h.put(h.vtables + 0x2000 + 0xd0, 'I', getter)
    for initial in [.2, .5, 1.]:
        for streamed in [False, True]:
            for kind in range(4):
                for duration in [0., .05, .3]:
                    initial, duration = f32(initial), f32(duration)
                    h.initialize_curve(initial)
                    h.put(h.sound + 0x24, 'I', 0x500e000)
                    h.put(0x500e000 + 0x30, 'I', 0x107a8e20 if streamed else 0)
                    h.put(h.result + 0x340, 'I', kind)
                    h.put(h.stack + 0x20, 'f', duration)
                    h.u.reg_write(UC_X86_REG_ESP, h.stack)
                    h.u.reg_write(UC_X86_REG_EBP, h.sound)
                    h.u.reg_write(UC_X86_REG_EDI, h.source)
                    h.u.reg_write(UC_X86_REG_XMM0, bits(duration))
                    h.pending_math = []
                    h.trace_math = True
                    h.calls = []
                    h.u.emu_start(0x103d6bf6, 0x103d6c73, count=100000)
                    assert h.u.reg_read(UC_X86_REG_EIP) == 0x103d6c73
                    state = h.fade_snapshot()
                    stopped = bool(h.calls)
                    math = h.math_receipts()
                    cases.append(dict(initial=initial, streamed=streamed, kind=kind, duration=duration,
                                      stopped=stopped, expected=state, math=math))
    return cases


def volumes(h, rng):
    # Full UpdateVolume now calls the REAL GetDuckMultiplier/GetFadeMultiplier.
    h.put(h.vtables + 0x30, 'I', 0x103d2920)
    h.put(h.vtables + 0x2c, 'I', 0x103d2810)
    queue_query = h.stub + 0x2a0
    h.u.mem_write(queue_query, b'\xa1' + struct.pack('<I', h.result + 0x344) + b'\xc3')
    h.put(h.vtables + 0x20, 'I', queue_query)
    cases = []
    for i in range(24):
        initial = [0., 1., .5][i % 3]
        h.initialize_curve(initial)
        h.put(h.audio + 0x80, '5f', .1, .3, .2, .4, .8)
        h.put(h.audio + 0x9c, '9f', *([1.] * 9))
        h.put(h.audio + 0xc0, 'Iff', 0, 0., 0.)
        h.put(h.source + 0x24, 'II', (i % 8) << 1, 456)
        h.put(h.source + 0x3c, 'I', i % 2)
        h.put(h.params + 0x28, 'B', i % 3)
        h.put(h.audio + 0x114, 'I', 123)
        h.put(h.result + 0x10c, 'I', 0)
        steps = []
        for tick in range(32):
            h.pending_math = []
            h.trace_math = True
            if tick in [0, 11, 22]:
                mode = 1 + ((i + tick) % 2)
                duration = f32(rng.choice([0., .1, .3]))
                kind = (i + tick) % 4
                h.put(h.source + 0x9c, 'ffI', duration, 0., mode)
                h.call(0x103d1cf0, h.source + 0x40, [bits(1. if mode == 1 else 0.), bits(duration), kind])
                begin = dict(mode=mode, duration=duration, kind=kind)
            else:
                begin = None
            delta = f32(rng.choice([0., 1 / 60, .05, .5, 1.]))
            voices = [0, 2, 0, 1][tick // 8]
            h.put(0x107a8be4, 'i', voices)
            h.call(0x103d1210, h.audio, [bits(delta)])
            queued = bool(tick % 3 == 0)
            flags = 0x4000 if tick % 5 == 0 else 0
            base = f32([0., .25, 1., 2.][i % 4])
            h.put(h.source + 0x20, 'I', flags)
            h.put(h.source + 0x90, 'f', base)
            h.put(h.result + 0x344, 'I', int(queued))
            h.calls = []
            h.call(0x103d0c60, h.source, [bits(delta)])
            state = h.fade_snapshot()
            held = h.get(h.source + 0xa8)
            volume = h.get(h.source + 0x94)
            stopped = bool(h.calls)
            math = h.math_receipts()
            steps.append(dict(begin=begin, delta=delta, voices=voices, queued=queued, flags=flags,
                              base=base, expected=state, held_bits=held, volume_bits=volume, stopped=stopped, math=math))
        cases.append(dict(initial=initial, category=i % 3, slot=i % 8, voice_exempt=bool(i % 2), steps=steps))
    return cases


def ducking(h, rng):
    cases = []
    for i in range(24):
        attack = f32(rng.choice([.05, .15, .3, 1.]))
        release = f32(rng.choice([.1, .3, 1.]))
        levels = [f32(v) for v in [rng.random(), rng.random(), rng.random()]]
        h.put(h.audio + 0x80, '5f', attack, release, *levels)
        h.put(h.audio + 0x9c, '9f', 1., 1., 1., 1., 1., 1., 1., 1., 1.)
        h.put(h.audio + 0xc0, 'Iff', 0, 0., 0.)
        steps = []
        for tick in range(48):
            voices = [0, 1, 3, 0, 1, 0][(tick // 8 + i) % 6]
            delta = f32(rng.choice([0., 1 / 60, .03, .15, .5]))
            h.put(0x107a8be4, 'i', voices)
            h.call(0x103d1210, h.audio, [bits(delta)])
            steps.append(dict(voices=voices, delta=delta,
                              words=h.words(h.audio + 0x9c, 9), active=bool(h.get(h.audio + 0xc0, 'B')),
                              direction_bits=h.get(h.audio + 0xc4), remaining_bits=h.get(h.audio + 0xc8)))
        cases.append(dict(attack=attack, release=release, levels=levels, steps=steps))
    factors = []
    h.put(h.audio + 0x9c, 'f', .2)
    h.put(h.audio + 0xa8, 'f', .4)
    h.put(h.audio + 0xb4, 'f', .8)
    h.put(h.audio + 0x114, 'I', 123)
    for category in [0, 1, 2, 255]:
        for slot in range(8):
            for exempt in [False, True]:
                for special in [False, True]:
                    for ducker in [False, True]:
                        h.put(h.params + 0x28, 'B', category)
                        h.put(h.source + 0x24, 'II', slot << 1, 123 if ducker else 456)
                        h.put(h.source + 0x3c, 'I', int(exempt))
                        h.put(h.result + 0x100 + 3 * 4, 'I', int(special))
                        actual = h.call(0x103d2920, h.source, floating=True)
                        factors.append(dict(category=category, slot=slot, voice_exempt=exempt,
                                            special_level=special, ducker=ducker, expected_bits=actual))
    return cases, factors


def priorities(h, rng):
    cases = []
    for i in range(1024):
        slot = i % 8
        radius = rng.choice([0., 1., 10., 100., 1024.])
        location = [f32(rng.uniform(-1000, 1000)) for _ in range(3)]
        listener = location[:] if i % 9 == 0 else [f32(v + rng.uniform(-1500, 1500)) for v in location]
        volume = f32(rng.choice([-1., 0., .0001, .2, .7, 1., 2.]))
        flags = rng.choice([0, 4, 0x100, 0x104])
        multiplier = f32(rng.choice([.5, 1., 100.]))
        previous = f32(rng.uniform(0, 6))
        h.put(h.source + 0x20, 'II', flags, slot << 1)
        h.put(h.source + 0x30, 'f', previous)
        h.put(h.source + 0x5c, '3f', *location)
        h.put(h.source + 0x78, 'f', radius)
        h.put(h.source + 0x90, 'f', volume)
        h.put(h.listener + 0x1c, '3f', *listener)
        h.put(h.result + 0x200, 'f', multiplier)
        h.seed = None
        h.call(0x103d50e0, h.source, [bits(1 / 60)])
        cases.append(dict(location=location, listener=listener, volume=volume, radius=radius,
                          radius_multiplier=multiplier, flags=flags, slot=slot, previous=previous,
                          seed_bits=h.seed, expected_bits=h.get(h.source + 0x30)))
    return cases


def pools(h, rng):
    cases = []
    for i in range(512):
        count = i % 17
        incoming_id = rng.choice([0, 1, 2, 3, 0x12340010, 0xffffffff])
        incoming_priority = f32(rng.choice([0., .1, .5, 1., 2.]))
        slots = []
        h.put(h.source + 0x24, 'I', incoming_id)
        h.put(h.source + 0x30, 'f', incoming_priority)
        for j in range(count):
            occupied = rng.choice([True, True, True, False])
            identity = rng.choice([0, 1, 2, 3, incoming_id, incoming_id ^ 1, 0x76543210])
            priority = f32(rng.choice([0., .1, .5, 1., 2.]))
            at = h.pool + j * 0x100
            h.put(h.pool_array + j * 4, 'I', at)
            h.put(at, 'I', h.vtables)
            h.put(at + 8, 'I', int(occupied))
            h.put(at + 0x24, 'I', identity)
            h.put(at + 0x30, 'f', priority)
            slots.append(dict(occupied=occupied, identity=identity, priority=priority))
        h.put(h.result + 0x100 + 5 * 4, 'I', count)
        h.calls = []
        accepted = bool(h.call(0x103d0d10, h.audio))
        cases.append(dict(identity=incoming_id, priority=incoming_priority,
                          slots=slots, accepted=accepted, calls=h.calls[:]))
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-fixture', type=Path)
    args = parser.parse_args()
    h = Original(args.game_data)
    rng = random.Random(641024)
    curve_cases = curves(h, rng)
    fade_cases = fades(h, rng)
    stops = stop_dispatch(h)
    duck_cases, duck_factors = ducking(h, rng)
    volume_cases = volumes(h, rng)
    priority_cases = priorities(h, rng)
    pool_cases = pools(h, rng)
    data = dict(binary_sha256=h.hashes,
                scope='Original curve/fade and duck controllers; running priority with recorded RSQRTSS seed; full FindSource with controlled pool/stop/attach callbacks',
                curves=curve_cases, fades=fade_cases, stops=stops, ducking=duck_cases, duck_factors=duck_factors, volumes=volume_cases,
                priorities=priority_cases, pools=pool_cases)
    if args.verify_fixture:
        assert json.loads(args.verify_fixture.read_text()) == data, 'Fresh original fixture mismatch'
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, separators=(',', ':')) + '\n')
    report = dict(passed=True, curve_steps=sum(len(c['steps']) for c in curve_cases),
                  fade_steps=sum(len(c['steps']) for c in fade_cases), stop_cases=len(stops),
                  composed_volume_steps=sum(len(c['steps']) for c in volume_cases),
                  duck_steps=sum(len(c['steps']) for c in duck_cases), duck_factors=len(duck_factors),
                  priority_cases=len(priority_cases), pool_cases=len(pool_cases),
                  fixture_verified=bool(args.verify_fixture))
    args.output.with_suffix('.validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
