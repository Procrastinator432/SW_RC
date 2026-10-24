"""Synthetic original FAbstractSource initialization, lifecycle, lookup and UpdateBase.
Uses the pinned engine/CRT mapper from Test-AudioControllersOriginal.py. Native
pointers are controlled synthetic handles. UpdateBase's volume/radii/priority,
actor validity/logging and driver pause queries are explicit callback boundaries.
The separate composed cases execute the real source/duck/fade/radius/priority
functions with clear/nonpositional hearing and controlled driver/actor callbacks.
No game scene, audio output, or original instructions are exported.
"""
import argparse
import importlib.util
import json
import random
from pathlib import Path

spec = importlib.util.spec_from_file_location('audio_controllers_original', Path(__file__).with_name('Test-AudioControllersOriginal.py'))
core = importlib.util.module_from_spec(spec)
spec.loader.exec_module(core)
bits, f32 = core.bits, core.f32


class Harness(core.Original):
    def __init__(self, game_data):
        super().__init__(game_data)
        self.boundaries = False
        self.composing = False
        self.queued = False
        self.trace = []
        self.pause_results = []
        self.clear_on_volume = False
        self.valid_actor = True
        self.pausequerystub, self.validstub = self.stub + 0x400, self.stub + 0x420
        self.namestub, self.logstub = self.stub + 0x440, self.stub + 0x460
        self.kindstub = self.stub + 0x480
        self.queuequerystub = self.stub + 0x4a0
        self.audiblestub = self.stub + 0x4c0
        self.u.mem_write(self.queuequerystub, b"\xc3")
        self.u.mem_write(self.audiblestub, b"\xb8\x01\x00\x00\x00\xc2\x0c\x00")
        self.put(self.vtables + 0x4000 + 0x10c, "I", self.audiblestub)
        for at in [self.pausequerystub, self.validstub, self.namestub, self.logstub]:
            self.u.mem_write(at, b'\xc3')
        self.u.mem_write(self.kindstub, b'\xa1' + core.struct.pack('<I', self.result + 0x400) + b'\xc3')
        self.put(self.vtables + 0x1c, 'I', self.pausequerystub)
        self.put(self.vtables + 4, 'I', 0x10358eb0)
        self.put(self.vtables + 0x2000 + 0xcc, 'I', self.kindstub)
        self.u.mem_write(0x500b100, b'synthetic\0')
        for lib in self.engine_pe.DIRECTORY_ENTRY_IMPORT:
            for imp in lib.imports:
                name = imp.name or b''
                target = None
                if b'kZero' in name: target = 0x500b000
                if b'?IsValid@UObject' in name: target = self.validstub
                if b'?GetName@UObject' in name: target = self.namestub
                if b'?Logf@FOutputDevice' in name: target = self.logstub
                if b'GLog' in name: target = 0x500b200
                if b'CurStat' in name: target = self.result + 0x900
                if target is not None: self.put(imp.address, 'I', target)
        self.put(self.result + 0x900, 'I', self.result + 0x910)

    def hook(self, u, at, size, data):
        super().hook(u, at, size, data)
        if self.composing and at in [0x103d0c60, 0x103d2520, 0x103d50e0, 0x10358f10]:
            self.trace.append({0x103d0c60:"volume",0x103d2520:"radii",0x103d50e0:"priority",0x10358f10:"stop"}[at])
        if at == self.queuequerystub:
            u.reg_write(core.UC_X86_REG_EAX, int(self.queued))
        if at == self.pausequerystub:
            u.reg_write(core.UC_X86_REG_EAX, int(self.pause_results.pop(0)))
        elif at == self.validstub:
            u.reg_write(core.UC_X86_REG_EAX, int(self.valid_actor))
        elif at == self.namestub:
            u.reg_write(core.UC_X86_REG_EAX, 0x500b100)
        if self.boundaries and at in [0x103d0c60, 0x103d2520, 0x103d50e0]:
            self.trace.append({0x103d0c60: 'volume', 0x103d2520: 'radii', 0x103d50e0: 'priority'}[at])
            if at == 0x103d0c60 and self.clear_on_volume:
                # Explicit synthetic callback effect. Clear itself is independently
                # executed and checked below; this callback does not run UpdateVolume.
                self.clear_fields(self.source)
            sp = u.reg_read(core.UC_X86_REG_ESP)
            u.reg_write(core.UC_X86_REG_EIP, self.get(sp))
            u.reg_write(core.UC_X86_REG_ESP, sp + 8)

    def clear_fields(self, at):
        for off in [8, 0xc, 0x10, 0x14, 0x20, 0x24, 0x30, 0x34, 0x38]: self.put(at + off, 'I', 0)
        self.put(at + 0xa8, 'I', bits(1.))

    def construct(self, seed, at=None):
        at = self.source if at is None else at
        self.u.mem_write(at, bytes(0xac))
        self.put(0x107a8bec, 'I', seed)
        self.call(0x103d24a0, at, [self.audio])
        return self.snapshot(at)

    def snapshot(self, at=None):
        at = self.source if at is None else at
        word = lambda off: self.get(at + off)
        return dict(allocated=bool(word(8)), sound=word(0xc), original_sound=word(0x10), actor=word(0x14),
                    lifetime_words=self.words(at + 0x18, 2), flags=word(0x20), identity=word(0x24),
                    unique_id=word(0x28), engine_index=self.get(at + 0x2c, 'i'), priority_bits=word(0x30),
                    cursor=word(0x34), requested_pause=self.get(at + 0x38, 'i'), voice_exempt=bool(word(0x3c)),
                    fade=dict(curve=dict(words=self.words(at + 0x40, 6), kind=self.get(at + 0x58, 'B')),
                              duration_bits=word(0x9c), elapsed_bits=word(0xa0), mode=word(0xa4), held_bits=word(0xa8)),
                    location_bits=self.words(at + 0x5c, 3), velocity_bits=self.words(at + 0x68, 3),
                    base_radius_bits=word(0x74), radii=dict(radius_bits=self.words(at + 0x78, 3), changed_at_bits=self.words(at + 0x84, 2)),
                    pitch_bits=word(0x8c), volume_bits=word(0x90), output_volume_bits=word(0x94), reserved98=word(0x98))


