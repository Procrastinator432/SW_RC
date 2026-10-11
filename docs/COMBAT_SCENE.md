# DC-17m-Kampf und Originalgeräusche – Meilenstein 3, erster Integrationsabschnitt

Android **0.11.0-combat-scene**, versionCode **13**, 10. Oktober 2026.
Die DC-17m verwendet jetzt ausgewählte Originaldefaults, fliegende Projektile und Originalgeräusche. **M3 ist damit noch nicht vollständig abgenommen:** native Laserbolzenrechnung, ursprüngliche Schadensmodifikatoren und Geonosian-KI sind weiterhin offen.

## Ausprobieren

1. `android/app/build/outputs/apk/debug/app-debug.apk` installieren bzw. aktualisieren.
2. **`analysis/milestone3/geo_01a-combat.rcplay`** auf das Gerät kopieren.
3. **Spielabschnitt geo_01a öffnen → Szene öffnen** und die neue Datei auswählen.
4. Drei Gegner ausschalten und die grüne Exfil-Markierung erreichen. Magazin: 60 Schuss; Nachladen über **Laden** oder automatisch bei leerem Magazin. GPU/CPU, Pause und Neustart bleiben verfügbar.

Die ursprünglichen `RCPLAY01`- und `RCPLAY02`-Dateien bleiben mit ihren bisherigen Kampfregeln verwendbar. Die neuen Regeln sind an das zusätzliche **RCPLAY03**-Szenenpaket gebunden. Originalassets befinden sich weiterhin im lokalen Szenenpaket, nicht in der APK.

Der Nutzer bestätigt, dass die Vorgängerversion mit animierten Originalmodellen auf seinem Gerät funktioniert. Die neue Kampfversion benötigt noch seinen persönlichen Test; eine quantitative ARM64-Dauerabnahme wurde weiterhin nicht durchgeführt.

## Belegte Daten und ausgeführter Pfad

`rc-combat-assets` löst die Klassendefaults einschließlich Properties-Overlays aus den Originalpaketen auf. `combat-assets.json` enthält Wert, deklarierende Klasse, Paket, Offset und Übernahmestufe. `rc-play-combat` erstellt das neue Paket aus diesen Daten und der unveränderten M2-Szene; `combat-receipt.json` hält Soundreferenzen, Notifyobjekte und nicht ausgeführte Ereignisse fest.

| Eigenschaft | Originalbefund | Verwendung |
|---|---|---|
| Waffenklasse | `CTInventory.DC17mBlaster`, `FireMode=FM_Automatic` | Halten feuert wiederholt |
| Magazin / Gesamtmunition | `ReloadCount=60`, `DC17mBlasterAmmo.MaxAmmo=300`, `InitAmmoAmount=-1` | Start 60 im Magazin und 240 Reserve; Gesamtbestand sinkt nur beim Schuss |
| Schussabstand | `Engine.Weapon.ShotDelay`, f32 0,10000000149 s | 60-Hz-Adapter mit Toleranz 1e-8 s am Timerende; zehn Schüsse/s |
| Trefferpfad | `DC17mBlasterAmmo.bInstantHit=false`, `ProjectileClass=DC17BlasterProj` | Bewegtes Projektil statt bisherigem Soforttreffer |
| Projektilschaden | `Properties.DC17BlasterProjDefaults.Damage=20` | 20 Basisschaden je Körperkontakt |
| Laserbolzen | `bLaserBolt=true`, `AccelFactor=60000`, `Speed=6000`, `MaxSpeed=10000`, `LifeSpan=10` | Startgeschwindigkeit null laut `Projectile.PostBeginPlay`; eigene beschleunigte Bewegung mit Geschwindigkeitsgrenze und Lebensdauer |
| Gegnergesundheit | `CTCharacters.GeonosianWarrior.Health=100` | Fünf Basistreffer statt bisher zwei |
| Nachladeclip | `HudArmsDC17mSet.Reload`, 45 Frames / 30 Hz | Originalrate; 44/30 s bis zum letzten Channel-Frame statt bisher 1,2 s |

