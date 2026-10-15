# GEO_01C: Superkampfdroiden-Auftritt und Originalziel3

Lieferung **0.40.0 / Code44**, `analysis/mission23-map3-guard/delivery`:
`republic-commando-0.40.0.apk` und passende `geo_01c.rcplay`.
APK installieren und die neue Szene über „Szene öffnen“ laden. Der gespeicherte
B→C-Übergang übernimmt die Ressourcen. Nach dem bisherigen Einstieg, Planungsraum
und Sun Facs Flucht zur Wachtür vorgehen. Der Auftritt startet nach dem ursprünglichen
Näheauslöser. Währenddessen nachladen. Einen sichtbaren Gegner anvisieren und
**BENUTZEN** drücken, um das Squad-Feuer auf ihn zu konzentrieren. Den Intro-SBD
ausschalten; anschließend weist das Ziel auf Sun Facs nächste Tür.

## Originalbindungen und Ausführung

Skript147 wartet auf `WindowDestroyed` und `StartSBDIntro`. Nach seiner originalen
Drei-Sekunden-Warteaktion folgen `SpawnGuardRoom`, `SBDIntro` und die Entriegelung
der GuardRoomDoor139. Weitere Warteaktionen, Originalaufnahmen D38G1509/CA1G1562,
Hinweisereignis und `GuardRoomDroids` werden ausgeführt. Aktion916 ist im Original
deaktiviert und wird ausdrücklich übersprungen. Timer und Aktionsübergänge laufen
im festen60Hz-Host; Ereignisweitergabe kann zusätzliche Ticks benötigen.

Die drei Originalfiguren321/322/323 spielen synchron `IntroSBDRightPush`
(90Frames/30FPS), `IntroSBD` (210/30) und `IntroSBDLeftSmash` (150/30).
Kanalstart, LOD-Animationstick, AnimEnd, relative Root-Matrizen und Notifies nutzen
die vorbereiteten nativen Rechnungen. Im Referenzlauf entstehen3/2/10Notifies.
Elf FireWeapon-Notifies lösen eine Original-Schussaufnahme aus. Effekt-, Ragdoll-
und Fußschrittnotifies werden zeitlich protokolliert; ihre vollständigen Seiteneffekte
sind noch nicht angebunden. Intro-Projektile verursachen hier keinen Spielerschaden.

Die animierten Introfiguren sind im begrenzten Host zunächst dekorativ und nicht
beschießbar. Ihre Root-Kurven bestimmen die Darstellung; eine vollständige native
PHYS_RootMotion-/Actor-Kollisions-/Karma-Ausführung wird nicht behauptet. Die beiden
kleinen Droiden verschwinden nach ihrem AnimEnd. Der SBD wird nach AnimEnd an seiner
Endposition unterstützt aufgestellt und als Kampfgegner übergeben. Es gibt keinen
zusätzlichen Abschuss für die dekorativen Droiden.

Der Intro-SBD besitzt die originalen400HP, Maße65/65/115 und den ursprünglichen
Gehclip WalkForward. Seine Zerstörung löst das Originalereignis `SBDDestroyed` aus.
Skript358 schließt daraufhin Originalziel3 ab, wartet zwei Sekunden, spielt
CA1G1727 und D38ZZ011 und setzt den Tracker auf `BreachDoorSunFac`.

Alle acht Fabriken des ursprünglichen `SpawnGuardRoom` sind gebunden:
196/197/228/229/230/231/232/233. Sie erzeugen einen weiteren SBD, sechs BattleDroids
und einen Warrior mit ihren Originallebenspunkten und Kollisionsmaßen. Die
Aufstellung sucht begrenzt um den Originalpunkt einen freien, unterstützten Platz;
Original- und Laufzeitpositionen stehen im Buildbeleg. Die Kampfaktivierung der
Fabrikgegner erfolgt im begrenzten Host bei `GuardRoomDroids`. Ursprüngliche
Patrouillen-/Startzustände und Factory-Scheduling sind nicht vollständig portiert.