def initialization(h, rng):
    cases = []
    for i in range(384):
        h.construct(i)
        h.put(h.source + 0x2c, 'i', -1)
        h.put(h.source + 0x38, 'i', -3)
        h.put(h.source + 0x68, 'fff', .25, -1., 3.)
        h.put(h.source + 0x98, 'f', .7)
        before = h.snapshot()
        inp = dict(engine_present=i % 17 != 0, sound=h.sound, original_sound=h.sound + 0x100,
                   actor=rng.choice([0, h.actor]), flags=rng.choice([0, 4, 0x100, 0x104]),
                   identity=(i % 8) * 2 + (i % 2) + 0x100, engine_index=rng.choice([-1, 0, 7]),
                   location=[f32(rng.uniform(-1000, 1000)) for _ in range(3)], volume=f32(rng.random()),
                   initial_output=f32(rng.uniform(.5, 2)), pitch=f32(rng.random()), fade_seconds=rng.choice([0., .05, .3, 1.]),
                   fade_curve=i % 4, radius=rng.choice([0., .5, 1., 10., 50., 100., 1000.]),
                   ambient_priority=f32(rng.random()), radius_caps=[rng.choice([0., 1., 20., 80.]) for _ in range(3)],
                   voice_exempt=bool(i % 2))
        inp['fade_seconds'] = f32(inp['fade_seconds'])
        h.put(h.audio + 0x2c, 'I', h.engine if inp['engine_present'] else 0)
        h.put(h.audio + 0xd8, 'fff', *inp['radius_caps'])
        h.put(h.params + 0x2c, 'I', int(inp['voice_exempt']))
        h.put(h.result + 0x400, 'I', inp['fade_curve'])
        h.put(0x500b300, 'fff', *inp['location'])
        h.call(0x103d2a40, h.source, [123, inp['actor'], inp['sound'], inp['original_sound'], inp['flags'],
               inp['identity'], inp['engine_index'] & 0xffffffff, 0x500b300,
               *[bits(inp[s]) for s in ['volume', 'initial_output', 'pitch', 'fade_seconds', 'radius', 'ambient_priority']]])
        cases.append(dict(input=inp, before=before, expected=h.snapshot()))
    h.put(h.audio + 0x2c, 'I', h.engine)
    return cases


