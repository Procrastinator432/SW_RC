# Konkrete Hosts für geladene Animationen

Stand: 2026-10-08. Dieser Abschnitt erledigt drei zusammenhängende Aufgaben: geladene Sequenzen an Root-/Kanal-Sampling anbinden, die vorhandene Linkup-Aktualisierung als konkreten gemeinsamen Host einsetzen und beide Pfade über den Frame-Einstieg an Originaldaten prüfen.

## 1. Sequenzidentitäten und Originaltracks

`RuntimeAnimationBindings` enthält einen Katalog auf bereits gelesene `StoredSequence`-Objekte, Namens-/Sequenzbindungen und einen gemeinsamen Linkup-Zustand. Die Katalogidentitäten und Linkupschlüssel sind opake, vom Aufrufer aufgelöste Werte. Sie werden nicht als originale Speicheradressen oder Package-Indizes interpretiert.

`RuntimePoseHost` implementiert sowohl `RootPoseHost` als auch `PreparedChannelStackHost`. Beide verwenden die rekonstruierte GetSequence-Logik: außerhalb des Editors den Kanalcache, im Editor Namenssuche und Rückschreiben des Ergebnisses. Eine Nullsequenz bleibt ein regulärer fehlender Treffer. Eine nicht auflösbare, von Null verschiedene Identität liefert einen kontrollierten Fehler, bevor Tracks gelesen werden.

Der Root-Host bestimmt die erste passende Linkup-Tabelle über den Sequenzschlüssel, liest daraus den Roottrack und ruft GetRotPos auf den geladenen komprimierten Originaltracks auf. Quaternion-Schreibzugriffe vor einem Positionsfehler bleiben erhalten. Die Frame-Multiplikation verwendet die ausdrücklich portable f64-/f32-Näherung des bisherigen Root-Diagnosehosts.

Der Kanal-Host löst dieselben Sequenz- und Mappingdaten auf und verwendet die bereits geprüfte ApplyAnimChannel-/TrackChannelHost-Implementierung. Nullsequenzen oder Sequenzen ohne Tracks behalten das ursprüngliche Verhalten: keine angewandte Pose, aber Aktualisierung der Kanal-History. Quaternion- und Winkelmathematik bleiben über Policies konfigurierbar.

## 2. Tatsächliche Linkup-Aktualisierung

`RuntimePreparationHost` ruft die rekonstruierte `refresh_linkup`-Funktion auf. Sein Mappingzustand wird mit den Pose-Hosts geteilt, sodass diese den gerade aktualisierten Inhalt verwenden. Der Host protokolliert hier nicht mehr ausschließlich Aufrufe; er berechnet die Zuordnung aus Mesh- und Animations-Referenzknochenidentitäten.

Die Originalregeln bleiben bestehen: Gleiche Mappinglänge wird unverändert wiederverwendet, selbst wenn Werte veraltet sind oder die Animation fehlt. Bei abweichender Länge und vorhandenen Animationsnamen gewinnt der erste exakte Namenstreffer; fehlende Knochen erhalten −1. Ohne Animationsnamen bleibt das Mapping unverändert. Bei der anschließenden Sequenz-Linkup-Suche gewinnt der erste passende Schlüssel. Fehlende Linkups und leere Root-Mappings liefern kontrollierte Fehler statt ungültiger Speicherzugriffe.

Die gemeinsame Struktur verwendet `Rc<RefCell<...>>` für getrennt aufrufbare Vorbereitungs-, Root- und Kanal-Hosts in einem Thread. Sie ist keine threadsichere globale Engine-Registry. Der Aufrufer bestimmt Lebensdauer, Zuordnung und Thread der Animationen. MeshToWorld bleibt ein ausdrücklich gelieferter Szenentransform; die virtuelle Actor-/Mesh-Transformberechnung wurde hier nicht ergänzt.

## 3. Originaldaten und gemeinsamer Frame-Einstieg

`rc-runtime-hosts-check` lädt die Originalanimationen erneut. Pro diagnostischem Linkuplauf beginnt das Mapping leer. Mesh-/Animationsnamen werden über die vorhandenen diagnostischen, ASCII-kanonischen Identitäten verbunden. Zwei geladene Sequenzreferenzen werden mit ausdrücklich diagnostischen Identitäten versehen; die drei Kanäle wählen die erste, die zweite und erneut die erste Sequenz.

Jeder Lauf führt über `evaluate_animation_frame` vier Vollpose-Schritte aus. Nach jedem Schritt folgt ein Nicht-Editor-Cacheaufruf. Anschließend wertet derselbe gemeinsame Einstieg im Editor den Root aus und verwendet dabei die tatsächliche GetSequence-Aktualisierung. Der Vorbereitungs- und der Pose-Host sehen dieselbe Mappingtabelle.

`Record-RuntimeAnimationHosts.py` prüft unabhängig:

- **225 Mapping-Neuaufbauten** aus Original-Knochennamen und **1.800 Wiederverwendungen** über das Längen-Gate.
- **900 Vollposen / 29.912 Matrizen**, lokale Posen, Scratch, Kanal-History und Bounds gegen den zuvor anhand Originaltracks und ASM geprüften Bericht.
- **900 Cache-Wiederholungen** ohne zusätzliche Kanal-Auswertung.
- **225 Editor-Root-Auswertungen**, davon 224 Samples und ein Referenzroot, gegen den unabhängig geprüften Original-Root-Bericht.
- Sämtliche Hostereignisse einschließlich 2.025 Transformabfragen, Sequenzauflösung, Editor-Suche, Mappingauswahl und Root-Samplezeit.

Acht neue Tests ergänzen Null-/ungültige Sequenzidentitäten, Editor-Aktualisierung, geteilte Mappingänderungen, veraltete Längencaches, doppelte/fehlende Linkups und Quaternion-Teilzustände bei Positionsfehlern. **386 Workspace-Tests**, Clippy mit `-D warnings` und Formatprüfung bestanden.

## Grenzen und nächste Schritte

Die konkreten Hosts arbeiten auf geladenen Originaltracks und tatsächlich aktualisierten Mappings. Identitätsregistrierung, Katalogaufbau und Szenentransform bleiben explizit geliefert. Der Diagnoseaufbau enthält jeweils einen aufgelösten Animations-Linkup pro Lauf; er behauptet keine dynamische Asset-/Actor-Registry oder native Ladefolge. Die allgemeine Hoststruktur unterstützt mehrere Linkups, deren Schlüssel und Zuordnung vom Aufrufer geliefert werden.

Kanalzeiten, Invalidierung, Scratchinitialisierung, Padding und Actor-Skalierung bleiben diagnostische Vorgaben. Directors und Weltpublikation werden in dieser Probe nicht zusätzlich verwendet; ihre bestehenden Prüfungen bleiben separat gültig. Echtes Actor-/Script-Ticking, Skinning und Renderer-Verwendung stehen weiterhin aus. Eine sichtbare native Spielanimation folgt daraus noch nicht. Androidprüfung am Projektende.

Nachweise: `analysis/reports/runtime-animation-hosts.json`, `runtime-animation-hosts-validation.json`, `runtime-hosts-tests.log` und `analysis/evidence.json`. Verwandt: [Frame-Zweigauswahl](SKELETAL_FRAME_DISPATCH.md), [Instanzvorbereitung](SKELETAL_PREPARATION.md), [Root-Einstieg](SKELETAL_ROOT_ENTRY.md), [Vollpose-Einstieg](SKELETAL_ANIMATION_ENTRY.md).
