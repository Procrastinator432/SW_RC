"""Record measured expanded target-search outcomes, keeping predictions separate."""
from pathlib import Path
import json

root=Path(__file__).resolve().parents[1]
result=json.loads((root/"analysis/collision/ai-duck-target-validation.json").read_text())
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=147
evidence["ai_duck_target_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
armed=sum(p["dispatch_decisions"].get("ArmedFirst",0)+p["dispatch_decisions"].get("ArmedSecond",0) for p in result["probes"])
predicted=sum(p["search_predictions"].get("Armed",0) for p in result["probes"])
summary=f"{result['ticks']} neue Diagnose-Ticks, {result['dispatches']} tatsächliche Kontaktweiterleitungen, {armed} tatsächliche Timeraktivierungen und {predicted} Armed-Suchvorhersagen.1906Script- und2340Legacy-Ticks samt sämtlichen bisherigen Berichtsfeldern unverändert."
path=root/"docs/AI_DUCK_TARGETS.md"
text=path.read_text(encoding="utf-8-sig")
marker="\n## Gemessene Ergebnisse\n"
if marker in text:
    text=text.split(marker)[0]
text+=marker+"\n"+summary+"\n\n| Karte | Start | Ticks | Outcome | Suchprognosen | Ausgeführte Entscheidungen | Größenwechsel |\n|---|---|---:|---|---|---|---:|\n"
for p in result["probes"]:
    predictions=", ".join(f"{k}:{v}" for k,v in p["search_predictions"].items()) or "keine"
    decisions=", ".join(f"{k}:{v}" for k,v in p["dispatch_decisions"].items()) or "keine"
    text+=f"| {p['map']} | {p['start']} | {p['ticks']} | {p['outcome']} | {predictions} | {decisions} | {p['shape_changes']} |\n"
if armed==0:
    text+="\nDie erweiterten Originalkartenläufe belegen weiterhin keine erfolgreiche Duckaktivierung. Die ausgewählten Wege bleiben blockiert. Die synthetischen Integrationstests sind weiterhin der Nachweis für erfolgreiche Kontaktaktivierung/Duckbewegung/Timerablauf. Die Suche ist auf diese drei Endpunkte und32horizontale Richtungen beschränkt; daraus folgt keine Aussage über sämtliche Originalkarten-Duckstellen.\n"
path.write_text(text,encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Erweiterte Duckziel-Suche" not in text:
    text+="\nErweiterte Duckziel-Suche (--ai-duck-target-diagnostic):32Richtungen/1500Einheiten, ersterHitjeRichtung, separateCanCrouchWalk-Prognose amgeschätztenKontaktpunkt. "+summary+"147Tests/Clippy/Formatbestehen; keine nativeKI-/Callbackparität. [Details](docs/AI_DUCK_TARGETS.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/AI_CONTACT_PROBES.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: erweiterte Zielsuche" not in text:
    text+="\n## Folgearbeit: erweiterte Zielsuche\n\nEin zusätzlicher Modus sucht in32Richtungen über1500Einheiten und bewertet den Duckweg separat am geschätzten ersten Kontaktpunkt. Vorhersagen werden getrennt von tatsächlich ausgeführten Kontaktentscheidungen gezählt. "+summary+" Siehe [AI_DUCK_TARGETS.md](AI_DUCK_TARGETS.md).\n"
path.write_text(text,encoding="utf-8")
outcomes=", ".join(f"{p['map']}/{p['start']}={p['outcome']}({p['ticks']}Ticks,{p['dispatch_decisions']},Prognose{p['search_predictions']})" for p in result["probes"])
note="2026-10-06: Erweiterte Duckziel-Suche --ai-duck-target-diagnostic ergänzt.32YawRichtungen/1500Einheiten, nurersterHitjeRichtung; geschätzteKontaktpositionSkin/Approach, separateCanCrouchWalk-Prognose, preferArmedsonstnearestblocked. <=300Advance+90Idle; keineNavigation/Steigungsrekonstruktion. "+summary+" Ergebnisse: "+outcomes+". PrognosenzählenkeineKontaktabdeckung; tatsächlichArmingnurArmedFirst/Second.147Tests/Clippy/Formatbestanden. Vorherige8Richtungs-Probenhistorischbehalten. Nachweis analysis/collision/*-ai-duck-target.json/ai-duck-target-validation.json und scripts/Validate-AiContact.py --duck-targets. SynthetischeController/WallSnapshotsweiterexplizit; nativeTiming/dynamischeActors/Callbacks/Navigationoffen. AndroidzumSchluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\AI_DUCK_TARGETS.md."
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Erweiterte Duckziel-Suche --ai-duck-target-diagnostic ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(summary)