def lifecycle(h, initial):
    cases = []
    for i, item in enumerate(initial[:96]):
        # Load a native initialized state by copying captured scalar fields from
        # a just-executed initialization, then replay Pause/Clear/base Stop.
        # Use initialization again so no Python version of Init serves as oracle.
        inp = item['input']
        h.construct(i)
        h.put(h.source + 0x2c, 'i', -1)
        h.put(h.audio + 0xd8, 'fff', *inp['radius_caps'])
        h.put(h.params + 0x2c, 'I', int(inp['voice_exempt']))
        h.put(h.result + 0x400, 'I', inp['fade_curve'])
        h.put(0x500b300, 'fff', *inp['location'])
        h.call(0x103d2a40, h.source, [0, inp['actor'], inp['sound'], inp['original_sound'], inp['flags'], inp['identity'],
               inp['engine_index'] & 0xffffffff, 0x500b300,
               *[bits(inp[s]) for s in ['volume', 'initial_output', 'pitch', 'fade_seconds', 'radius', 'ambient_priority']]])
        before = h.snapshot()
        steps = []
        for op, value in [('pause', -7), ('pause', 1), ('stop', 0), ('clear', 0), ('pause', 0)]:
            address = {'pause': 0x10358f00, 'stop': 0x10358f10, 'clear': 0x10358eb0}[op]
            result = h.call(address, h.source, [value & 0xffffffff] if op == 'pause' else [])
            steps.append(dict(op=op, request=value, result=result if op != 'clear' else None, expected=h.snapshot()))
        cases.append(dict(before=before, steps=steps))
    return cases


def base_updates(h, rng):
    cases = []
    for i in range(512):
        h.construct(i)
        h.put(h.source, 'I', h.vtables)
        h.put(h.source + 8, 'I', int(i % 13 != 0))
        h.put(h.source + 0x14, 'I', h.actor if i % 3 else 0)
        h.put(h.source + 0x20, 'I', rng.choice([0, 4, 0x100, 0x104]))
        h.put(h.source + 0x24, 'I', rng.choice([0, 1, 6, 7, 0x108, 0x10a]))
        h.put(h.source + 0x2c, 'i', -1)
        h.put(h.source + 0x5c, 'fff', 5., 6., 7.)
        h.put(h.source + 0x68, 'fff', 8., 9., 10.)
        inp = dict(paused=[i % 11 == 0, i % 7 == 0], actor_valid=i % 5 != 0, engine_present=i % 17 != 0,
                   listener_velocity=[f32(rng.uniform(-10, 10)) for _ in range(3)],
                   actor_location=[f32(rng.uniform(-100, 100)) for _ in range(3)],
                   actor_velocity=[f32(rng.uniform(-10, 10)) for _ in range(3)])
        h.put(h.audio + 0x2c, 'I', h.engine if inp['engine_present'] else 0)
        h.put(h.listener + 0x28, 'fff', *inp['listener_velocity'])
        h.put(h.actor + 0x138, 'fff', *inp['actor_location'])
        h.put(h.actor + 0x150, 'fff', *inp['actor_velocity'])
        h.put(h.audio + 0x22c, 'I', 0)
        h.pause_results = list(inp['paused'])
        h.valid_actor = inp['actor_valid']
        h.clear_on_volume = i % 4 == 0
        before = h.snapshot()
        h.trace = []
        h.boundaries = True
        result = h.call(0x103d5340, h.source, [bits(.05)])
        h.boundaries = False
        cases.append(dict(before=before, input=inp, delta=f32(.05), clear_on_volume=h.clear_on_volume,
                          updated=bool(result), counter_incremented=bool(h.get(h.audio + 0x22c)), calls=h.trace, expected=h.snapshot()))
    h.put(h.audio + 0x2c, 'I', h.engine)
    return cases


