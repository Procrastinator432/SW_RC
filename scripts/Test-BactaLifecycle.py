"""Exercise the real station via recorded player inputs, including interrupted use and saved animations."""
import argparse
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('scene', type=Path)
    p.add_argument('replay', type=Path)
    p.add_argument('--dll', type=Path, default=Path('target/release/rc_native.dll'))
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--capture-views', action='store_true', help='Write ordinary player camera PNGs at station phase changes')
    a = p.parse_args()
    raw, replay = a.scene.read_bytes(), a.replay.read_bytes()
    opening = json.loads(raw[12:12+struct.unpack_from('<I',raw,8)[0]])
    notify_roles = {n['original']:n['role'] for n in opening['bacta']['notifies']}
    controller = opening['bacta'].get('controller')
    assert replay[:8] == b'RCINPUT1' and (len(replay)-12) % 20 == 0
    rows = list(struct.iter_unpack('<ffffI', replay[12:]))
    api = c.CDLL(str(a.dll.resolve()))
    api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
    api.rc_game_create.restype = c.c_uint64
    api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float, c.c_float, c.c_float, c.c_float, c.c_uint32]
    api.rc_game_checkpoint.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
    api.rc_game_checkpoint.restype = c.c_ssize_t
    api.rc_game_restore_checkpoint.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
    api.rc_game_audio.argtypes = [c.c_uint64, c.c_uint32, c.c_void_p, c.c_size_t]
    api.rc_game_audio.restype = c.c_ssize_t
    api.rc_game_release.argtypes = [c.c_uint64]
    api.rc_game_draw.argtypes = [c.c_uint64, c.c_size_t, c.c_size_t, c.c_void_p, c.c_size_t]
    api.rc_game_draw.restype = c.c_ssize_t
    api.rc_game_command.argtypes = [c.c_uint64, c.c_uint32]
    error = c.create_string_buffer(1024)

    def create():
        b = c.create_string_buffer(raw)
        h = api.rc_game_create(b, len(raw), error, len(error))
        assert h, error.value
        return h

    def save(h):
        n = api.rc_game_checkpoint(h, None, 0)
        assert n > 20
        b = c.create_string_buffer(n)
        assert api.rc_game_checkpoint(h, b, n) == n
        return b.raw

    def restore(h, b):
        data = c.create_string_buffer(b)
        assert api.rc_game_restore_checkpoint(h, data, len(b), error, len(error)) == 0, error.value

    def state(h):
        return json.loads(save(h)[20:])['state']

    def step(h, row):
        result = api.rc_game_advance(h, 1/60, *row)
        assert result == 1, (result, state(h)['statistics']['ticks'], state(h)['phase'])

    def pixels(h, width=160, height=90):
        b = (c.c_uint32 * (width*height))()
        assert api.rc_game_draw(h, width, height, b, width*height) == width*height
        return bytes(b)

    def audio(h):
        n = api.rc_game_audio(h, 2, None, 0)
        b = c.create_string_buffer(n)
        assert api.rc_game_audio(h, 2, b, n) == n
        return list(struct.iter_unpack('<QQIiII', b.raw[12:]))

    h = create()
    phases, notify_ids, checks, transitions = set(), set(), [], []
    station_audio = []
    probe_save = None
    last = None
    try:
        for i, row in enumerate(rows):
            step(h, row)
            # Drain PCM events as the Android host does; the checkpoint still
            # carries the animation channel and once-only notify history.
            events = audio(h)
            for sequence,tick,role,owner,category,flags in events:
                if owner == 736:
                    assert category == 2 and flags == 0
                    station_audio.append(dict(tick=tick, role=role))
            if not 2720 <= i <= 3100:
                continue
            s = state(h)
            b = s['opening']['bacta']
            phase = b['phase']
            if phase != last:
                transitions.append(dict(tick=i+1, phase=phase, health=s['mission']['health']))
                last = phase
                if a.capture_views:
                    from PIL import Image
                    views = a.output.parent / 'player-views'
                    views.mkdir(parents=True, exist_ok=True)
                    image = Image.frombytes('RGBA', (720,405), pixels(h,720,405), 'raw', 'BGRA').convert('RGB')
                    image.save(views / f'{i+1:05}-{phase}.png')
                    (views / f'{i+1:05}-{phase}.json').write_text(json.dumps(s, indent=2)+'\n', encoding='utf-8')
            if phase == 'Loading':
                assert s['mission']['health'] == 25, 'Healing started before FinishAnim'
            for tick, original in b['notify_events']:
                notify_ids.add(original)
            if i == 2750:
                probe_save = save(h)
            if phase not in phases:
                phases.add(phase)
                saved = save(h)
                other = create()
                try:
                    restore(other, saved)
                    assert save(other) == saved
                    assert pixels(other) == pixels(h)
                    # Continue both branches without altering the main replay.
                    baseline = create()
                    try:
                        restore(baseline, saved)
                        for future in rows[i+1:i+91]:
                            step(baseline, future)
                            step(other, future)
                        assert save(other) == save(baseline)
                        assert pixels(other) == pixels(baseline)
                    finally:
                        api.rc_game_release(baseline)
                finally:
                    api.rc_game_release(other)
                checks.append('checkpoint-' + phase)
            if phase == 'Depleted':
                assert s['mission']['completed'] and s['mission']['health'] == 100
        final = state(h)
        if final['phase'] != 'Won':
            a.output.parent.mkdir(parents=True, exist_ok=True)
            a.output.with_name('incomplete-A-state.json').write_text(json.dumps(final,indent=2)+'\n',encoding='utf-8')
        assert final['phase'] == 'Won', final['phase']
        assert station_audio == [dict(tick=t, role=notify_roles[n]) for t,n in final['opening']['bacta']['notify_events']], station_audio
        assert len(station_audio) == 6
        assert phases == {'Standby', 'Loading', 'Active', 'Holstering', 'Depleted'}, phases
        assert notify_ids == {11, 12, 13, 14, 15, 16}, notify_ids
        assert probe_save
        probe = create()
        try:
            restore(probe, probe_save)
            for damage in ['missing-module', 'invalid-channel', 'future-notify']:
                value = json.loads(probe_save[20:])
                b = value['state']['opening']['bacta']
                if damage == 'missing-module':
                    value['state']['opening'].pop('bacta')
                elif damage == 'invalid-channel':
                    b['playback']['channels'][0]['words'][6] = 0x7fc00000
                else:
                    b['notify_events'].append([20000000, 11])
                body = json.dumps(value, separators=(',', ':')).encode()
                checksum = 0xcbf29ce484222325
                for byte in body:
                    checksum = ((checksum ^ byte)*0x100000001b3) & 0xffffffffffffffff
                bad = b'RCSAVE01' + struct.pack('<IQ', len(body), checksum) + body
                data = c.create_string_buffer(bad)
                assert api.rc_game_restore_checkpoint(probe, data, len(bad), error, len(error)) != 0
                assert save(probe) == probe_save, 'Invalid checkpoint changed live state'
                checks.append('reject-' + damage)
            modes = [('release', (0,0,0,0,0))] if controller else [('release', (0,0,0,0,0)), ('look-away', (0,0,3.14,0,16)), ('leave-range', (-1,0,0,0,16))]
            if controller:
                reference = create()
                try:
                    restore(reference, probe_save)
                    restore(probe, probe_save)
                    for _ in range(40):
                        step(reference,(0,0,0,0,16))
                        step(probe,(1,1,3.14,2,127))
                    assert save(probe) == save(reference), 'UsingItem must ignore movement/look/weapon/jump inputs'
                    assert state(probe)['mission']['health'] > 25
                    checks.append('using-item-ignores-free-controls')
                finally:
                    api.rc_game_release(reference)
                modes.append(('release-leave-range',(-1,0,0,0,0)))
            for name, first in modes:
                restore(probe, probe_save)
                # Pause consumes no simulation time or queued input; first
                # released-use tick starts the original Holster animation.
                if name == 'release':
                    assert api.rc_game_command(probe, 0) == 0
                step(probe, first)
                interrupted_health = None
                for _ in range(120):
                    step(probe, first if 'leave-range' in name else (0,0,0,0,first[-1]))
                    current = state(probe)
                    if not current['mission']['using'] and interrupted_health is None:
                        interrupted_health = current['mission']['health']
                s = state(probe)
                assert interrupted_health is not None
                assert s['mission']['health'] == interrupted_health, (name, s['mission']['health'])
                if name != 'leave-range':
                    assert s['mission']['health'] == 25
                assert s['opening']['bacta']['phase'] == 'Standby', (name, s['opening']['bacta'])
                checks.append(name + '-no-heal')
            restore(probe, probe_save)
            for _ in range(6): step(probe, (-1,0,0,0,0))
            for _ in range(60): step(probe, (0,0,0,0,0))
            for _ in range(65): step(probe, (0,0,0,0,16))
            s = state(probe)
            assert s['opening']['bacta']['phase'] == 'Active' and s['mission']['health'] > 25
            checks.append('step-back-use')
            if a.capture_views:
                image = Image.frombytes('RGBA', (720,405), pixels(probe,720,405), 'raw', 'BGRA').convert('RGB')
                image.save(views / 'step-back-Active.png')
                (views / 'step-back-Active.json').write_text(json.dumps(s,indent=2)+'\n',encoding='utf-8')
            restore(probe, probe_save)
            for _ in range(90): step(probe, (0,0,0,0,0))
            for _ in range(300): step(probe, (0,0,0,0,16))
            s = state(probe)
            assert s['mission']['completed'] and s['mission']['health'] == 100
            assert s['opening']['bacta']['phase'] == 'Depleted'
            checks.append('release-reuse-completes')
        finally:
            api.rc_game_release(probe)
    finally:
        api.rc_game_release(h)
    report = dict(passed=True, scene_sha256=hashlib.sha256(raw).hexdigest(), replay_sha256=hashlib.sha256(replay).hexdigest(), transitions=transitions, original_notify_ids=sorted(notify_ids), station_audio=station_audio, checks=checks, full_map_a_replay_ticks=len(rows), map_a_completed=True, limits='No Android device/audio or particle-effect acceptance')
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