Der vorhandene C-Kampfhost verwendet weiterhin begrenzte Sicht-, Bewegungs-,
Angriffs- und Schadensregeln. Ein anvisierter Gegner wird mit BENUTZEN innerhalb
2500Einheiten und bei freier Schusslinie als Squad-Ziel gewählt. Die bestehende
Unterstützungslogik priorisiert dieses Ziel, solange es lebt und sichtbar ist.
Nach dessen Tod wird die Markierung entfernt. Das ist keine native CTBot-/Squad-
Goal-/Rüstungs-/Ballistikparität. Insbesondere bleiben die30Schaden-Spieler-
Hitscan-Regel und die gemeinsame15Schaden-Unterstützungstaktung Portregeln.

Die Soundbank enthält23Originalsamples. Die zusätzliche Droidenaufnahme stammt
aus `Properties.BattleDroidBlasterDefaults.FireSound`:
`GEN_Sound.Weapons.wep_GEN_sbdGun_01`. Sie wird hier auch als gemeinsame SBD-
Schussaufnahme benutzt, einschließlich seines tatsächlichen Kampf-Schusses.
Die vollständige native SBD-Waffen-/Audiozuordnung und räumliche Audioparität
bleiben offen. Spieler- und Squad-Waffentöne bleiben erhalten.

## Abnahme

`playthrough/validation.json` spielt4.653normale C-Eingaben nach dem tatsächlichen
B-Abschluss: sämtliche bisherigen C-Abschnitte, Anmarsch zur Tür, Auftritt,
Nachladen, Squad-Zielwahl und SBD-Kampf bis zum nächsten Türtracker. Keine laufende
Position, Gesundheit, Munition oder Missionsereignisse werden im Abnahmelauf gesetzt.
Ergebnis: Ziele1/2/3,5HP,8/122Munition,31Spielerschüsse, sechs Abschüsse, zehn
Squad-Unterstützungstreffer und ein gegnerischer Angriff. Die acht zusätzlichen
Wachraumgegner sind am Ende noch am Leben; Ziel3 verlangt den Intro-SBD, nicht
deren vollständige Beseitigung. Insgesamt entstehen88Audioereignisse.

Geprüft werden das ausbleibende vorzeitige Intro, alle drei Animationsenden,
die15Originalnotifies, die blockierte Squad-Zielwahl durch die geschlossene Tür,
neun eindeutige zusätzliche Gegnerbindungen, der einmalige
SBD-Zerstörungsauslöser, Türentriegelung, Zielabschluss und Tracker. Die20Sekunden
nach dem gespeicherten C-Anmarschpräfix sind bei20/60/144Eingabeframes proSekunde
vollständig zustandsgleich. Neustart stellt Zustand und Grafikframe exakt wieder
her; der gespeicherte Eingabelauf reproduziert den vollständigen Endzustand.

Die Windows- und Android-Native-Prüfung spielt davor18.730A- und16.414B-Eingaben
durch und wechselt tatsächlich nachC. Die APK-Prüfung öffnet C mit der tatsächlich
unterAndroid erzeugten gespeicherten Übergabe und prüft Einstieg, GPU, Touch,
Waffenton und Pause. Dies ist kein vollständiger manueller Touch-Durchlauf und
kein physischer ARM64-Test. Beide Java-Szenenleser werden unter96MiBHeap geprüft.
Die Liefersicherung kontrolliert auch die Unverändertheit früherer Belege.

## Nächster Abschnitt

Die Gesamtmission bleibt unvollständig. Originalziel0 und Ziele4–6 fehlen weiter.
Als Nächstes den durchgehenden Weg durch den Wachraum bis `BreachDoorSunFac`
prüfen, die Original-Bactastationen310/261 anbinden und das Türbrechmanöver mit
Originalziel4 integrieren. Danach folgen Scharfschützenabschnitt, Sun Facs Ende
und Aufzug/ursprünglicher Ausgang nachGEO_03A. Die weiteren ursprünglichen
Factory-Patrouillen und die native Kampf-/Karma-/Effektparität bleiben eigene Arbeit.