Gespeicherte Originalquellen liefern die Kette `PlayerController.Fire → Pawn.Fire → Weapon.Fire/ServerFire → CalculateFire → Ammunition.DoFire → CreateProjectile → Projectile.PostBeginPlay/ProcessTouch`. `DoFire` verbraucht eine Patrone; `ServerFire` verringert den Magazinstand. Der ausgewählte Munitionspfad erzeugt ein Projektil. `Range=5000` gehört zum Instant-Hit-Zweig und begrenzt diesen Projektiladapter deshalb nicht. `ProcessTouch` verursacht Schaden und zerstört das Projektil. Der originale Nachladezustand füllt das Magazin aus dem noch vorhandenen Gesamtbestand nach `AnimEnd`.

Diese gespeicherten Quellen und Defaults sind Belege für die Auswahl. Ein vollständiger Abgleich dieser Kette mit Bytecode und sämtlichen nativen Aufrufen ist **noch ausstehend**.

## Animation und Audio

Vier unveränderte PCM-WAV-Samples werden aus `gen_sound.uax` übernommen: DC-17m-Schuss, Nachladen und zwei Geonosian-Körperfallvarianten. Der Schuss startet mit dem Rust-Schussereignis. Nachladen und Körperfall starten beim Überqueren der originalen normalisierten Notifyzeit im festen Spieltakt.

Eintritt, halb offene Zeitintervalle, Loopgrenzen und gehaltenes Schlussbild verhindern doppelte Ereignisse. Jede Gegnerinstanz führt ihre eigene Zeit. Der Rust-Zustand enthält monoton nummerierte Soundereignisse einschließlich Tick und Besitzer. `rc_game_audio` liefert Größenabfragen und kurze Kopien ohne Verbrauch; erst eine vollständige Kopie verbraucht die Ereignisse. Pause verwirft ausstehende Ereignisse; Neustart setzt auch die Sequenznummer zurück.

Android lädt die WAV-Dateien über einen eigenen `SoundPool`, wartet auf alle Ladebestätigungen und spielt die Ereignisse einmalig ab. Temporäre Cachedateien werden nach dem Laden entfernt; Pause, Szenenwechsel und Zerstörung stoppen Streams bzw. geben den Pool frei. Ältere Szenen behalten ihre synthetischen Rückmeldungen. Die neue Szene verwendet ausschließlich die aufgelösten Originalsamples.

Die Audio-Host-Grenzen sind ausdrücklich: Lautstärke 0,35 ist eine Portierungswahl; native AudioParams, Distanzdämpfung, Raumklang, Prioritäts- und Likelihood-Regeln sind nicht nachgebaut. `SoundMultiple` verwendet derzeit die erste aufgelöste Originalvariante. Die Ereigniswarteschlange behält höchstens 256 Einträge. Android-Nachweise belegen Sampleladen und erfolgreiche Wiedergabeaufrufe, keinen subjektiven Hörtest oder gemessene Audiolatenz.

## Verbleibende Adapter und nächster M3-Abschnitt

