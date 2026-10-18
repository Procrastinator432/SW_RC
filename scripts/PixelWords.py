"""Read legacy JSON words or losslessly packed little-endian material pixels."""
import base64
import struct


def pixel_words(value):
    if isinstance(value, list):
        return value
    if not isinstance(value, str) or not value.startswith("base64:"):
        raise ValueError("Invalid packed material encoding")
    raw = base64.b64decode(value[7:], validate=True)
    if len(raw) % 4 or len(raw) > 8 * 1024 * 1024 * 4:
        raise ValueError("Invalid packed material pixels")
    return (word for (word,) in struct.iter_unpack('<I', raw))
