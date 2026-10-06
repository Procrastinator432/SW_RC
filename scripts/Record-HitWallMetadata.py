"""Record verified actor-class filters, native controller evidence and map regression."""
from pathlib import Path
import json

root=Path(__file__).resolve().parents[1]
result=json.loads((root/"analysis/collision/ai-wall-metadata-validation.json").read_text())
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=148
evidence["hit_wall_metadata_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
summary=f"{result['total_static_objects']} statische Actor-Klassenbindungen geprüft;424KI-Diagnose-Ticks und sämtliche bisherigen Reportwerte unverändert, ausgenommen zusätzliche Klassenmetadaten und Scopebeschreibung."
path=root/"docs/HIT_WALL_METADATA.md"
text=path.read_text(encoding="utf-8-sig")
marker="\n## Originalkarten-Ergebnis\n"
if marker in text:
    text=text.split(marker)[0]
text+=marker+"\n"+summary+"\n\n| Karte | Statische Klassenbindungen | KI-Ticks | Weiterleitungen | Pawn-Ausschlüsse |\n|---|---:|---:|---:|---:|\n"
for m in result["maps"]:
    text+=f"| {m['map']} | {m['classified_static_objects']} | {m['probe_ticks']} | {m['dispatches']} | {m['excluded_pawn_objects']} |\n"
text+="\nDie hier gelieferten statischen Klassen lösen keine Pawn-Ausschlüsse aus; die native Pawn-/Unterklassenbedingung ist im neuen Unit-Test geprüft. Kontaktentscheidungen bleiben dieselben blockierten Fälle wie im vorherigen Zielbericht; kein zusätzlicher erfolgreicher Ducknachweis.\n"
path.write_text(text,encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig").replace("147 Rust-Unit-Tests","148 Rust-Unit-Tests")
if "Native Wandklassen-Metadaten" not in text:
    text+="\nNative Wandklassen-Metadaten eingebunden: Pawn-/Unterklassen-Ausschluss aus Originalhierarchie statt pauschaler false-Vorgabe. IsHumanControlled und Basis-Controller-Vtableslots direkt geprüft; Runtime-Overrides/Notifyantworten bleiben explizite Snapshots. "+summary+"148Tests/Clippy/Formatbestehen. [Details](docs/HIT_WALL_METADATA.md).\n"
path.write_text(text,encoding="utf-8")
for relative in ("docs/HIT_WALL.md","docs/AI_CONTACT_PROBES.md","docs/AI_DUCK_TARGETS.md","docs/AI_CONTACT.md"):
    path=root/relative
    text=path.read_text(encoding="utf-8-sig")
    if "## Folgearbeit: native Wandklassen" not in text:
        text+="\n## Folgearbeit: native Wandklassen\n\nDer bisher unspezifische native Klassenvergleich ist als APawn/Unterklassen-Ausschluss identifiziert. Die CLI-Diagnose leitet ihn jetzt aus den tatsächlichen statischen Actor-Klassen und der Originalhierarchie ab; Notifyfalse bleibt Diagnosevorgabe und World.BSP eine explizite statische Weltpolicy. Controller.IsAPlayerController-Vtableslots der nativen Basisklassen direkt ausPE bestätigt, abgeleitete Runtime-Overrides weiterhin Snapshot. "+summary+"148Tests bestehen. Siehe [HIT_WALL_METADATA.md](HIT_WALL_METADATA.md).\n"
    path.write_text(text,encoding="utf-8")
note="2026-10-06: Native Wandklassen-Metadaten eingebunden. processHitWall10490104CMP1075a270 entspricht exportPrivateStaticClassAPawn, Parent+30; WallEventSnapshot.from_actor_class prüftOriginalCatalogActor/PawnAncestry, UnknownErr, NotifySnapshoterhalten. CLIführtklassifizierteActor-KlassenbindungenundReportswall_metadata, keinpauschalesExcludedfalseaußerexplizitemWorld.BSPPolicy. APawn.IsHumanControlled10480780 prüftController41c/virtuellenSlot1d0; ControllerVtable10658650->1031a880return0, PlayerController10663620->103247d0return1, PEBytes/Exportsgeprüft. DerivedOverrides/Possessionweiteroffen, HumanRuntimeSnapshotbleibt. NeuerAncestry/Notify/Unknowntest;148Tests/Clippy/Formatbestanden. "+summary+" Nachweise analysis/reports/hit-wall-class-controller-proof.json, analysis/collision/ai-wall-metadata-validation.json/*-ai-wall-metadata.json; scripts/Verify-HitWallMetadata.py/Validate-AiWallMetadata.py. KeinneuerDuckErfolgnachweis, nativeTiming/Callbacks/DynamicActors/Navigationoffen. AndroidzumSchluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\HIT_WALL_METADATA.md."
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Native Wandklassen-Metadaten eingebunden." not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(summary)
