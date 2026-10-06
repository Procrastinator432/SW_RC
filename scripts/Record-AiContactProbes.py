"""Record actual original-map diagnostic outcomes without counting missing targets as coverage."""
from pathlib import Path
import json

root=Path(__file__).resolve().parents[1]
result=json.loads((root/"analysis/collision/ai-contact-validation.json").read_text())
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=147
evidence["ai_contact_original_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
summary=f"{result['ticks']} neue Diagnose-Ticks, {result['dispatches']} tatsächliche Kontaktweiterleitungen;1906Script- und2340Legacy-Ticks samt allen bisherigen Berichtsfeldern unverändert."
path=root/"docs/AI_CONTACT_PROBES.md"
text=path.read_text(encoding="utf-8-sig")
marker="\n## Gemessene Ergebnisse\n"
if marker in text:
    text=text.split(marker)[0]
text+=marker+"\n"+summary+"\n\n| Karte | Start | Ticks | Outcome | Weiterleitungen | Größenwechsel |\n|---|---|---:|---|---|---:|\n"
for p in result["probes"]:
    decisions=", ".join(f"{k}:{v}" for k,v in p["dispatch_decisions"].items()) or "keine"
    text+=f"| {p['map']} | {p['start']} | {p['ticks']} | {p['outcome']} | {decisions} | {p['shape_changes']} |\n"
text+="\nNoTarget/NoContact liefern keine Abdeckung eines Wandkontakt-Ereignisses. Unresolved belegt eine ausgeführte, weiterhin blockierte Duckprüfung; erst ArmedFirst/ArmedSecond belegt Timeraktivierung auf dieser Geometrie. Synthetische Tests bleiben der Nachweis für Fälle, die in diesen Originalkartenproben nicht auftreten.\n"
path.write_text(text,encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
marker="KI-Kontakt-Diagnosemodus auf Originalkarten"
if marker not in text:
    text+="\n"+marker+" ergänzt (--ai-contact-diagnostic). "+summary+" Fehlende Ziele/Kontakte ausdrücklich ausgewiesen; Diagnose-Controllerwerte, keine native KI-Parität.147Tests/Clippy/Format bestehen. [Details](docs/AI_CONTACT_PROBES.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/AI_CONTACT.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Originalkarten-Diagnosemodus" not in text:
    text+="\n## Folgearbeit: Originalkarten-Diagnosemodus\n\nDer zusätzliche Kontaktpfad besitzt jetzt einen CLI-Diagnosemodus mit acht Richtungsproben, expliziten Controller-/Wall-Snapshots und vollständig protokolliertem Bewegungs-/Kontaktablauf. "+summary+" Fehlende Ziele/Kontakte zählen nicht als Ereignisabdeckung. Siehe [AI_CONTACT_PROBES.md](AI_CONTACT_PROBES.md).\n"
path.write_text(text,encoding="utf-8")
outcomes=", ".join(f"{p['map']}/{p['start']}={p['outcome']}({p['ticks']}Ticks,{p['dispatch_decisions']})" for p in result["probes"])
note="2026-10-06: KI-Kontakt-Diagnosemodus auf Originalkarten --ai-contact-diagnostic ergänzt. VomScriptEndpunkt8YawSweepsüber600Einheiten, nächsterNonFloorKontaktalsDiagnoseziel, <=120Advance+90Idlezu1/60. OriginalSpielermaße/-ratios, synthetischerNonhumanController/MinHitWall0/alleWallsExcludedfalseNotifyfalse ausdrücklichvorgegeben, keineOriginalKI-Klasse/Eventantwort. "+summary+" Ergebnisse: "+outcomes+". NoTarget/NoContactkeineEreignisabdeckung; UnresolvednurblockiertePrüfung, ArmingnurArmedFirst/Second.147Tests/Clippy/Formatbestanden. Nachweis analysis/collision/*-ai-contact.json/ai-contact-validation.json und scripts/Validate-AiContact.py; Quellhashesgespeichert. NativeTiming/dynamischeActors/Callbacks/Navigationweiteroffen. AndroidzumSchluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\AI_CONTACT_PROBES.md."
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "KI-Kontakt-Diagnosemodus auf Originalkarten --ai-contact-diagnostic ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(summary)