- Die native Laserbolzenintegration ist noch nicht rekonstruiert. Der Adapter beschleunigt im festen Schritt, begrenzt die Geschwindigkeit und prüft den gesamten Weg gegen statische Welt und lebende Gegner. Es gibt keine native Schritt-/Gleit-/Reflexionsparität. Bolzen werden als kleine blaue Geometrie erst ab 80 Einheiten Kameradistanz dargestellt; die ursprünglichen Emitter fehlen.
- Schussursprung ist weiterhin das vorbereitete Spielerauge. Originaler `FPFireOffset`, Näheprüfung vor dem Spawn, Zielhilfe, Rückstoß und Knochen-/Schwierigkeits-/DamageType-Modifikatoren fehlen.
- Gegnerverhalten bleibt der bestehende Adapter: Sichtlinie bis 1600, direkte kollisionsgeprüfte Annäherung mit 65 Einheiten/s, 5 Spielerschaden alle 1,4 s. Originalwerte wie Sichtweite 5000 und GroundSpeed 450 sind inventarisiert, aber ohne den zugehörigen nativen Zustandsablauf nicht eingesetzt. Pfadfindung und die eigentliche Krieger-Angriffsart sind als Nächstes bis zu ihren nativen Abhängigkeiten aufzulösen.
- Trefferkörper bleibt 22/22/60 bei der aus M2 übernommenen Modellskalierung. Originalradius 50/Halbhöhe 115 wurden nicht unbesehen auf die vorbereitete Begegnung übertragen. Ragdoll-Notify und Magazin-Dampf-Emitter werden dokumentiert, aber nicht ausgeführt.
- Die digitale Anzeige auf dem Waffenmodell verwendet noch ihre Ausgangs-UVs. Das HUD zeigt den verbindlichen Magazin-/Reservestand. Weitere Gegner-, Treffer- und Materialgeräusche fehlen, soweit sie nicht über die drei übernommenen Soundnotifies erreichbar sind.

**Nächster konkreter Arbeitsschritt:** GeonosianWarrior-Bot/Attack-State, tatsächlich ausgerüstete Waffen bzw. Nahkampffunktion und `Pawn.TakeDamage` mit Bytecode/nativem Pfad abgleichen. Danach Laserbolzen-Nativrechnung, Schussursprung und Trefferregeln ersetzen und die normale Android-Gefechtsprobe wiederholen. Erst anschließend M3 abnehmen und mit der zusammenhängenden Mission aus M4 beginnen.

## Nachweise

Alle neuen Nachweise liegen getrennt unter `analysis/milestone3`; ältere Meilensteinbelege bleiben erhalten. Der abschließende `validation-manifest.json` bindet Quellen, Originalpakete und gelieferte Artefakte über SHA256.

- Gezielte Tests: Projektilflug vor Treffer, durchgehende Wandverdeckung, Feuerrate, Munitionserhaltung, automatisches Nachladen nach Loslassen, Notify-Eintritt/Loop/Ende und beschädigte WAV-Daten.
- `check/combat-validation.json`: identischer Spielzustand bei 120/30/10 Hostaufrufen/s, drei Gegner/15 Treffer, pro Gegner zwei Körperfallereignisse, genau ein Nachladegeräusch, Sieg/Niederlage, Reset, fehlerhafte Eingabe und unverändernder Renderpfad. Der vollständige Replay-Fingerprint enthält zusätzlich den Gleitkomma-Zeitrest des Hosts; dieser ist nicht Teil der Cadence-Gleichheitsaussage.
- `audio-abi-validation.json` und `abi-validation.json`: echte DLL, Kopiergrenzen, Verbrauch genau einmal, Handles, Pause, Reset und Zustandsbeobachtung.
- `legacy-replay.json`: 1.002 alte Kommandos und historische Referenzbilder unverändert; `m2-regression/animation-validation.json`: neun Originalclips, Instanztrennung, Todesende, Kamera, Pause/Reset und beschädigte Zuordnungen.
- Android-Abnahmen und abschließende Buildprüfungen werden im Manifest mit ihren tatsächlichen Ergebnissen aufgeführt. Ein Emulator ersetzt die physische ARM64-Prüfung nicht.

Aktueller Prüfbefund: **942 Workspace-Tests**, Clippy und Format bestehen. ARM64- und x86_64-Build, APK-Signatur, ZIP- und vier ELF-Ausrichtungen mit 16 KiB sind geprüft. Der vollständige normale Android-Gefechtslauf besteht über GPU und CPU. CPU/GPU-Startbild: mittlerer Kanalfehler 0,560/255; 99,151 % der Kanäle innerhalb zwei Stufen. Ein tatsächlicher Kontextneubau stellt Bild und Pose exakt wieder her. Drei Wiederladungen bleiben in der kurzen Speicherprobe begrenzt (letzte native Heap-Proben etwa 64,5 MiB); daraus folgt keine Langzeit-Leakfreiheit.

