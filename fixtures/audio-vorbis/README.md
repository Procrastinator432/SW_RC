# Synthetische Vorbis-Fixtures

Diese Dateien enthalten ausschließlich selbst erzeugte Sinustöne beziehungsweise Stille. Keine Musik oder sonstige Originalassets. `host-tick.json` ist ein synthetischer vorbereiteter Zustand aus dem vorherigen Originalprüfrahmen; Streamressourcen wurden für die neue Registrierung geleert. `init-native.json` enthält 128 synthetische Eingaben/Ergebnisse der ganzen originalen Init-Methode, keine ausführbaren Originalbytes.

Erzeugung mit FFmpeg (jeweils `-f lavfi -i FILTER -c:a libvorbis -q:a 3 -y NAME.ogg`):

| Name | FILTER |
|---|---|
| mono | `sine=frequency=431:sample_rate=22050:duration=0.125` |
| stereo | `aevalsrc=0.18*sin(2*PI*307*t)\|0.13*sin(2*PI*701*t):s=44100:d=2` |
| clipped | `aevalsrc=1.15*sin(2*PI*503*t):s=22050:d=0.1` |

`silence.ogg`: `-f lavfi -i anullsrc=r=22050:cl=mono -t 0.01 -c:a libvorbis`.
`three-channel.ogg`: ebenso mit `cl=2.1`, nur als abzuweisender Kanalzahlfall.

Die Encodercontainer können bei erneuter Erzeugung andere Seriennummern enthalten. Maßgeblich sind die eingefrorenen Dateien mit Hashes in `manifest.json`. Referenzbytes: `soundfile.read(dtype='float32', always_2d=True)`, danach `numpy.clip(numpy.rint(samples.astype(float64)*32768), -32768, 32767).astype('<i2').tobytes()`. Das verwendet einen unabhängigen Vorbisdecoder mit expliziter Sättigung. [Prüfumfang und Grenzen](../../docs/AUDIO_VORBIS.md).
