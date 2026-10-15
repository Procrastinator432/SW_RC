# Geschützstellung: spielbarer Abschnitt 0.23.0

Lieferung: `analysis/mission4-complete/republic-commando-0.23.0.apk` und `analysis/mission4-complete/geo_01a-gunner.rcplay`. Beide zusammen verwenden. Die Szene enthält lokal aus der vorhandenen Installation gewonnene Originalassets; keine zusätzliche Asset-Auswahl nötig.

## Ausprobieren

APK installieren, „Spielabschnitt geo_01a öffnen“ und dann die neue Szenendatei auswählen. Bacta benutzen, die fünf Droiden an der Absturzstelle besiegen und die Granaten aufnehmen. Der Wegweiser führt zur Geschützstellung. Mit **GRANATE** (Tastatur **G**) werfen und Abstand gewinnen: Die Explosion kann auch den Spieler treffen. Ein erfolgreicher Projektilkontakt löst den Bunkerumbau aus; nach der Originalbestätigung erscheint „Abschnitt geschafft“. **Neustart** stellt den gesamten Abschnitt wieder her.

## Erreichte Grenze

Ein durchgehender Lauf der ausgewählten Originalziele **0, 1, 2 und 3** ist abschließbar. Das ist der Meilenstein bis zur gesicherten Geschützstellung, noch nicht die gesamte erste Originalmission.

- Export850/Prop11 ist als texturiertes Originalmodell `Arena_HR.ArenaWalls.Bunker1` sichtbar. Nach einer Sekunde wechselt es zu `BunkerBlown`: 1.356 Dreiecke und dessen 35 ursprüngliche Kollisionshüllen bleiben erhalten.
- Export1093/Prop4 zeigt `BunkerHatch`; nach dem Skriptereignis verschwinden Mesh und Kollision. Beide Props waren im früheren statischen Bildexport wegen ungelöster Subklassendefaults ausgelassen worden, obwohl ihre Kollision bereits existierte. Jetzt werden sie vollständig dynamisch dargestellt; es wurden keine fremden statischen Flächen entfernt.
- Trigger621 verlangt weiterhin tatsächlichen GrenadeProj-Kontakt. Der Treffer bestätigt Ziel3 sofort; sichtbare Zerstörung folgt nach60 Simulationsticks, CA1G1716 nach120, Abschnittsabschluss nach322. Kein Sieg beim bloßen Granaten-Pickup oder Treffer.
- Originale thermale Explosions- und Bunkergeräusche sind eingebunden, zusätzlich zu Waffen-, Aufnahme-, Abprall- und Wurfgeräuschen. Insgesamt20 Audiorollen.
- Explosionen verwenden Originaltexturen `EffectTex.Explosions.explosion_1a` und `EffectTex.DustAndDirt.DirtCloud_V3`. Größen, Lebenszeiten und Bildraster werden aus den Originalemittern gelesen. Sichtbarkeit wird durch Pixelvergleiche geprüft.
- Radialschaden berücksichtigt Opfergröße, Distanz, lebende Gegner, den Spieler und Sichtschutz durch die vorhandene Weltkollision. Dies ergänzt die weiterhin begrenzten Kampfregeln.

## Nachweise

`playthrough/validation.json`: durchgängiger Rust-Lauf, Sieg beiTick6045, fünfKills,20HP, exakte Verzögerungen, einmalige Audioereignisse, unverändertes statisches GPU-Paket und vollständiger Zustands-/Bildneustart. Die zerstörte Bunkerkollision und die entfernte Luke sind zusätzlich geprüft. `native/abi-validation.json` prüft denselben Abschnitt über die native Schnittstelle einschließlich ungültiger Handles, Eingabeflags und Puffergrenzen.

`android/native-replay.json`:7296 normale Eingabekommandos mit der **Android-x86_64-Bibliothek** unterAPI35, fünfKills,20HP und Sieg; CPU-Bilder, Originalaudioereignisse und Neustart geprüft. `android/validation.json`: APK über den normalen Android-Dateidialog geladen, GPU-Ausgabe, Touch-Schießen mit originalem Waffensample und Pause geprüft. Der komplette Android-Missionslauf nutzt die native ABI; ein vollständiger Touch-Durchlauf und die Abnahme dieser Version auf einem physischen ARM64-Gerät bleiben separat.

1036 Bibliothekstests, Clippy und Formatierung bestanden. Frühere Lieferungen werden unverändert aufbewahrt. `validation-manifest.json` bindet APK, Szene, Prüfergebnisse, Quellarchiv und Originaldateien an ihre Hashes.

## Weiterhin begrenzte Umsetzung

Nicht alle ursprünglich ausgelassenen Props und Umgebungsdetails sind wiederhergestellt; außerhalb des geprüften Wegs bestehen noch Lücken in der Szenenabdeckung. Die Explosion ist ein deterministischer Sprite-Adapter mit Alpha-Ausschnitt statt vollständiger additiver/transluzenter Originalpartikel. Thermale Effekte verwenden die originale Standard-Metallvariante; materialabhängige Effektwahl, Licht, Impuls und der vollständige ParticleEmitter-Interpreter fehlen. Kampf, KI, trockener Projektilflug und ruhende Karma-Wände behalten die bisher dokumentierten Adaptergrenzen. Die laufenden Original-Effektfabriken und `SpawnSmallBunker` werden als Skriptereignisse erfasst; der folgende Krieger-/Dialogabschnitt ist noch nicht spielbar.

## Nächster Meilenstein

Den nächsten Originalabschnitt ab `SpawnSmallBunker`/`Conversation04` anbinden: tatsächliche Fabrikinstanzen und Gegner, Anschlussweg und Kollisionsübergänge, zugehörige Dialoge und Zielbedingungen. Zuerst den Originalablauf und seine Assets vollständig zuordnen; danach einen zusammenhängenden Eingabelauf ab dem bestehenden Einstieg bis zum neuen Abschluss prüfen. Vor einer weiteren Lieferung Texturen, Audio und Android erneut prüfen.
