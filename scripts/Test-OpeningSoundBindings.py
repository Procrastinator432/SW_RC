"""Check every opening/intro/rappel cue against the native final scene sound bank."""
import argparse
import base64
import ctypes as c
import hashlib
import io
import json
import struct
import wave
import zipfile
from pathlib import Path


def fingerprint(data):
    value = 0xCBF29CE484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f'{value:016x}'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('scene', type=Path, help='Final RCPLAY or campaign ZIP')
    p.add_argument('--dll', type=Path, default=Path('target/release/rc_native.dll'))
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--ticks', type=int, default=0)
    a = p.parse_args()
    raw = a.scene.read_bytes()
    if zipfile.is_zipfile(a.scene):
        with zipfile.ZipFile(a.scene) as z:
            raw = z.read('geo_01a.rcplay')
    assert raw[:8] == b'RCPLAY13', 'Expected final A opening layer'
    opening = json.loads(raw[12:12 + struct.unpack_from('<I', raw, 8)[0]])
    expected = opening['sound_bindings']
    assert expected, 'No cue identity manifest'
    refs = []
    for program in opening['programs']:
        for action in program['actions']:
            op = action['op']
            if op['kind'] == 'Sound':
                refs.append(dict(op, kind='script', original=action['original']))
    refs.extend(dict(kind='intro', original=i, role=r)
                for i, r in enumerate(opening['intro']['sounds']))
    refs.extend(dict(kind='rappel-notify', original=n['original'], role=n['role'])
                for n in opening['rappel']['notifies'] if n['role'] is not None)
    identities = {b['role']: b for b in expected}
    assert len(identities) == len(expected)
    assert set(identities) == {r['role'] for r in refs}, 'Incomplete cue manifest'
    api = c.CDLL(str(a.dll.resolve()))
    api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
    api.rc_game_create.restype = c.c_uint64
    api.rc_game_audio.argtypes = [c.c_uint64, c.c_uint32, c.c_void_p, c.c_size_t]
    api.rc_game_audio.restype = c.c_ssize_t
    api.rc_game_release.argtypes = [c.c_uint64]
    buf, err = c.create_string_buffer(raw), c.create_string_buffer(1024)
    handle = api.rc_game_create(buf, len(raw), err, len(err))
    assert handle, err.value
    timeline = []
    try:
        n = api.rc_game_audio(handle, 0, None, 0)
        assert n > 12
        bank = c.create_string_buffer(n)
        assert api.rc_game_audio(handle, 0, bank, n) == n
        data, at, sounds = bank.raw, 12, []
        assert data[:8] == b'RCSND001'
        for _ in range(struct.unpack_from('<I', data, 8)[0]):
            size = struct.unpack_from('<I', data, at)[0]
            at += 4
            sounds.append(data[at:at + size])
            at += size
        assert at == len(data)
        for ref in refs:
            identity = identities[ref['role']]
            pcm = sounds[ref['role']]
            assert fingerprint(pcm) == identity['pcm'], (ref, identity)
            with wave.open(io.BytesIO(pcm), 'rb') as f:
                seconds = f.getnframes() / f.getframerate()
            if ref['kind'] == 'script':
                assert abs(ref['caption_seconds'] - seconds) < 1e-6, (ref, seconds)
                assert ref['seconds'] == 0 or abs(ref['seconds'] - seconds) < 1e-6
            ref.update(source=identity['source'], pcm_seconds=seconds,
                       sha256=hashlib.sha256(pcm).hexdigest())
        enemy = next(s for s in opening['sounds'] if s['source'] == opening['enemy_fire_source'])
        encoded = enemy['wave']
        enemy_pcm = base64.b64decode(encoded[7:]) if isinstance(encoded, str) else bytes(encoded)
        assert sounds[-1] == enemy_pcm, 'Enemy cue must be appended after all original cues'
        if a.ticks:
            api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float,
                                          c.c_float, c.c_float, c.c_float, c.c_uint32]
            names = {b['pcm']: b['source'] for b in expected}
            for _ in range(a.ticks):
                assert api.rc_game_advance(handle, 1 / 60, 0, 0, 0, 0, 0) == 1
                n = api.rc_game_audio(handle, 1, None, 0)
                events = c.create_string_buffer(n)
                assert api.rc_game_audio(handle, 1, events, n) == n
                for at in range(12, n, 24):
                    seq, tick, role, owner = struct.unpack_from('<QQIi', events.raw, at)
                    pcm = sounds[role]
                    with wave.open(io.BytesIO(pcm), 'rb') as f:
                        seconds = f.getnframes() / f.getframerate()
                    timeline.append(dict(sequence=seq, tick=tick, role=role, owner=owner,
                                         source=names.get(fingerprint(pcm), 'other'), seconds=seconds))
    finally:
        api.rc_game_release(handle)
    report = dict(passed=True, scene_sha256=hashlib.sha256(raw).hexdigest(),
                  cue_references=len(refs), unique_cues=len(identities), cues=refs, timeline=timeline)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ['cues', 'timeline']}))


if __name__ == '__main__':
    main()
