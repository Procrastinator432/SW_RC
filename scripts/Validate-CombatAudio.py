"""Live DLL proof: events survive queries/short buffers and are consumed exactly once."""
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'analysis/milestone3'
api = c.CDLL(str(ROOT / 'target/release/rc_native.dll'))
api.rc_game_create.argtypes = [c.c_void_p, c.c_size_t, c.c_void_p, c.c_size_t]
api.rc_game_create.restype = c.c_uint64
api.rc_game_audio.argtypes = [c.c_uint64, c.c_uint32, c.c_void_p, c.c_size_t]
api.rc_game_audio.restype = c.c_ssize_t
api.rc_game_advance.argtypes = [c.c_uint64, c.c_double, c.c_float, c.c_float, c.c_float, c.c_float, c.c_uint32]
api.rc_game_advance.restype = c.c_int
api.rc_game_command.argtypes = [c.c_uint64, c.c_uint32]
api.rc_game_release.argtypes = [c.c_uint64]
raw = (OUT/'geo_01a-combat.rcplay').read_bytes()
data = c.create_string_buffer(raw)
error = c.create_string_buffer(512)
game = api.rc_game_create(data, len(raw), error, len(error))
assert game, error.value

def packet(kind):
    n = api.rc_game_audio(game, kind, None, 0)
    assert n >= 12
    short = c.create_string_buffer(b'\xa5'*(n+16), n+16)
    assert api.rc_game_audio(game, kind, short, n-1) == n
    assert short.raw == b'\xa5'*(n+16)
    assert api.rc_game_audio(game, kind, None, 0) == n
    assert api.rc_game_audio(game, kind, short, n) == n
    assert short.raw[n:] == b'\xa5'*16
    return short.raw[:n]

try:
    bank = packet(0)
    assert bank[:8] == b'RCSND001'
    assert struct.unpack_from('<I', bank, 8)[0] == 4
    assert bank == packet(0)
    assert api.rc_game_audio(game, 2, None, 0) == -1
    assert api.rc_game_audio(game, 0, None, 1) == -1
    assert api.rc_game_audio(0, 0, None, 0) == -1
    assert api.rc_game_advance(game, 1/60, 0, 0, 0, 0, 2) == 1
    event = packet(1)
    assert event[:8] == b'RCAUD001' and struct.unpack_from('<I', event, 8)[0] == 1
    assert struct.unpack_from('<QQIi', event, 12) == (1, 1, 0, -1)
    assert len(packet(1)) == 12
    assert api.rc_game_command(game, 1) == 0
    assert len(packet(1)) == 12
    assert api.rc_game_advance(game, 1/60, 0, 0, 0, 0, 2) == 1
    assert packet(1) == event
    assert api.rc_game_command(game, 1) == 0
    assert api.rc_game_advance(game, 1/60, 0, 0, 0, 0, 2) == 1
    assert api.rc_game_command(game, 0) == 0
    assert len(packet(1)) == 12
    (OUT/'audio-abi-validation.json').write_text(json.dumps({
        'sound_bank_count': 4, 'bank_sha256': hashlib.sha256(bank).hexdigest(),
        'query_and_short_copy_preserve_events': True, 'successful_copy_consumes_once': True,
        'reset_restarts_sequence': True, 'suspend_discards_stale_events': True,
        'invalid_handles_kinds_pointers_rejected': True, 'copy_canaries_intact': True,
    }, indent=2), encoding='utf-8')
    print('Audio ABI passed: original WAV bank, consume-once, canaries, reset and suspend')
finally:
    api.rc_game_release(game)
