# GEO_01B: zusammenhängender Missionslauf / 0.35.0

Lieferung: `analysis/mission18-map2-route/delivery/republic-commando-0.35.0.apk` und die dazugehörige `geo_01b.rcplay` (RCPLAY14). GEO_01A verwendet weiterhin die unveränderte Szene aus `analysis/mission15-complete/delivery`. Für den Einstieg in GEO_01B den tatsächlichen Kartenwechsel nach dem Abschluss von GEO_01A verwenden; der gespeicherte Übergang erhält Lebenspunkte und Munition.

## Steuerung

Bewegen, zielen und schießen wie bisher. An Sprengmarkern **BENUTZEN drei Sekunden halten**, mindestens 300 Spieleinheiten zurücktreten und mit **FEUER** zünden. An den Slice-Konsolen **BENUTZEN sechs Sekunden halten**; Fixer muss beigetreten sein. Bacta-Stationen heilen beim Halten von BENUTZEN. Blasterboxen liefern Reservemunition. Beim direkten Szenenstart ohne gespeicherten Übergang verwendet der Port seinen regulären Startzustand.

## Erreichter Abschnitt

Der automatisierte Lauf durch GEO_01B verwendet ausschließlich normale, aufgezeichnete Controller-Eingaben. Der Einstieg übernimmt den echten GEO_01A-Abschluss mit 20 Lebenspunkten, 13 Schuss und 180 Reserveschuss. Im laufenden Spiel werden keine Positionen, Lebenspunkte, Gegner oder Ereignisse gesetzt. Zwei originale Bacta-Stationen und regulär aufgesammelte Munitionskisten versorgen den Spieler.

Nach 16.414 Eingaben sind alle acht Ziele und fünf Spreng-/Slice-Aktionen abgeschlossen. Vier Schlusskorridor-Droiden lösen ihren Originalzähler aus; auch `DialogLocatedSunFac` und `DialogRappelToSunFac` werden regulär ausgelöst; der Ausgang fordert `GEO_01C.ctm#playerstart` an. Ergebnis: 10 Lebenspunkte, 20 Schuss, 141 Reserveschuss, 112 Schüsse/Treffer, 35 Abschüsse und sieben Nachladevorgänge. 39 der 42 Fabrikdeklarationen werden natürlich aktiviert und erzeugen 42 Gegner; sieben optionale Gegner bleiben am Ausgang am Leben. Drei Fabrikdeklarationen haben im ausgewerteten Ereignispfad keine reguläre Auslösung und werden nicht künstlich gestartet.

Scorch und Fixer folgen mit physischen Körperschritten. 345 originale Navigationspunkte und 3.562 geometrisch körpergeprüfte gerichtete Verbindungen bilden den importierten Gehgraphen. Die Laufgeschwindigkeit von 450 stammt aus den übernommenen Squad-Assets. Kleine Bodenabsenkungen und Rampen akzeptieren reguläre Bodenkontakte; eine Abstandskontrolle verhindert das Zusammenlaufen der beiden Begleiter. GEO_01A behält seine bisherige Korridortoleranz.

Drei ursprünglich unsichtbare zerstörte Fighter-Hüllen blockierten den ersten Hangar dauerhaft. Der Port entfernt ihre Kollision nun nach dem zugehörigen Sprengereignis und einer begrenzten Aktivierungs-/Lebenszeit. Beim Neustart kehren die Hüllen und Ereigniszustände zurück. Die vollständige kurzlebige Karma-Physik und Darstellung dieser Wracks ist damit noch nicht rekonstruiert.

## Nachweise

- `replay/validation.json`: gesamter GEO_01B-Eingabestrom gegen den Referenzzustand, begrenzte physische Scorch-Schritte sowie identischer Neustartzustand, Grafikframe und Wrack-Kollision.
- `native/validation.json`: tatsächliche 18.730 GEO_01A-Eingaben, transaktionaler Kartenwechsel, danach alle 16.414 GEO_01B-Eingaben über die Windows-C-Schnittstelle; Status, Ausgangsposition, Audio und Neustart.
- `android/native-validation.json`: derselbe komplette A/B-Eingabestrom über die Android-x86_64-Bibliothek, inklusive Ausgangsanforderung und Neustart. Statuswerte werden exakt verglichen, die Position mit 0,5 Spieleinheiten Toleranz. Gemessene Abweichung: etwa 0,299 Einheiten in Y, 0,00115 in X und keine in Z nach dem vollständigen Lauf; der Plattformvergleich behauptet keine bitidentischen Fließkommazustände.
- `android/validation.json`: APK mit tatsächlich erzeugtem gespeichertem GEO_01A-Übergang, Dateiöffnung, GPU, Touch-Bewegung/Schießen/Ducken, Audio und Pause. Dies ist eine APK-Einstiegsprüfung; der vollständige Missionslauf erfolgt über die native Schnittstelle.
- `validation-manifest.json`: geprüfte Lieferdateien, SHA-256 und unveränderte vorherige Lieferungen. Die Szene wird zusätzlich mit bekanntem und unbekanntem Dateiumfang unter 96 MiB Java-Heap gelesen.

## Offene Arbeit

GEO_01C ist weiterhin nur eine Übergangsanforderung und wird noch nicht geladen. Nächster Meilenstein ist der Import von GEO_01C mit originalem Einstieg und anschließendem Missionsabschluss. Vor vollständiger Originalparität fehlen weiterhin Teile der KI-, Squad-, Animations-, Projektil- und Karma-Regeln, einzelne Material-/Effektdetails sowie ein vollständiger manueller Touch-Lauf und die Abnahme dieser Version auf einem physischen ARM64-Gerät. Der Gehgraph ersetzt keine originale ReachSpec-/FollowLinks-Rekonstruktion. Der automatisierte Lauf verwendet präzises Zielen und belegt noch keine menschliche Schwierigkeitsabstimmung.
