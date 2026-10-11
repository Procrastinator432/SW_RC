# Projektil- und Schussursprung-Integration M3e

Stand 2026-10-11. Lieferung: Android 0.13.0 (Code15) und `analysis/milestone3e/geo_01a-ballistics.rcplay`. Neue APK erforderlich. Das Paket erweitert die Nahkampfszene M3d; beide Vorgänger bleiben separat gespeichert.

## Ergebnis

Laserbolzen starten weiterhin mit Geschwindigkeit null. Die neue Integration speichert einen f32-Geschwindigkeitsvektor und führt die vorbereitete originale Beschleunigungs-, Begrenzungs- und Bewegungsrechnung aus. MaxSpeed begrenzt den Vektorbetrag; ohne Beschleunigung wird die Begrenzungsfunktion entsprechend dem Originalzweig nicht aufgerufen. MaxSpeed null stoppt im ausgeführten AProjectile-Begrenzungspfad entgegen dem allgemeinen Kommentar in Projectile.uc den beschleunigten Bolzen.

Originaler FPFireOffset der DC17m: (127,4;30;-10), bezogen auf die View-Achsen. Der Ursprung entsteht aus vorbereitetem Auge und ursprünglicher GetAxes-Rotatormatrix. Der direkte AdjustAim-Zweig verfolgt den Blickstrahl, korrigiert die Richtung vom versetzten Ursprung zum Ziel und benutzt die originale 0,4-Winkelschwelle gegen rückwärts/seitwärts austretende Bolzen. Die im Original vorhandene Subtraktion des seitlichen Offsets von der Welt-Y-Komponente des weit entfernten AimSpot bleibt erhalten.

Vor dem Spawn wird der Weg vom Auge zum Schussursprung auf Wände und vorbereitete Gegnerkörper geprüft. Liegt etwas dazwischen, entsteht kein Bolzen dahinter. Für diese nicht geworfene Munition mit Radius null wird ein unmittelbar getroffener Gegner über den Ammo-Schadenswert getroffen. Munition, Schussstatistik und Schussereignis werden dabei genau einmal verbraucht/ausgelöst. Der bestehende DamageVariance-/Schild-/Gesundheitspfad bleibt angeschlossen.

## Originalbelege und Abgrenzung

`projectile-native.c/.asm` exportieren engine.dll AActor::physProjectile 10492300, AActor::BoundProjectileVelocity 1048b5a0 und die verwendete AProjectile-Übersteuerung 1048b800. `rc-package::projectile_motion` übernimmt den vorbereiteten ersten, nicht abprallenden Schritt bis zum MoveActor-Aufruf. Weltbewegung/Touch/HitWall-Aufrufe bleiben an die vorbereitete Szenenkollision angebunden.

`Record-ProjectileMotion.py` spielt die originalen SSE-Instruktionen der Beschleunigungsspeicherungen, kompletten AProjectile-Begrenzung und Bewegungsdelta-Berechnung ab. 2.051 Fälle stimmen in allen f32-Ausgabebits mit Rust überein. Die RSQRTSS-Saat ist in beiden ausdrücklich durch f32(1/f32(sqrt(x))) ersetzt. Das belegt die umgebende Instruktionsrechnung unter dieser Mathematikgrenze, keine Hardware-RSQRT-Bitparität und keine Ausführung der vollständigen physProjectile-Funktion. FVector-Rotator-Trigonometrie verwendet portables atan2 vor der vorhandenen Original-Sinustabelle.

GetFireStart, der direkte AdjustAim-Zweig und CreateProximityProjectile stammen aus den extrahierten Weapon.uc, PlayerController.uc und Ammunition.uc. CTPlayer.uc enthält keine aktive AdjustAim-Übersteuerung (der betreffende Code ist auskommentiert). Diese Scriptpfade sind quellenbasiert portiert; keine vollständige VM-Parität wird behauptet. Die Kamera/Augenhöhe, statische Welt-/Box-Actor-Traces und Lebensdauer-Taktung bleiben vorbereitete Hostregeln. Rückprall, reflektierende Actors, Materialeffekte, Knochen-Touch-Trace, Netzwerk-/Rollenablauf, bewegliche Hindernisse und vollständige Spawn-Callbacks sind nicht enthalten.

