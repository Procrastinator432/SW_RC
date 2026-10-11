# M3j: Defensive Sprungplanung

Fortsetzung: **M3l / Android 0.18.0** integriert diese vorbereiteten Komponenten in die spielbare Szene. [Aktuelle Lieferung und Ausführungsgrenzen](DEFENSIVE_JUMP_SCENE.md). Der nachfolgende Stand dokumentiert die ursprüngliche Analyse.

Stand 11. Oktober 2026: Die originale defensive Knotensuche und Goal-Planung sind als vorbereitete Rust-Komponente rekonstruiert und geprüft. Die spielbare Lieferung bleibt **Android 0.17.0 / M3i** mit den drei Szenen unter `analysis/milestone3i`. M3j erzeugt Analyse-, Prüf- und Quellenartefakte unter `analysis/milestone3j`; eine neue APK oder geänderte Szene gehört nicht zu diesem Abschnitt.

## Ergebnis

`rc-package::defensive_jump` übernimmt die Entscheidungen von `ACTBot::FindNearestNavNode` (`10012e00`) und `UStateGeonosianWarriorAttack::DoDefensiveJump` (`100399a0`). Die Abfragen verwenden vorbereitete Host-Antworten für Verfügbarkeit, Klasse, Zone, Anchor, Erreichbarkeit und freie Bewegung. Der Aufrufer liefert die originale verkettete Reihenfolge und eindeutige, positive Knotenkennungen; Null bezeichnet einen fehlenden Actor.

- Deaktivierte Knoten werden übersprungen. Die Verfügbarkeitsabfrage erfolgt vor Klassen-, Zonen- und Distanzfilterung. Ohne erforderlichen Pawn-Anchor endet die Suche sofort.
- Radius und aktuelle beste Distanz werden strikt verglichen. Der native anfängliche Bestwert ist `1e20` (`1004dd98`, Bits `60ad78ec`). Gleich weit entfernte Knoten behalten innerhalb einer Suche ihre Listenreihenfolge.
- Die Suchrechnung addiert `(y²+x²)+z²`. Der anschließende Vergleich zwischen Perch und JumpDest verwendet `(z²+y²)+x²`; auch diese Float-Reihenfolge ist übernommen. Bei gleicher oder ungeordneter Distanz gewinnt JumpDest (`COMISS/JBE`).
- `MustDoDefensiveJump` wird vor beiden Suchaufrufen gelöscht. Die Suchen erfolgen in der Reihenfolge Perch, JumpDest mit Radius 10000 und Flags `clear=false`, `reachable=true`, `same_zone=true`.
- Nach Auswahl wird das Ziel reserviert und ChangeAimTurn geplant. Freie Bewegung führt zu JumpTo, andernfalls zu Goto mit Toleranz null. Für JumpDest folgen Blick-/Dreh-Goals zum Gegner, danach Sleep 0,5 s. Ist der Gegner vom Ziel erreichbar, folgt Follow mit Abstand 500 und Modus 1; andernfalls wird gegebenenfalls zum eigenen Anchor zurückgeplant. Die drei nativen Anchor-Abfragen dieses Rückwegs bleiben erhalten.
- Ohne Ziel bleibt `IsMeleeAttacking` erhalten; mit Ziel werden dieses Flag und das Strafe-Flag am Ende gelöscht. Die Goal-Liste setzt erfolgreiche Objektallokationen voraus.

`geonosian_attack::request_defensive_jump` übernimmt außerdem den Originalzweig nach dem Einreihen eines Nahkampf-Goals (`1003a2f1..1003a328`). Beide Zieltypen verbrauchen genau eine Zufallszahl; die Prüfung verwendet `probability >= fraction(rand)`. Damit kann auch ein Nullwert bei Rand-Ergebnis null erfolgreich sein. Die installierten Defaults betragen ungefähr 0,2 gegen andere Ziele und 0,8 gegen einen PlayerController.

## Originalkarte und Nachweise

