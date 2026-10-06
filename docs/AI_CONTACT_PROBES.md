# KI-Kontaktproben auf Originalkarten

`rc-mesh-probe <GameData> <map.ctm> <report.json> --ai-contact-diagnostic` führt die bisherigen Script-Bewegungsabläufe aus und startet an deren Endpunkten einen zusätzlichen statischen Kontaktablauf. Es werden Original-Spielermaße und aufgelöste Speed-/Crouch-Ratios verwendet. Die Probe simuliert eine nicht menschlich gesteuerte Figur mit diesen Maßen; sie rekonstruiert keine konkrete KI-Klasse.

Die Zielsuche prüft acht horizontale Yaw-Richtungen in8192-Schritten über jeweils600Einheiten mit dem Steh-Shape. Der nächste geometrisch gefundene Kontakt mit abs(NormalZ)<minimum_up und entgegen der Bewegungsrichtung wird als Diagnoseziel gewählt. Alle Suchqueries müssen vollständig sein. Die Suche ist keine Navigation, kein Pathfinding und kein Nachweis eines begehbaren Wegs. Liegt kein Ziel vor, wird NoTarget ausdrücklich gemeldet.

Die Probe beschleunigt mit Forward1 maximal120Ticks zu1/60Sekunde. Ab der ersten tatsächlichen Kontaktweiterleitung oder nach Ablauf des Vorwärtsbudgets folgen90Idle-Ticks. Yaw und Destination bleiben konstant. Der eigene AI-Kontaktadapter übernimmt Physics, ersten Wandkontakt, Duckprüfung und gegebenenfalls Recovery. Ein Treffer in der Zielsuche allein zählt nicht als Kontaktweiterleitung; ausbleibende tatsächliche Kontakte werden als NoContact protokolliert. ContactStanding und ContactStillCrouched beschreiben den beobachteten Duck-Endzustand und behaupten keinen vollständig erfolgreichen KI-Spielablauf. Queryfehler erscheinen im Report und führen zu einem Fehlerexit.

ControllerPresent=true, HumanControlled=false, DirectHitWall=false und MinHitWall0 sind ausdrücklich vorgegebene Diagnosewerte. Für World.BSP und alle gelieferten statischen Actors werden ExcludedWallClass=false und NotifyHandled=false als synthetische Snapshots geliefert. Diese Werte sind keine aus dem Originalspiel beobachteten Klassen-/Callbackantworten. CanCrouch und die Bewegungsratios stammen dagegen aus den aufgelösten Originaldefaults. Weapon1, gesund, kein Sprung und keine dynamischen Bases sind Diagnosevorgaben.

Berichte enthalten acht Suchqueries, Zielauswahl, jeden Physics-/Kontakt-/Recovery-Frame, Outcome und endgültigen Körper-/Timerzustand. `scripts/Validate-AiContact.py` vergleicht sämtliche bisherigen Berichtsteile gegen die Script-Motion-Berichte, zählt tatsächlich ausgeführte Kontaktweiterleitungen und Größenwechsel und prüft vollständige Volumes, inaktive Sprungevents sowie die Erhaltung der kollisionsbegrenzten Endgeschwindigkeit. Neue Reports: analysis/collision/*-ai-contact.json; Zusammenfassung: ai-contact-validation.json.

147 Workspace-Tests, Clippy und Formatprüfung bestehen. Die Diagnose aktiviert den zusätzlichen Pfad in der CLI; der bisherige Spielerpfad bleibt getrennt. Native innerhalb-des-Schritts-Reihenfolge, Controllerereignisse, dynamische Actorfilter, KI-Navigation und Android-Anbindung fehlen weiterhin. Android-Prüfung zum Schluss.

## Gemessene Ergebnisse

238 neue Diagnose-Ticks, 2 tatsächliche Kontaktweiterleitungen;1906Script- und2340Legacy-Ticks samt allen bisherigen Berichtsfeldern unverändert.

| Karte | Start | Ticks | Outcome | Weiterleitungen | Größenwechsel |
|---|---|---:|---|---|---:|
| geo_01a | PlayerStart0 | 0 | NoTarget | keine | 0 |
| dm_hangar | PlayerStart8 | 121 | ContactStanding | Unresolved:1 | 0 |
| dm_hangar | PlayerStart44 | 117 | ContactStanding | Unresolved:1 | 0 |

NoTarget/NoContact liefern keine Abdeckung eines Wandkontakt-Ereignisses. Unresolved belegt eine ausgeführte, weiterhin blockierte Duckprüfung; erst ArmedFirst/ArmedSecond belegt Timeraktivierung auf dieser Geometrie. Synthetische Tests bleiben der Nachweis für Fälle, die in diesen Originalkartenproben nicht auftreten.

## Folgearbeit: erweiterte Zielsuche

Ein zusätzlicher Modus sucht in32Richtungen über1500Einheiten und bewertet den Duckweg separat am geschätzten ersten Kontaktpunkt. Vorhersagen werden getrennt von tatsächlich ausgeführten Kontaktentscheidungen gezählt. 424 neue Diagnose-Ticks, 3 tatsächliche Kontaktweiterleitungen, 0 tatsächliche Timeraktivierungen und 0 Armed-Suchvorhersagen.1906Script- und2340Legacy-Ticks samt sämtlichen bisherigen Berichtsfeldern unverändert. Siehe [AI_DUCK_TARGETS.md](AI_DUCK_TARGETS.md).

## Folgearbeit: native Wandklassen

Der bisher unspezifische native Klassenvergleich ist als APawn/Unterklassen-Ausschluss identifiziert. Die CLI-Diagnose leitet ihn jetzt aus den tatsächlichen statischen Actor-Klassen und der Originalhierarchie ab; Notifyfalse bleibt Diagnosevorgabe und World.BSP eine explizite statische Weltpolicy. Controller.IsAPlayerController-Vtableslots der nativen Basisklassen direkt ausPE bestätigt, abgeleitete Runtime-Overrides weiterhin Snapshot. 742 statische Actor-Klassenbindungen geprüft;424KI-Diagnose-Ticks und sämtliche bisherigen Reportwerte unverändert, ausgenommen zusätzliche Klassenmetadaten und Scopebeschreibung.148Tests bestehen. Siehe [HIT_WALL_METADATA.md](HIT_WALL_METADATA.md).
