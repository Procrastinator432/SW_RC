# ApplyAnimChannel auf vorbereiteten Puffern

Original: `engine.dll` USkeletalMeshInstance::ApplyAnimChannel **10501420**.
Rust: `crates/rc-package/src/skeletal_channel.rs`.

Die Funktion arbeitet auf einem ausgewählten Kanal, einem bereits aufgelösten
Sequenz-/Linkup-Snapshot, Referenzpose, vorheriger Instanzpose und vorbereiteter
Scratchpose. Sequenzlookup, globale Scratchallokation und Kanalaktivitätsauswahl
liegen davor. Eine fehlende Sequenz oder ein leerer maskierter Trackcount wird
als `frames=None` übergeben; das Ergebnis ist false, die History wird trotzdem
fortgeschrieben. `Some(frames)` steht für eine vorhandene Sequenz mit Tracks.

## Rekonstruierte Regeln

- Kanalindex 0 benutzt Gewicht 1; andere benutzen Floatwort +38.
- Frames +1c/+30 werden auf [0,1] begrenzt, NaN wird 0, -0 bleibt erhalten.
  Die Multiplikation mit dem Sequenzframecount erfolgt hier nachweislich per SSE.
- Ease(t) ist 2*t*t bis .5, danach 1-2*(1-t)^2. Kein Clamp.
  Übergangsfortschritt ist (Ease(+2c)-Ease(+34))/(1-Ease(+34)); +2c==1
  erzwingt Fortschritt 1, auch bei alter History 1.
- Knochenintervall ist [+3c,+40). Negative Trackzuordnung kopiert die Referenzpose
  unmittelbar, unabhängig von Gewicht und Blendwert. Sonst kopiert Blend==0
  unmittelbar die vorherige Instanzpose.
- Übrige Knochen sampeln die aktuelle Trackzeit. Bei Blend<1 und Index>Move-Bone
  wird zusätzlich die vorherige Trackzeit gesampelt. Eine Winkel- und
  Positionsdistanz begrenzt den Übergang von der vorherigen Instanzpose zum Ziel.
  Positionslängen verwenden z²+y²+x² und RSQRTSS mit einer Newtonkorrektur;
  das Ergebnis wird bei Distanzquadrat 0 nach der Rechnung auf +0 maskiert.
- Gewicht>=1 **oder NaN** kopiert das Ziel. Bei kleinerem Gewicht bekommt Slerp
  Ease(Blend)*Gewicht; Position bekommt nur Gewicht. Der Decompiler unterschlägt
  den versteckten Slerp-Parameter, daher gilt hier die Assemblerbelegung.
- Normales Ende schreibt +2c nach +34 und +1c nach +30, auch bei leerem Intervall.
  Rust-Fehler erhalten vorherige Knochenänderungen; die History wird dann nicht
  fortgeschrieben. Ungültige Puffer werden als Fehler statt als Speicherzugriff
  außerhalb des Puffers behandelt.

## Bewusste Hostgrenzen

`SkeletalChannelHost` liefert Trackpose, Slerp, RSQRTSS-Seed und Winkelbudget.
Der bestehende Track-/Quaternioncode kann anschließend darüber angebunden
werden. Die Probe liefert feste Werte, um die Steuerung unabhängig von diesen
mathematischen Komponenten zu prüfen. Sie behauptet keine Original-Spielpose.

`core.dll` AngleDiffFast **1011feb0** wurde zusätzlich exportiert. Es summiert
Quaternionprodukte in x87-Reihenfolge w,z,y,x, nimmt den Betrag, begrenzt ihn auf
1 und liefert 2*acos. ApplyAnimChannel vergleicht die ungerundete x87-Summe
Angle(old,target)*progress+Angle(target,previousSample) mit dem gespeicherten
ersten Winkel; die anschließende Division verwendet den f32-Speicherwert dieser
Summe. Diese CPU-Präzisionsgrenze bleibt ausdrücklich im Host, statt sie durch
eine angeblich bitgenaue portable acos-Rechnung zu ersetzen.

## Prüfung

Sieben gezielte Tests prüfen Fallbackvorrang, History, Teiländerungen bei Fehlern,
Frame-/NaN-Grenzen, unterschiedliche Blendgewichte und Positionslimit.
`rc-skeletal-channel-probe` erzeugt 3072 vorbereitete Fälle mit Sequenzzustand,
Gewicht, Blendwert, Move-Index, Trackzuordnung, Winkelhost und Hostfehlerposition.
`scripts/Record-SkeletalChannel.py` berechnet Ergebnisse, Rohwort-History und
Aufrufreihenfolge unabhängig und prüft zentrale Originalinstruktionen.

Die echte Track-Anbindung mit austauschbarer Winkel-Policy und Originaldaten-
Diagnose ist inzwischen in [ORIGINAL_CHANNEL_POSES.md](ORIGINAL_CHANNEL_POSES.md)
ergänzt. Die hier beschriebene Probe mit festen Hostwerten bleibt eine getrennte
Prüfung der Steuerung und Fehlergrenzen.

Offen: native x87-Winkelrechnung,
restliche ApplyAnimation-Vorbereitung/Directors/Bounds, Skinning. Androidprüfung
erst am Projektende. Dies ist ein weiterer Animationsbaustein, kein spielbarer Port.
