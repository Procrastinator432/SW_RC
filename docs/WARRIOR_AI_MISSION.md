# Warrior-KI in der Originalmission – 0.28.0

Die drei Warrior-Fabrikgegner des Artillerieabschnitts verwenden jetzt einen eigenen missionsgebundenen Controller mit Wahrnehmung, Wegsuche, originalen Angriffsentscheidungen, Nahkampf-Goals und Animations-Treffern. Die bisherige direkte Verfolgung mit sofortigem5-Schaden-Angriff bleibt nur für alte Szenen erhalten.

## Ausprobieren

APK: `analysis/mission9-ai/republic-commando-0.28.0.apk`.
Szene: `analysis/mission9-ai/geo_01a-warrior-ai.rcplay` (RCPLAY11,94.561.144 Bytes).
Die APK installieren und die neue Szene über den normalen Dateidialog öffnen. Der bisherige Ablauf mit Bacta, Absturzstelle, Granaten, Scorch, Sprengauftrag und Turmzugang bleibt erhalten.

Warriors können den Spieler erkennen, erreichbare Wege verfolgen und im Nahkampf angreifen. Ein Treffer entsteht erst am originalen MeleeHit-Notify. Rechtzeitiges Zurückweichen kann ihn vermeiden; eigene Treffer unterbrechen den Angriff und lösen die Trefferanimation aus. Der ursprüngliche ForcePike-Basiswert125 ersetzt den bisherigen vorläufigen Wert5. Der Missionshost hat weiterhin keinen Schildvorrat; nach dem bisherigen Kampf mit20HP ist ein ungeschützter Nahkampftreffer tödlich. Artilleriewarnung und Deckung beachten.

## Originalgebundene Regeln

Das Original `GeonosianWarriorBot.BurstCount=1` steuert das native zweisekündige Nahkampf-Goal. `rc_package::geonosian_attack` übernimmt die bereits geprüfte Entscheidungsreihenfolge. `rc_package::melee_goal` übernimmt Aktivierung, strikte Erreichbarkeitsfrist, Reichweiten-/Facing-Grenzen und das Warten auf das Animationsende.

MeleeAttack und MeleeAttack2 verwenden die originalen Treffer-Notifies115/134 bei0,3125 und0,4003226 der Sequenz. Mesh, Material, Skeleton und Clips stammen ausGeonosian.ukx. ForcePike-Reichweite350, Basis-Schaden125, Schadensklassen und Commando-Verteidigungsregeln stammen aus der zuvor abgenommenen Originaldaten-Szene von Meilenstein3l. Treffer laufen durch den vorhandenen Pawn.TakeDamage-/DamageVariance-Adapter, mit Normal-Faktor1 und dem bestehenden Missionsgesundheitsmodell.

78 Original-PathNodes und208 gerichtete PathList/ReachSpec-Verbindungen bestehen Körper- und Bodenprüfung mit50/50/115. Nur normale Geh-Verbindungen ohnepruned/jump/perch/fly sind zugelassen. Jede tatsächliche Bewegung prüft erneut Körperkollision und Bodenunterstützung. Originalgeschwindigkeit450, blockierte Wege, geschlossene Barrikaden und zu niedrige Durchgänge werden berücksichtigt.

## Bewusste Hostregeln

Augen-Sichtstrahl mit maximal2600 Einheiten; nach Sichtverlust drei Sekunden Verfolgung der zuletzt gesehenen Position. Keine Aktualisierung dieser Position durch Wände. Dijkstra, Anschlussauswahl, Neuplanung nach1s beziehungsweise2s bei gesperrtem Weg und0,25s Trefferreaktion sind deterministische Portregeln. Die beiden stationären Angriffe wechseln sich ab. Augenoffsets80/60, sofortiges Drehen und die Bodenauswahl sind geometrische Adapter.

Ausfallschritte, offensive/defensive Sprünge und Granaten melden in diesem missionsgebundenen Controller noch keine Verfügbarkeit. Die dafür vorhandenen separaten Testsysteme sind damit noch nicht an diese Mission angebunden. Allgemeine GoalQueue, Flug/Perch, Squad-Befehle, Begleiterkampf/-Wiederbelebung, ursprüngliche Wahrnehmungsparameter und vollständige CTBot-Parität bleiben offen. Diese Lieferung vervollständigt den grundlegenden Warrior-Geh-/Nahkampfablauf, nicht die gesamte Spiel-KI.

## Prüfbelege

Der zusammenhängende Missionslauf erreicht die Originalziele0–6 nach16.640 normalen Eingaben mit20HP/acht Abschüssen und exaktem Neustart. Die ältere RCPLAY10-Lieferung bleibt bei16.648 Eingaben unverändert erfolgreich. Beschuss-Niederlage bleibt separat geprüft.

Zusätzliche normale Eingabeläufe belegen tatsächliche KI-Verfolgung ohne Gegenwehr, Angriffsbeginn inTick14.728 und den einzelnen Treffer/Niederlage erst inTick14.758; Zurückweichen vermeidet diesen Treffer, ein Schuss unterbricht ihn vorher. Drei eigenständige Eingabedateien werden in Rust sowie Windows-/Android-ABI geprüft. Der geometrische Umwegtest ist ausdrücklich separat: PathNode141→PathNode282 ist direkt blockiert und über fünf Wegpunkte erreichbar; alle Abschnitte bestehen die Originalweltprüfung. Er setzt keine Missionszustände für den normalen Durchlauf.

1.067 Bibliothekstests, Clippy, Format/Diff, Android-BuildARM64/x86_64, APK-Dateidialog/GPU/Audio/Touch und Szenenlesen mit112MiB-Java-Heap sind im Liefermanifest gebunden. Vollständiger manueller Touch-Durchlauf und physische ARM64-Abnahme dieser Version bleiben offen.

## Fortsetzung

Als Nächstes die vorhandenen Ausfallschritt-/JumpDest-Komponenten mit korrekter Originalgröße in diesen Controller einbinden. Danach allgemeine Squad-Zielwahl, Waffenattachment, Begleiterkampf und Wiederbelebung rekonstruieren. Die fehlende Einleitung und das Laden vonGEO_01B bleiben weitere Voraussetzungen für die vollständige erste Mission.
