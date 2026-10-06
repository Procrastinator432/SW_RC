"""Persist the verified crouch-motion milestone in evidence and project notes."""
from pathlib import Path
import json

root = Path(__file__).resolve().parents[1]
validation = json.loads((root / "analysis/collision/crouch-motion-validation.json").read_text(encoding="utf-8-sig"))
evidence_path = root / "analysis/evidence.json"
evidence = json.loads(evidence_path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 124
evidence["crouch_motion_validation"] = validation
evidence_path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

results = """
## Originalkarten-Ergebnisse

Die reproduzierbare Validierung besteht für 210 neue Ticks: geo_01a/PlayerStart0 74, dm_hangar/PlayerStart8 74 und dm_hangar/PlayerStart44 62. Jeder Ablauf führt zwei Größenwechsel aus und endet stehend, am Boden, mit Velocity0 und gelöschten Scriptflags. Die letzten30 Ruhe-Ticks zeigen jeweils exakt keinen Höhendrift. Keine Falling-Ticks oder blockierten Aufstehversuche in diesen drei Originalkartenabläufen; diese Fälle bleiben durch synthetische Integrationstests abgedeckt.

Die beiden normalen Abläufe erreichen Ducktempo225 (450*0,5) und benötigen13 Brems-Ticks. PlayerStart44 bleibt in PhysicsVolume5 mit GravityZ-110, erreicht maximal145,0667 und hat zum Ende der30 Bewegungs-Ticks bereits Velocity0 bei einem Kontakt mit der schrägen Fläche von StaticMeshActor131. Der eigene statische Solver begrenzt dort die Bewegung; der einzelne folgende Brems-Tick belegt keine native Bremsparität oder freie Beschleunigung bis225.

Alle bisherigen Berichtsfelder sind gegenüber den Script-Motion-Berichten vollständig identisch, einschließlich1906 Script-Ticks und2340 Legacy-Controller-Ticks. Nachweis: `analysis/collision/crouch-motion-validation.json`. Insgesamt sechs integrierte Größenwechsel. Diese Messwerte ergänzen die isolierten60 Größenwechsel des früheren Crouch-Berichts.
"""
doc = root / "docs/CROUCH_MOTION.md"
if "## Originalkarten-Ergebnisse" not in doc.read_text(encoding="utf-8-sig"):
    with doc.open("a", encoding="utf-8") as output:
        output.write("\n" + results)

note = """2026-10-06: Duckbewegung mit Scriptphase und statischer PC-Bewegung verbunden. Native APawn.performPhysics104955f0 nach analysis/decompiled/pawn-physics-dispatch.c exportiert: Crouch vor Walking-Bewegung, UnCrouch bei Release nach Bewegung, bei anderer Physics vorher und gegebenenfalls erneut danach. Blockierter Aufstehversuch behält die Duckform und wird erneut versucht; zuvor abgelehnter Sprungevent bleibt verbraucht. Neuer VolumeWorld.crouching_script_tick mit explizitem CrouchingScriptBody, geprüftem Shape, gemeinsamem Bewegungsrechner und atomischer Zustandsrückgabe. bTryToUncrouch-Timer ausdrücklich abgewiesen. Vier neue Integrationstests für Ducktempo/Release-Reihenfolge, niedrige Decke/Retry/frischen Sprung, Aufrichten vor Falling und Fehler ohne Teilzustand.124 Workspace-Tests, Clippy und Formatprüfung bestanden. Originalkarten-Modus --crouch-motion-diagnostic:210 neue Ticks (geoStart0=74, HangarStart8=74, Start44=62), sechs Größenwechsel, alle Endzustände stehend/am Boden/Velocity0/Scriptflagsfalse, letzte30Ticks ohne Höhendrift. Normale Abläufe erreichen225; geringer-Schwerkraft-Ablauf erreicht145,0667 und wird an schrägem StaticMeshActor131 durch eigenen statischen Solver begrenzt. Keine freie Speedcap-/Bremsparitätsaussage für diesen Hangarfall.1906 Script- und2340 Legacy-Controller-Ticks samt allen alten Berichtsfeldern unverändert. Nachweise analysis/collision/*-crouch-motion.json/crouch-motion-validation.json, scripts/Validate-CrouchMotion.py und analysis/evidence.json. Native Cylinder/FarMove/SSE-Parität, Timer, dynamische Bases, Callbacks und vollständige Laufzeit bleiben offen. Android-Prüfung weiterhin zum Schluss; kein APK-Build/Emulatorlauf. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\CROUCH_MOTION.md."""
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "Duckbewegung mit Scriptphase und statischer PC-Bewegung verbunden" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print("Recorded verified crouch-motion evidence and wiki milestone.")
