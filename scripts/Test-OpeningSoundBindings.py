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
    refs.extend(dict(kind='bacta-notify', original=n['original'], role=n['role'])
                for n in opening.get('bacta', {}).get('notifies', []))
    if opening.get('bacta', {}).get('ambient_loop') is not None:
        refs.append(dict(kind='bacta-ambient', original=736, role=opening['bacta']['ambient_loop']))
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
    owned = bool(opening.get('sound_policies'))
    suppressed_caption_checked = False
    try:
        n = api.rc_game_audio(handle, 0, None, 0)
        assert n > 12
        assert n <= 8*1024*1024+1024, 'Bank exceeds the Android JNI packet limit'
        bank_bytes = n
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
                padding = 0.1 if opening.get('sound_policies') else 0
                assert ref['seconds'] == 0 or abs(ref['seconds'] - seconds - padding) < 1e-6
            ref.update(source=identity['source'], pcm_seconds=seconds,
                       sha256=hashlib.sha256(pcm).hexdigest())
        enemy = next(s for s in opening['sounds'] if s['source'] == opening['enemy_fire_source'])
        encoded = enemy['wave']
        enemy_pcm = base64.b64decode(encoded[7:]) if isinstance(encoded, str) else bytes(encoded)
        assert sounds.count(enemy_pcm) == 1, 'Enemy cue must remain uniquely bound in the native bank'
        if a.ticks:
            api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float,
                                          c.c_float, c.c_float, c.c_float, c.c_uint32]
            names = {b['pcm']: b['source'] for b in expected}
            for _ in range(a.ticks):
                assert api.rc_game_advance(handle, 1 / 60, 0, 0, 0, 0, 0) == 1
                kind = 2 if owned else 1
                legacy_size = api.rc_game_audio(handle, 1, None, 0)
                n = api.rc_game_audio(handle, kind, None, 0)
                if owned:
                    assert n == 12 + ((legacy_size - 12) // 24) * 32
                    tiny = c.create_string_buffer(1)
                    assert api.rc_game_audio(handle, kind, tiny, 1) == n
                    assert api.rc_game_audio(handle, 1, None, 0) == legacy_size
                events = c.create_string_buffer(n)
                assert api.rc_game_audio(handle, kind, events, n) == n
                assert api.rc_game_audio(handle, 1, None, 0) == 12, 'Full copy must consume both views'
                assert events.raw[:8] == (b'RCAUD002' if owned else b'RCAUD001')
                for at in range(12, n, 32 if owned else 24):
                    seq, tick, role, owner = struct.unpack_from('<QQIi', events.raw, at)
                    category, protected = struct.unpack_from('<II', events.raw, at+24) if owned else (2, 0)
                    pcm = sounds[role]
                    with wave.open(io.BytesIO(pcm), 'rb') as f:
                        seconds = f.getnframes() / f.getframerate()
                    timeline.append(dict(sequence=seq, tick=tick, role=role, owner=owner, category=category, protected=bool(protected),
                                         source=names.get(fingerprint(pcm), 'other'), seconds=seconds))
                    if owned and names.get(fingerprint(pcm), '').endswith('.CA1G1541'):
                        api.rc_game_checkpoint.argtypes = [c.c_uint64, c.c_void_p, c.c_size_t]
                        api.rc_game_checkpoint.restype = c.c_ssize_t
                        size = api.rc_game_checkpoint(handle, None, 0)
                        saved = c.create_string_buffer(size)
                        assert api.rc_game_checkpoint(handle, saved, size) == size
                        state = json.loads(saved.raw[20:])['state']['opening']
                        assert any('BlockedByOwnedVoice' in e[2] for e in state['events'])
                        assert not any('still scanning' in caption[0] for caption in state['captions'])
                        assert 'primary target' in state['subtitle']
                        suppressed_caption_checked = True
            if owned and a.ticks >= 1800:
                assert suppressed_caption_checked
                assert not any(e['source'].endswith('.CA1G1545') for e in timeline)
                last = {}
                for event in timeline:
                    if event['category'] != 1: continue
                    previous = last.get(event['owner'])
                    if previous:
                        assert event['tick'] / 60 >= previous['tick'] / 60 + previous['seconds'], (previous, event)
                    last[event['owner']] = event
    finally:
        api.rc_game_release(handle)
    report = dict(passed=True, scene_sha256=hashlib.sha256(raw).hexdigest(),
                  native_bank_bytes=bank_bytes,
                  cue_references=len(refs), unique_cues=len(identities), owned_voice_semantics=owned,
                  suppressed_caption_checked=suppressed_caption_checked, cues=refs, timeline=timeline)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ['cues', 'timeline']}))


if __name__ == '__main__':
    main()