def lookups(h, rng):
    cases = []
    for i in range(256):
        active, waiting = [], []
        active_count, queued_count = rng.randrange(8), rng.randrange(8)
        addresses = []
        for j in range(active_count + queued_count):
            at = h.pool + j * 0x100
            h.construct(j, at)
            h.put(at + 0x14, 'I', rng.choice([0, h.actor, h.actor + 0x100]))
            h.put(at + 0xc, 'I', rng.choice([0, h.sound, h.sound + 0x100]))
            h.put(at + 0x10, 'I', rng.choice([0, h.sound, h.sound + 0x100]))
            h.put(at + 0x28, 'I', rng.randrange(6))
            h.put(at + 8, 'I', rng.randrange(2))
            addresses.append(at)
            (active if j < active_count else waiting).append(h.snapshot(at))
        for j, at in enumerate(addresses[:active_count]): h.put(h.pool_array + j * 4, 'I', at)
        for j, at in enumerate(addresses[active_count:]): h.put(0x5051000 + j * 4, 'I', at)
        h.put(h.result + 0x100 + 5 * 4, 'I', active_count)
        h.put(h.audio + 0x3c, 'II', 0x5051000, queued_count)
        q = dict(actor=rng.choice([0, h.actor, h.actor + 0x100]), sound=rng.choice([0, h.sound, h.sound + 0x100]),
                 auto_actor=bool(i % 2), engine_actor=rng.choice([0, h.actor, h.actor + 0x100]))
        if q['actor'] == 0 and q['sound'] == 0:
            q['actor'] = h.actor  # Native both-null GetSourceIndex dereferences null.
        for sound in [h.sound, h.sound + 0x100]:
            h.put(sound, 'I', h.vtables + 0x2000)
            h.put(sound + 0x30, 'I', 2 if q['auto_actor'] else 0)
        h.put(h.params + 0x54, 'I', 1 if q['auto_actor'] else 0)
        h.put(h.engine + 0x30, 'I', q['engine_actor'])
        idx = h.call(0x103d2f90, h.audio, [q['actor'], q['sound']])
        queue = h.call(0x10363480, h.audio, [q['actor'], q['sound']])
        found = h.call(0x103634d0, h.audio, [q['actor'], q['sound']])
        identity = rng.randrange(8)
        by_id = h.call(0x10363540, h.audio, [identity])
        def locate(at):
            if not at: return None
            j = addresses.index(at)
            return dict(Active=j) if j < active_count else dict(Queued=j-active_count)
        probes = []
        for at in addresses:
            probes.append(bool(h.call(0x10358e80, at, [q['actor'], q['sound']]) & 0xff))
        generated = [h.call(0x10359630, h.audio, [at]) for at in addresses]
        cases.append(dict(active=active, queued=waiting, query=q, index=None if idx == 0xffffffff else idx,
                          queued_index=None if not queue else addresses.index(queue)-active_count, source=locate(found),
                          id=identity, by_id=locate(by_id), matches=probes, generated_ids=generated))
    return cases