PickTarget-/Bone-Zielhilfe und stochastischer AdjustAimError bleiben deaktiviert. Der Original-AimError beträgt 15 und wird als offener Integrationspunkt im Defaultsbeleg mitgeführt. Game.ReduceDamage bleibt wie in M3d eine explizite Identitätsfunktion. Diese Teilintegration macht M3 noch nicht vollständig originalgetreu.

## Prüfung und Wiederholung

966 Workspace-Bibliothekstests, Format und Clippy bestehen. Neue Verhaltenstests prüfen Ursprung vor einer nahen Wand, unmittelbaren Actor-Treffer ohne Bolzenspawn, Munitionsverbrauch und Treffbarkeit vom versetzten Ursprung. Der wirkliche Szenencheck prüft Projektile, Varianz, Nahkampf-Notifies, Schilde/Regeneration, Ausweichen, Tod, drei Abschüsse, Niederlage und Reset sowie gleiche Zustände bei verschiedenen Host-Bildraten.

Die Oracle-Datei lässt sich mit `scripts/Record-ProjectileMotion.py` erzeugen und über `rc-projectile-check` vergleichen. `rc-play-ballistics GAME_DATA MELEE_SCENE OUTPUT_DIRECTORY` baut die neue Szene aus M3d und lokal aufgelösten Waffendefaults. Nachweise und die fest abgelegte APK liegen unter `analysis/milestone3e`; `validation-manifest.json` bindet Quellen, Originaldateien und gelieferte Artefakte.

Zum Ausprobieren APK installieren und `geo_01a-ballistics.rcplay` über **SZENE ÖFFNEN** laden. Drei Gegner ausschalten und die Exfil-Markierung erreichen. Der persönliche ARM64-Gerätetest dieser Version steht aus.

## Nächster Schritt

Als nächster zusammenhängender M3-Abschnitt folgen der AI-Goal-Host und eine kollisionsgeprüfte Navigation für die Geonosianer. Danach Lunge-Bewegungsübernahme, GameRules/Schwierigkeitsgrad, AimError/Zielhilfe und weitere Kampfklänge. Die vollständige Mission M4 folgt nach der M3-Abnahme.

Abgeschlossene Android-Abnahme: beide vollständigen Gefechtsläufe über GPU/CPU bestehen, einschließlich Dateiöffnen/Fehlerbehandlung, Touch-Zielen/Feuern/Bewegen/Sprung/Nachladen, Sieg/Niederlage und Neustart. CPU/GPU-Startbild: mittlerer Kanalfehler 0,550/255, 99,139 % innerhalb zwei Stufen. Backendwechsel und echter Kontextneubau erhalten den pausierten Zustand; Bild und Pose nach Kontextneubau exakt. Drei Wiederladungen bestehen die kurze Speichergrenzenprüfung (letzter nativer Heap etwa 64,2 MiB); keine Langzeit-Leakfreiheits- oder physische Leistungsbehauptung. Signatur, ZIP-Ausrichtung und beide ABI-Builds sind geprüft. Historischer 1.002-Kommandos-Replay sowie M2-, M3- und M3d-Szenenprüfungen bestehen.


2026-10-11: M3f integriert Gegnernavigation und ausgewählten Goal-Host. Neue Lieferung Android 0.14.0/Code16 plus geo_01a-navigation.rcplay unter analysis/milestone3f. 37 originale, bodengeprüfte PathNodes und 68 gerichtete PathList/ReachSpec-Gehverbindungen; eigener begrenzter Dijkstra-Host, Neuplanung/Warten, vorbereitete native Manöverwahl und MeleeGoal-Reihenfolge. Originalkarten-Umweg PathNode3 -> PathNode191 mit Notify-Treffer, Zielwechsel, Tod, Warten, Reset, Bildratenvergleich und drei Abschüssen geprüft. Native FollowLinks/GoalQueue, Lunge/Sprung, Angriffsanzahlfeld und GameRules bleiben offen. Details: D:/Rust Projects/RepublicCommandoAndroid/docs/NAVIGATION_SCENE.md. Nächster Abschnitt: Lunge-Bewegung.