Die Grafikprobe wurde korrigiert, weil ein durch die Pause neu veröffentlichtes Bild bei Tick 165 zunächst gegen den CPU-Start bei Tick 0 verglichen wurde. Sie wartet jetzt auf eine neue Aufnahme bei Tick 0 und liest diese vor der Pause; die vorher festgelegten Bildtoleranzen wurden beibehalten. Die anfängliche Diagnose bleibt unter `graphics-timing-diagnostic/` erhalten.

Die Mehrfingerprobe mit Pointer-IDs 2/7/19/23 besteht ebenfalls: gleichzeitige Bewegung/Blick/Feuer/Sprung, letzte Blickkoordinaten beim Loslassen, CANCEL, Fokusverlust/Wiederaufnahme und Dateidialog-Abbruch. Quellenarchiv `source-snapshot.zip` und fest abgelegte APK `republic-commando-0.11.0.apk` stehen neben der Szenendatei; diese APK ist identisch zum abschließend geprüften Build.


## M3-Fortsetzung: native Angriffsentscheidung

Der zweite Rekonstruktionsabschnitt löst ForcePike-Ausrüstung, Karteninstanzunterschiede und vier Originalclips mit MeleeHit-Zeiten auf. Die vorbereitete native Manöverauswahl ist als Rust-Baustein portiert; 9.408 Originalinstruktions-Replays und 898 Pakettests bestehen. Die spielbare Szene verwendet weiterhin den bisherigen Gegneradapter. Goal-Ausführung, Angriffsclips/Notifies und endgültige Schadensregeln sind vor einer neuen Android-Lieferung zusammen zu integrieren. Details und konkrete nächste Schritte: [GEONOSIAN_ATTACK.md](GEONOSIAN_ATTACK.md).


M3-Fortsetzung 2026-10-10: Die native DamageVariance-Argumentbelegung ist anhand der Instruktionen geklärt. Varianz und vorbereitete Pawn-Schadensrechnung sind portiert; 4.038 Originalfälle und 958 Workspace-Bibliothekstests bestehen. Originaldaten zeigen 100 CloneCommando-Schilde, 62,5 ForcePike-Schaden vor weiteren Spielregeln und unterschiedliche DC-17-Schadenstypfaktoren. Noch keine Anbindung an die spielbare Szene oder neue APK. Nächste Integration: f32-Gesundheit/Schilde, Varianz/GameRules-Grenze und Melee-Goal/Clip/Notify zusammenführen. [Schadenspfad und Nachweisgrenzen](DAMAGE_PATH.md).


## M3d: spielbare Nahkampfintegration

Die neue Szene und APK 0.12.0 verbinden Original-Angriffsclips/Hit-Notifies mit fractional Gesundheit, Schildregeneration und ausgewählten DamageVariance-Listen. Bisherige vorbereitete Bausteine sind teilweise integriert; Navigation, native Goal-Ausführung, Lunge-Bewegung und GameRules bleiben offen. Aktuelle Nachweise, Lieferdateien und Abgrenzungen: [MELEE_SCENE.md](MELEE_SCENE.md).


## M3e, 2026-10-11: Projektil- und Schussursprung-Integration

Android 0.13.0 und geo_01a-ballistics.rcplay verbinden vorbereitete native Vektorbewegung, originalen FPFireOffset, direkte Zielpunktkorrektur und die Näheprüfung vor dem Spawn mit der Nahkampfszene. Mathematik-/Kollisionsgrenzen und verbleibender AimError sind ausdrücklich dokumentiert. Aktueller Stand und nächste Schritte: [BALLISTICS_SCENE.md](BALLISTICS_SCENE.md).