def composed(h, rng):
    cases = []
    for i in range(24):
        h.construct(i)
        h.put(h.source, 'I', h.vtables)
        h.put(h.source + 0x2c, 'i', -1)
        h.put(h.audio + 0x2c, 'I', h.engine)
        h.put(h.engine + 0x30, 'I', h.actor)
        h.put(h.audio + 0xd8, 'fff', 0., 0., 0.)
        h.put(h.params + 0x2c, 'I', i % 2)
        h.put(h.params + 0x28, 'B', i % 3)
        h.put(h.result + 0x400, 'I', i % 4)
        h.put(0x500b300, 'fff', 1., 2., 3.)
        flags = [0, 4, 0x100, 0x4000][i % 4]
        h.call(0x103d2a40, h.source, [0, h.actor if i % 2 else 0, h.sound, h.sound, flags,
               0x100 + (i % 8) * 2, 0xffffffff, 0x500b300, bits(.75), bits(.75), bits(1.1), bits(.15), bits(40.), bits(.4)])
        for offset, function in [(4, 0x10358eb0), (0x10, 0x10358f10), (0x20, h.queuequerystub),
                                 (0x2c, 0x103d2810), (0x30, 0x103d2920)]:
            h.put(h.vtables + offset, 'I', function)
        settings = dict(attack=f32(.15), release=f32(.3), levels=[f32(.2), f32(.4), f32(.8)])
        h.put(h.audio + 0x80, '5f', settings['attack'], settings['release'], *settings['levels'])
        h.put(h.audio + 0x9c, '9f', *([1.] * 9))
        h.put(h.audio + 0xc0, 'Iff', 0, 0., 0.)
        def duck_snapshot():
            return dict(settings=settings, words=h.words(h.audio + 0x9c, 9), active=bool(h.get(h.audio + 0xc0, 'B')),
                        direction_bits=h.get(h.audio + 0xc4), remaining_bits=h.get(h.audio + 0xc8))
        before, duck_before = h.snapshot(), duck_snapshot()
        steps = []
        for tick in range(32):
            begin = None
            if tick == 12:
                begin = dict(mode=2, duration=f32(.3), kind=i % 4)
                h.put(h.source + 0x9c, 'ffI', begin['duration'], 0., 2)
                h.call(0x103d1cf0, h.source + 0x40, [bits(0.), bits(begin['duration']), i % 4])
            delta = f32([1/60, .05, 1., 0.][tick % 4])
            voices = [0, 2, 0, 1][tick // 8]
            frame = dict(base=dict(paused=[False, tick % 7 == 0], actor_valid=True, engine_present=True,
                                  listener_velocity=[1., 2., 3.], actor_location=[f32(tick), 2., 3.], actor_velocity=[4., 5., 6.]),
                         now=f32(tick / 60), occlusion=None, listener_location=[10., 20., 30.], radius_multiplier=100.,
                         rsqrt_seed_bits=None, category=i % 3, special_level=tick % 11 == 0,
                         ducker=tick % 5 == 0, queued=tick % 3 == 0)
            h.put(0x107a8be4, 'i', voices)
            h.call(0x103d1210, h.audio, [bits(delta)])
            h.put(h.listener + 0x1c, '3f', *frame['listener_location'])
            h.put(h.listener + 0x28, '3f', *frame['base']['listener_velocity'])
            h.put(h.actor + 0x138, '3f', *frame['base']['actor_location'])
            h.put(h.actor + 0x150, '3f', *frame['base']['actor_velocity'])
            h.put(h.result + 0x200, 'f', frame['radius_multiplier'])
            h.put(h.result + 0x100 + 3 * 4, 'I', int(frame['special_level']))
            h.put(h.audio + 0x114, 'I', h.get(h.source + 0x28) if frame['ducker'] else 0xffffffff)
            h.put(0x1076390c, 'f', frame['now'])
            h.put(h.audio + 0x22c, 'I', 0)
            h.pause_results = list(frame['base']['paused'])
            h.queued = frame['queued']
            h.valid_actor = True
            h.trace, h.seed = [], None
            h.composing = True
            updated = h.call(0x103d5340, h.source, [bits(delta)])
            h.composing = False
            frame['rsqrt_seed_bits'] = h.seed
            steps.append(dict(begin=begin, voices=voices, delta=delta, frame=frame, expected=h.snapshot(),
                              ducking=duck_snapshot(), events=h.trace, updated=bool(updated),
                              counter_incremented=bool(h.get(h.audio + 0x22c))))
        cases.append(dict(before=before, duck_before=duck_before, steps=steps))
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-data', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--verify-fixture', type=Path)
    args = parser.parse_args()
    h = Harness.__new__(Harness)
    # Keep mapper source unchanged; PE import metadata is only needed by this harness.
    import pefile
    h.engine_pe = pefile.PE(str(args.game_data / 'System/engine.dll'))
    Harness.__init__(h, args.game_data)
    rng = random.Random(0x52435352)
    constructors = []
    for seed in [0, 1, 0x7fffffff, 0xfffffffe, 0xffffffff]:
        constructors.append(dict(seed=seed, expected=h.construct(seed), counter=h.get(0x107a8bec)))
    init = initialization(h, rng)
    data = dict(schema=1, hashes=h.hashes, constructors=constructors, initialization=init,
                lifecycle=lifecycle(h, init), base_updates=base_updates(h, rng), lookups=lookups(h, rng), composed=composed(h, rng))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, separators=(',', ':')) + '\n')
    if args.verify_fixture:
        assert json.loads(args.verify_fixture.read_text()) == data, 'Source fixture differs from original replay'
    report = dict(passed=True, constructors=len(constructors), initialization=len(init),
                  lifecycle_steps=sum(len(c['steps']) for c in data['lifecycle']),
                  base_updates=len(data['base_updates']), lookup_scenarios=len(data['lookups']),
                  match_probes=sum(len(c['matches']) for c in data['lookups']),
                  generated_id_probes=sum(len(c['generated_ids']) for c in data['lookups']), composed_steps=sum(len(c['steps']) for c in data['composed']), fixture_verified=bool(args.verify_fixture))
    args.output.with_suffix('.validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
