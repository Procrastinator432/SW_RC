# Originale Animationstracks aus UKX-Paketen

`crates/rc-package/src/skeletal_animation.rs` liest originale `Engine.MeshAnimation`-Exporte für SWRC-Paketversionen 151–159 mit Licensee-Version 1. Die Feldreihenfolge stammt aus UMeshAnimation.Serialize (`105149d0`), FMeshAnimSeq.Serialize (`10457df0`), FSkelAnimSeq.Serialize (`10513590`) und AnalogTrack.Serialize (`10508b70`) samt Array-Hilfsfunktionen. Kritische Offsets, Reihenfolge und Versionsgrenzen sind zusätzlich anhand des Originalassemblers geprüft.

## Gelesene Daten

Nach den vorhandenen UObject-Properties folgen das unverändert benannte Wort +28, Referenzknocheneinträge und Sequenzen. Ein Referenzknocheneintrag enthält einen Paketnamen und zwei 32-Bit-Wörter. Die Interpretation dieser Wörter als Linkup-/Hierarchieinformation wird nicht vorweggenommen.

Sequenzen enthalten Namen, Namensgruppen, FirstFrame, Frameanzahl, Notifies mit Zeit/Name/Objektreferenz, Rate, Mindestblendzeit und RandomizeStart-Byte sowie weitere originale Wörter. Ungeklärte Felder behalten ihre nativen Offsetnamen. Die Sequenzflags bei +54 bleiben ein vollständiges Wort. Alle Floatfelder werden als rohe Bits erhalten. Notify-Objektreferenzen und Paketnamen werden auf gültige Tabelleneinträge geprüft; es werden keine Notify-Funktionen ausgeführt.

Die Archive speichern jeden Track in dieser Reihenfolge:

1. Positionsmaßstab als vier Floatbits-Bytes.
2. CompactIndex-Anzahl und signierte i16-Positionstripel.
3. CompactIndex-Anzahl und unveränderte u16-Rotationstripel.
4. CompactIndex-Anzahl und Byte-Schlüsselzeiten.

Dies unterscheidet sich von der Feldreihenfolge im Laufzeitsnapshot. Der Decoder erzeugt daraus die vorhandenen `AnimationTrack`-Werte; Laufzeit-Anzahltags werden nicht aus Dateidaten erfunden. Die kompakten Anzahlen werden auf Negativwerte, Obergrenze und verbleibende Bytes geprüft. Leere Arrays bleiben leer. Der Sampler meldet weiterhin fehlende tatsächlich benötigte Schlüssel als Fehler.

## Originalbestand und Prüfung

Die lokale Installation enthält zwanzig UKX-Dateien. Davon liefern neunzehn Dateien die unterstützten Animationsexporte. Die unabhängige Pythonprüfung liest Paketnamen, Imports, Exports und alle nativen Felder erneut aus den Originalbytes.

| Daten | Anzahl |
|---|---:|
| Unterstützte MeshAnimation-Exporte | 166 |
| Referenzknocheneinträge | 4.930 |
| Sequenzen | 1.398 |
| Tracks | 61.310 |
| Komprimierte Rotationsschlüssel | 1.432.532 |
| Komprimierte Positionsschlüssel | 235.590 |
| Dauerbytes | 2.148.799 |
| Parsefehler / ungelesene Restbytes | 0 / 0 |

Jeder decodierte Track wird über einen FNV-1a-64-Fingerprint seiner kanonischen Felder gegen die unabhängig gelesenen Paketbytes geprüft. Dieser Fingerprint dient dem Datenvergleich; Quellen und Berichte erhalten zusätzlich SHA-256-Nachweise. Der Bericht enthält vollständige Sequenzmetadaten und Anzahlen/Fingerprints aller Tracks, ohne die gesamte Schlüsselmenge als JSON zu duplizieren.

Für jede Sequenz werden drei Samples ihres ersten gespeicherten Tracks bei Zeit 0, Frameanzahl/2 und Frameanzahl berechnet: insgesamt 4.194 erfolgreiche Samples mit endlichen Rotations-/Positionswerten. Die unabhängige Prüfung kontrolliert alle Schlüsselwahlen gegen die Dauerbytes. Die Quaternionberechnung verwendet die zuvor geprüfte portable Mathevariante; es gibt hier keinen nativen Pose-Referenzlauf. Track null ist ausdrücklich **noch keinem Mesh-Root zugeordnet**. Framezeiten sind direkt gewählt, keine Emulation des ursprünglichen x87-Aufrufers.

Vier neue Tests prüfen Track-Feldreihenfolge und signierte Werte, erhaltene Floatbits, leere Arrays, sämtliche Trunkierungen der Track-/Sequenzfixtures, negative Anzahlen, Namen, Gruppen, Notify-Objektreferenzen und Sequenzmetadaten. Insgesamt 252 Workspace-Tests (242 rc-package, 4 rc-native, 6 rc-render), Clippy mit Warnungen als Fehler und Formatprüfung bestanden.

```text
cargo run -p rc-inspect --bin rc-animation-track-check -- "D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Animations" analysis/reports/original-animation-tracks.json
python scripts/Record-OriginalAnimationTracks.py
```

Nachweise: `analysis/decompiled/skeletal-animation-serializers.c/.asm`, `skeletal-animation-arrays.c`, `mesh-sequence-arrays.c`, `analog-track-serialize.c`, `analog-track-arrays.c`, `analysis/reports/original-animation-tracks.json`, `original-animation-tracks-validation.json` und `analysis/evidence.json`. Paketdateien werden ausschließlich gelesen.

## Noch offene Arbeit

`beast.ukx` ist Version 148 und enthält einen weiteren MeshAnimation-Export. Der native Loader decodiert für Versionen unter 151 das ältere Rotationsformat und quantisiert es neu. Dies ist ausdrücklich als `UnsupportedLegacyRotation` ausgewiesen und nicht in den 166 erfolgreich decodierten Exporten enthalten. Versionen unter 141 besitzen darüber hinaus einen umfangreicheren Konvertierungspfad. Der aktuelle Decoder lehnt beide ab.

Referenzskelett-/SkeletalMesh-Decodierung, Aufbau der Mesh-Animation-Linkups, Zuordnung von Tracks zu Knochen, vollständige Pose/Blends, Skinning und sichtbare Animation sind weiterhin offen. Die erfolgreiche Datendecodierung bedeutet noch keine vollständige Spielwiedergabe. Androidprüfung bleibt am Schluss.

Fortsetzung: [Originales Referenzskelett und Linkups](ORIGINAL_SKELETAL_LINKUPS.md) liest jetzt 130 SkeletalMesh-Präfixe mit 3.111 Knochen, baut 225 aufgelöste Linkups und prüft 8.172 zugeordnete Root-Track-Samples. Die oben beschriebene frühere Track-null-Diagnose bleibt unverändert erhalten. Weitere Meshdaten, Vollpose und Skinning sind weiterhin offen.