`rc-defensive-assets` prüft die originale `LevelInfo.NavigationPointList`: 361 Knoten, keine Zyklen und gültige NavigationPoint-Unterklassen. Alle 15 JumpDest-Actors und `WarriorPerch0` liegen in dieser Liste. Ihre aufgelösten Locations, Aktivierungsflags, Referenzzähler und Herkunftsnachweise stehen in `defensive-receipt.json`. Die Liste wird vollständig aufgezeichnet; daraus wird keine reale Erreichbarkeit behauptet.

`Record-DefensiveJump.py` interpretiert die exportierten originalen x86-Instruktionen mit vorbereiteten Host-Aufrufen. `rc-defensive-check` vergleicht deren Ergebnisse mit Rust: **1.798 Knotensuchen, 392 Goal-Pläne und 224 Zufallszweige**, insgesamt 2.414 Fälle. Verglichen werden Ziel, Abfragefolge, Goal-Reihenfolge und Argumente sowie Zustandsflags und Zufallsverbrauch. Die Fixtures enthalten fehlende Anchors/Ziele, alle Filterkombinationen, strikte Radiusgrenzen, Gleichstände und Float-Summen an der 2²⁴-Grenze. Rust-Tests prüfen zusätzlich NaN und die native Bestwertgrenze.

**980 Bibliothekstests**, Clippy, Format und Whitespace-Prüfung bestehen. Alle drei M3i-Szenen bestehen erneut Ersttreffer, Niederlage, neun Treffer für drei Abschüsse, Neustart, Bildratenvergleich, Neutralzonen und Fehler-Rollback. Ihre Niederlagen bleiben bei Tick 360 / 159 / 38. Der alte Replay mit 1.002 Kommandos ist exakt. Quellenarchiv und Hashmanifest binden die Ergebnisse an Originaldaten und behalten die vorherige Lieferung bei.

## Laufzeitintegration als nächster Schritt

Die neue Planung wird gemeinsam mit der Flug-/Landekette an die Szene angeschlossen. Bis dahin werden die neuen Sprungwahrscheinlichkeiten und Zufallsziehungen im Spiel noch nicht verwendet; es gibt keine sichtbaren defensiven Sprünge dieser Komponente. Die bisherigen Angriffspfade bleiben nutzbar. Eine erneute Android-Prüfung folgt nach dieser Integration; die Emulatornachweise der unveränderten 0.17.0-Lieferung bleiben deren eigener Nachweis.

Die weitere Arbeit ist konkret eingegrenzt:

1. `AActor::CalcTrajectoryVelocity` (`engine.dll 103bbfa0`) und `GetAnimRootTranslation` (`103ba690`) samt Gravity-/Geschwindigkeitsparametern rekonstruieren.
2. `UGOAL_JumpTo::OnActivate` (`100320a0`) und `PreUpdate` (`10032840`) mit Startanimation, `GOAL_ChangePhysics` und Landebedingungen verbinden. Der Wert `0x15` im ChangePhysics-Konstruktor ist ein **ChangeFlags**-Maskenargument, keine Physics-Enum 21.
3. Einen begrenzten ausführbaren Goal-Host mit Reservierung/Freigabe, Kollisionsprüfung, Tod-/Trefferabbruch und Rückkehr zum Kampf integrieren. Danach die originale Zufallsziehung nach Melee aktivieren und die drei Szenen einschließlich Android erneut prüfen.

Allgemeine Script-GoalQueue, dynamische Verfügbarkeit/Zonen-/Anchor-Abfragen, native Flug-/Landesimulation und physische ARM64-Dauerabnahme bleiben offen.


Fortsetzung M3k: Die vorbereitete native Flugbahnberechnung ist jetzt umgesetzt und gegen 4.240 Instruktionsfälle geprüft. Start-/Landekette und Szenenanbindung bleiben offen. [Befunde und nächster Schritt](JUMP_TRAJECTORY.md).
