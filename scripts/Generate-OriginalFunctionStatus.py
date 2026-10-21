"""Generate the dated, evidence-bounded feature and original-function inventory.

Requires a fresh rc-inspect scan of the installed GameData directory. This tool
does not infer implementation from matching names, test counts or class ancestry.
Only the explicit links below annotate individual original definitions. They
prove the stated partial path, never whole-function equivalence.
"""
from __future__ import annotations

import argparse
import collections
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import zipfile


STATUS = {
    "I": "Integriert im angegebenen Umfang",
    "T": "Teilweise / begrenzter Spielhost",
    "B": "Rekonstruierter Analysebaustein",
    "O": "Offen / keine Spielintegration belegt",
    "U": "Nicht einzeln bewertet",
    "E": "Plattformersatz / Portinfrastruktur",
}

# Keys identify exported UFunction definitions, not inherited occurrences.
# Even an I feature maps only a reviewed subset of a whole original function.
FUNCTION_LINKS = {
    "ctinventory.CTAmmoRecharging.PostBeginPlay": ["W01"],
    "ctinventory.CTAmmoRecharging.Tick": ["W01"],
    "ctinventory.CTAmmoRecharging.UseCharge": ["W01"],
    "ctinventory.CTAmmoRecharging.HasAmmo": ["W01"],
    "ctinventory.DC15Ammo.Tick": ["W01"],
    "ctinventory.DC15Ammo.UseCharge": ["W01"],
    "ctinventory.DC15s.Fire": ["W02"],
    "ctgame.BactaDispenser.Active.BeginState": ["I01", "I02"],
    "ctgame.BactaDispenser.Active.Tick": ["I01"],
    "ctgame.BactaDispenser.Active.EndState": ["I01", "I02"],
    "ctgame.CTPlayer.UsingItem.PlayerMove": ["P19"],
    "ctgame.CTPlayer.UsingItem.PlayerTick": ["P19"],
    "ctgame.CTPlayer.UsingItem.BeginState": ["P19"],
    "ctgame.CTPlayer.UsingItem.EndState": ["P19"],
    "ctgame.CTPlayer.UsingItem.StopUse": ["P19"],
    "ctgame.CTPlayer.Use": ["P18"],
    "ctgame.CTPlayer.StopUse": ["P18"],
    "ctcharacters.PlayerCommando.CalculateDamageFrom": ["D05"],
    "ctcharacters.PlayerCommando.SaveSquadTravelInfo": ["L06"],
    "ctcharacters.CloneCommando.Revive": ["D12"],
    "engine.Pawn.TakeDamage": ["D02", "D03", "D04", "D06"],
    "engine.Pawn.SetShields": ["D02"],
    "engine.Pawn.DoJump": ["P03"],
    "engine.Pawn.StartCrouch": ["P05"],
    "engine.Pawn.EndCrouch": ["P05"],
    "engine.Pawn.Reviving.BeginState": ["D12"],
    "engine.Pawn.Reviving.Tick": ["D12"],
    "engine.Pawn.Reviving.EndState": ["D12"],
    "engine.GameInfo.ReduceDamage": ["D06"],
    "engine.Actor.CalculateDamageFrom": ["D03"],
    "engine.Actor.KAddImpulse": ["I10"],
    "engine.Actor.HurtRadius": ["D08", "I10"],
    "engine.KarmaProp.AffectKarma": ["I10"],
    "engine.KarmaProp.Trigger": ["I09", "I10"],
    "engine.Actor.PlayOwnedSound": ["T02"],
    "engine.Actor.PlayAnim": ["N03"],
    "engine.Actor.LoopAnim": ["N03"],
    "engine.Actor.FinishAnim": ["I01"],
    "engine.Actor.AttachToBone": ["N12"],
    "engine.ParticleEmitter.SpawnParticle": ["I02", "G14"],
    "engine.Emitter.FadeOut": ["I02"],
    "gameplay.ACTION_PlaySound.InitActionFor": ["T02"],
    "gameplay.ACTION_PlaySound.CompleteWhenTimer": ["T02"],
    "gameplay.ACTION_TeleportToPoint.InitActionFor": ["K03"],
    "mpgame.DMGame.PendingMatch.BeginState": ["M12"],
}

# Exact addresses only, in the ordinary System DLL. Scalar variants are kept
# separately and are not automatically assigned the same proof.
NATIVE_LINKS = {
    ("engine.dll", "10591ab0"): ["I10"],
    ("engine.dll", "1048f900"): ["P02"],
    ("engine.dll", "10496dc0"): ["P03"],
    ("engine.dll", "1043f3b0"): ["P10"],
    ("engine.dll", "10501420"): ["N08"],
    ("engine.dll", "10450240"): ["N04"],
    ("engine.dll", "1044f750"): ["N10"],
    ("engine.dll", "1044faf0"): ["N04"],
    ("engine.dll", "10509270"): ["N09"],
    ("engine.dll", "10500fc0"): ["N02"],
    ("engine.dll", "1044e760"): ["G21"],
    ("engine.dll", "10452050"): ["G07"],
    ("engine.dll", "10452210"): ["G07"],
    ("core.dll", "10143b40"): ["N08", "G21"],
    ("d3ddrv.dll", "1001fc70"): ["G21"],
}


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def read_catalog(path: Path) -> list[dict]:
    section = ""
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        if line.startswith("@ "):
            section = line[2:]
            continue
        fields = line.split("|")
        if len(fields) != 8:
            raise ValueError(f"Expected eight fields: {line[:90]}")
        ident, title, status, scope, limit, code, proof, original = fields
        if status not in STATUS or not section:
            raise ValueError(f"Invalid feature status/section: {ident}")
        result.append(dict(id=ident, section=section, feature=title,
                           status=status, status_label=STATUS[status], scope=scope,
                           limit=limit, code=code.split(";"), evidence=proof.split(";"),
                           original=original))
    if len({x["id"] for x in result}) != len(result):
        raise ValueError("Duplicate feature id")
    return result


def write_csv(path: Path, rows: list[dict], fields: list[str] | None = None) -> None:
    if not rows:
        raise ValueError(f"No rows: {path}")
    with path.open("w", encoding="utf-8-sig", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields or list(rows[0]), delimiter=";", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def package_from_report(path: Path) -> str:
    return path.name.split("__")[-1].removesuffix(".json").removesuffix(".u")


def section_at(data: bytes, rva: int) -> str:
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    count = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    first = pe + 24 + opt_size
    for i in range(count):
        pos = first + i * 40
        virtual_size, virtual_address, raw_size = struct.unpack_from("<III", data, pos + 8)
        flags = struct.unpack_from("<I", data, pos + 36)[0]
        if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
            return "code" if flags & 0x20000000 else "data"
    return "unknown-section"


def classify_package(path: str) -> str:
    base = Path(path).stem.lower()
    if base in {"mpgame", "ipdrv", "xinterfacegamespy", "xinterfacemp"}:
        return "Multiplayer/Netzwerk"
    if base == "xinterfacelive":
        return "Xbox-/plattformabhängige Interfacefunktionen; PC-Nutzung ungeprüft"
    if base in {"editor", "unrealed", "uwindow", "udebugmenu"}:
        return "Editor/Debug/Legacy-UI; Relevanz einzeln offen"
    if base.startswith("xinterface"):
        return "Original-UI/Menüs"
    if base in {"core", "engine"}:
        return "Allgemeine Engine"
    return "Spiel-/Assetpaket"


def map_row(name: str) -> dict:
    lower = name.lower()
    selected = {
        "geo_01a.ctm": ("T", "Ausgewählter Missionsablauf spielbar", "K01", "K02–K07; keine vollständige Originalparität"),
        "geo_01b.ctm": ("T", "Ausgewählter Missionsablauf spielbar", "K08", "Lokaler Missions-/Kampfhost; Inszenierung teilweise"),
        "geo_01c.ctm": ("T", "Ausgewählter Missionsablauf spielbar", "K09", "K09–K12; Inszenierung/KI teilweise"),
        "geo_03a.ctm": ("T", "Nur Fabrikeinstieg integriert", "K13", "Weiterer Missionsverlauf offen"),
    }
    if lower.startswith("geo_"):
        group = "Geonosis"
    elif lower.startswith("ras_"):
        group = "Republic Assault Ship"
    elif lower.startswith("yyy"):
        group = "Kashyyyk"
    elif lower.startswith("ctf_"):
        group = "Multiplayer: CTF-Kartendatei"
    elif lower.startswith("dm_"):
        group = "Multiplayer: DM-Kartendatei"
    else:
        group = "Entry/Prolog/Epilog/Spezialkarte"
    if lower in selected:
        status, scope, feature, limit = selected[lower]
    elif "briefing" in lower or lower in {"pro.ctm", "epilogue.ctm"}:
        status, scope, feature, limit = "O", "Keine vollständige Abfolge integriert", "K17", "Originaldatei lesbar; Inhalt nicht als Spielabschnitt abgenommen"
    elif lower in {"entry.ctm", "1138.ctm", "mpcustom.ctm"}:
        status, scope, feature, limit = "U", "System-/Spezialinhalt; nicht als Kampagne abgenommen", "K18", "Konkreter Nutzungsumfang nicht einzeln bewertet"
    else:
        feature = "K14" if lower.startswith("geo_") else "K15" if lower.startswith("ras_") else "K16" if lower.startswith("yyy") else "M01" if lower.startswith("dm_") else "M03"
        status, scope, limit = "O", "Keine Spielintegration im aktuellen Paket", "Originaldatei lesbar; kein durchgängiger Port-Spielabschnitt belegt"
    return dict(map=name, original_group=group, status=status,
                scope=scope, feature_id=feature, limit=limit)


def generate(root: Path, scan: Path, out: Path) -> dict:
    catalog = read_catalog(root / "docs/original-function-status/features.psv")
    features = {x["id"]: x for x in catalog}
    for x in catalog:
        for path in x["code"] + x["evidence"]:
            if not (root / path).is_file():
                raise ValueError(f"Missing evidence for {x['id']}: {path}")
    inventory = json.loads((scan / "inventory.json").read_text(encoding="utf-8"))
    if inventory["parse_errors"]:
        raise ValueError("Original scan contains package/PE parse errors")
    game_data = Path(inventory["source"])
    # On Windows, extended paths are valid inputs. They are kept out of the report.
    source_string = str(game_data).removeprefix("\\\\?\\")
    game_data = Path(source_string)
    out.mkdir(parents=True, exist_ok=True)
    expected_packages = {r["path"] for r in inventory["files"] if "package" in r}
    actual_packages = {p.name.removesuffix(".json").replace("__", "/")
                       for p in (scan / "packages").glob("*.json")}
    if expected_packages != actual_packages:
        raise ValueError("Missing or stale package reports in scan directory")
    source_errors = sum(len(r.get("source_errors", [])) for r in inventory["files"])
    if source_errors:
        raise ValueError("Original scan contains ScriptText extraction errors")

    functions, classes, states, package_rows = [], [], [], []
    found_links = set()
    for path in sorted((scan / "packages").glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        package = package_from_report(path)
        original_path = path.name.removesuffix(".json").replace("__", "/")
        package_rows.append(dict(path=original_path,
                                 sha256=sha(game_data / original_path),
                                 exports=len(data["exports"]),
                                 class_count=0, function_count=0, state_count=0))
        for index, e in enumerate(data["exports"], 1):
            kind = e["class_path"]
            if kind not in {"Class", "Core.Function", "Core.State"}:
                continue
            original_id = package + "." + e["object_path"]
            source_file = scan / "sources" / original_path.replace("/", "__") / (e["object_path"].split(".")[0] + ".uc")
            common = dict(original_id=original_id, package=original_path,
                          export_index=index, object_path=e["object_path"],
                          package_group=classify_package(original_path))
            if kind == "Core.Function":
                links = FUNCTION_LINKS.get(original_id, [])
                if links:
                    found_links.add(original_id)
                common.update(
                    owner_class=e["object_path"].split(".")[0],
                    state=".".join(e["object_path"].split(".")[1:-1]),
                    function_name=e["name"],
                    serialized_bytes=e["serial_size"],
                    source_text_available=source_file.is_file(),
                    source_text_path=str(source_file.relative_to(root)).replace("\\", "/") if source_file.is_file() else "",
                    implementation_status="Teilpfad / semantischer Portadapter belegt; keine Ganzfunktionsparität" if links else "Nicht einzeln bewertet",
                    feature_ids=",".join(links),
                    scope=" / ".join(features[k]["scope"] for k in links),
                    limit=" / ".join(features[k]["limit"] for k in links),
                    evidence=";".join(sorted({p for k in links for p in features[k]["evidence"]})),
                )
                functions.append(common)
                package_rows[-1]["function_count"] += 1
            elif kind == "Class":
                common.update(super_class_index=e["super_class"],
                              source_text_available=source_file.is_file(),
                              implementation_status="Klasse inventarisiert; individuelle Verhaltensparität nicht bewertet")
                classes.append(common)
                package_rows[-1]["class_count"] += 1
            else:
                common.update(implementation_status="State inventarisiert; generische Ausführung nicht belegt")
                states.append(common)
                package_rows[-1]["state_count"] += 1
    unknown = set(FUNCTION_LINKS) - found_links
    if unknown:
        raise ValueError("Unknown exact original function links: " + ", ".join(sorted(unknown)))
    ids = [f["original_id"].casefold() for f in functions]
    if len(set(ids)) != len(ids):
        raise ValueError("Duplicate qualified original function definition")

    binaries, native, native_found = [], [], set()
    for record in inventory["files"]:
        if "pe" not in record:
            continue
        original_path = record["path"]
        original = game_data / original_path
        raw = original.read_bytes()
        binary_id = original_path.lower()
        pe = record["pe"]
        counts = collections.Counter()
        for e in pe.get("exports") or []:
            kind = "forwarder" if e.get("forwarder") else section_at(raw, e["rva"])
            counts[kind] += 1
            key = (Path(original_path).name.lower(), e["address"].lower())
            links = NATIVE_LINKS.get(key, []) if binary_id.startswith("system/") and binary_id.count("/") == 1 else []
            if links:
                native_found.add(key)
            native.append(dict(binary=original_path, ordinal=e["ordinal"],
                               symbol=e.get("name") or "", address=e["address"],
                               rva=e["rva"], kind=kind, forwarder=e.get("forwarder") or "",
                               implementation_status="Begrenzter Originalbaustein / Teilpfad belegt; keine Gesamtparität" if links else "Nicht einzeln bewertet",
                               feature_ids=",".join(links),
                               evidence=";".join(sorted({p for k in links for p in features[k]["evidence"]}))))
        binaries.append(dict(path=original_path, sha256=hashlib.sha256(raw).hexdigest(),
                             exported_symbols=len(pe.get("exports") or []), **dict(counts)))

    # Exact-address evidence may describe a non-exported internal entry. Such
    # entries are kept explicitly, rather than fabricated as a PE export.
    internal_known = [dict(binary="System/" + dll, address=address,
                           feature_ids=",".join(ids),
                           status="Dokumentierte interne Adresse; kein PE-Export an dieser Adresse gefunden")
                      for (dll, address), ids in NATIVE_LINKS.items()
                      if (dll, address) not in native_found]
    maps = [map_row(Path(f["path"]).name) for f in inventory["files"]
            if f["path"].lower().endswith(".ctm")]
    maps.sort(key=lambda x: x["map"].lower())
    modules = []
    for crate in ["rc-game", "rc-package", "rc-render", "rc-native"]:
        for p in sorted((root / "crates" / crate / "src").glob("*.rs")):
            if p.stem.endswith("_tests") or p.stem in {"tests", "lib"}:
                continue
            text = p.read_text(encoding="utf-8")
            prefix = []
            for line in text.splitlines()[:12]:
                if line.startswith("//!"):
                    prefix.append(line[3:].strip())
                elif prefix:
                    break
            rel = p.relative_to(root).as_posix()
            feature_ids = [x["id"] for x in catalog if rel in x["code"]]
            modules.append(dict(path=rel, crate=crate, sha256=sha(p),
                                declared_scope=" ".join(prefix),
                                feature_ids=",".join(feature_ids),
                                classification="Funktionsmatrix beachten; Modulvorhandensein allein beweist keine Spielintegration"))

    write_csv(out / "original-functions.csv", functions)
    write_csv(out / "original-classes.csv", classes)
    write_csv(out / "original-states.csv", states)
    write_csv(out / "original-packages.csv", package_rows)
    write_csv(out / "native-exports.csv", native)
    write_csv(out / "native-binaries.csv", binaries)
    if internal_known:
        write_csv(out / "known-internal-addresses.csv", internal_known)
    write_csv(out / "maps.csv", maps)
    write_csv(out / "port-modules.csv", modules)
    flat_catalog = [{**x, "code": ";".join(x["code"]), "evidence": ";".join(x["evidence"])} for x in catalog]
    write_csv(out / "features.csv", flat_catalog)
    (out / "features.json").write_text(json.dumps(catalog, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")

    release_path = root / "build/releases/0.53.0"
    delivery = json.loads((release_path / "source-and-delivery.json").read_text(encoding="utf-8"))
    release_check = {}
    for key in ["apk", "package"]:
        p = Path(delivery[key]["path"])
        digest = sha(p)
        if digest != delivery[key]["sha256"]:
            raise ValueError(f"Release hash mismatch: {p}")
        release_check[key] = dict(path=p.relative_to(root).as_posix(), sha256=digest)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    # Documentation-only commits do not change the audited game implementation.
    # Check runtime/build inputs against the release instead of requiring the
    # report itself to live at the old release's exact commit.
    runtime_paths = ["crates", "android", "config", "fixtures", "build.ps1", "Cargo.toml", "Cargo.lock"]
    changed_runtime = subprocess.check_output(
        ["git", "diff", "--name-only", delivery["source_commit"], "--", *runtime_paths],
        cwd=root, text=True,
    ).splitlines()
    changed_scripts = subprocess.check_output(
        ["git", "diff", "--name-only", delivery["source_commit"], "--", "scripts"],
        cwd=root, text=True,
    ).splitlines()
    changed_runtime += [p for p in changed_scripts if p != "scripts/Generate-OriginalFunctionStatus.py"]
    if changed_runtime:
        raise ValueError("Runtime/build inputs differ from the recorded release; review the catalog: " + ", ".join(changed_runtime))
    verification = json.loads((release_path / "verification-summary.json").read_text(encoding="utf-8"))
    counts = collections.Counter(x["status"] for x in catalog)
    metadata = dict(
        date="2026-10-21", source_commit=delivery["source_commit"], report_generation_commit=commit, release="0.53.0",
        runtime_inputs_match_release=True,
        inventory_files=len(inventory["files"]), parse_errors=inventory["parse_errors"],
        original_packages=len(package_rows), original_classes=len(classes),
        original_functions=len(functions), original_states=len(states),
        recovered_source_files=len(list((scan / "sources").rglob("*.uc"))),
        individual_function_evidence_links=len(found_links),
        individually_unrated_functions=len(functions) - len(found_links),
        original_maps=len(maps), feature_rows=len(catalog), feature_status_counts=dict(counts),
        original_pe_binaries=len(binaries), native_exported_symbols=len(native),
        native_export_kinds=dict(collections.Counter(x["kind"] for x in native)),
        native_address_links_exported=len(native_found), native_address_links_internal=len(internal_known),
        native_export_count_is_not_a_function_or_progress_count=True,
        all_internal_native_functions_enumerated=False,
        implementation_status_not_inferred_from_name_matching=True,
        current_release_hashes_verified=release_check,
        existing_release_tests_passed=verification.get("passed"),
        existing_release_library_tests=verification.get("library_tests"),
        tests_rerun_for_this_document=False,
        catalog_sha256=sha(root / "docs/original-function-status/features.psv"),
    )
    (out / "audit.json").write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    write_overview(root, out, catalog, maps, package_rows, metadata)
    # Read the actual deliverables back, checking every table's cardinality.
    for file, expected in [("original-functions.csv", len(functions)), ("original-classes.csv", len(classes)),
                           ("native-exports.csv", len(native)), ("maps.csv", len(maps)), ("features.csv", len(catalog))]:
        with (out / file).open(encoding="utf-8-sig", newline="") as f:
            actual = len(list(csv.DictReader(f, delimiter=";")))
        if actual != expected:
            raise ValueError(f"CSV roundtrip count mismatch: {file}")
    publish_central_bundle(root, out, metadata)
    return metadata


def publish_central_bundle(root: Path, out: Path, meta: dict) -> None:
    """Keep the central entry, generated data and portable bundle consistent."""
    central = root / "docs/original-function-status"
    report = root / "docs/ORIGINAL_FUNCTION_STATUS.md"
    entry = central / "README.md"
    archive = central / "Originalfunktionen_Umsetzungsstand_2026-10-21.zip"
    paths = [report, entry, central / "features.psv",
             root / "scripts/Generate-OriginalFunctionStatus.py",
             *sorted(out.glob("*.csv")), out / "features.json", out / "audit.json"]
    checked, optional_local, missing = 0, 0, []
    for page in [report, entry]:
        for target in re.findall(r"\]\(([^)]+)\)", page.read_text(encoding="utf-8")):
            if target.startswith(("#", "https:", "http:")):
                continue
            linked = (page.parent / target.split("#", 1)[0]).resolve()
            if linked in {archive, out / "overview-validation.json"}:
                continue  # Created below, then checked separately.
            if linked.is_relative_to(root / "analysis") or linked.is_relative_to(root / "build"):
                optional_local += 1
                continue
            checked += 1
            if not linked.is_file():
                missing.append(str(linked))
    if missing:
        raise ValueError("Broken central report links: " + ", ".join(missing))
    validation = dict(passed=True, central_entry="docs/original-function-status/README.md",
                      central_file_links_checked=checked, optional_local_evidence_links=optional_local,
                      missing_central_links=[], feature_rows=meta["feature_rows"],
                      original_functions=meta["original_functions"], original_maps=meta["original_maps"],
                      current_release_hashes_verified=True,
                      artifact_sha256={p.relative_to(root).as_posix(): sha(p) for p in paths})
    check_path = out / "overview-validation.json"
    check_path.write_text(json.dumps(validation, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for path in [*paths, check_path]:
            z.write(path, path.relative_to(root).as_posix())
        z.writestr("LIESMICH.txt", "Zentraler Funktionsbericht vom 21.10.2026, Release 0.53.0.\n"
                   "Start: docs/original-function-status/README.md\n"
                   "Bericht: docs/ORIGINAL_FUNCTION_STATUS.md\n"
                   "CSV-Dateien: UTF-8 mit BOM, Semikolon als Trennzeichen.\n"
                   "Enthalten sind Bericht, Katalog, Generator, Audit und alle Detailtabellen.\n"
                   "Verlinkte Originalquellen und historische Buildnachweise bleiben lokale Projektartefakte.\n"
                   "Ungepruefte Einzelfunktionen sind entsprechend gekennzeichnet.\n")
    with zipfile.ZipFile(archive) as z:
        if z.testzip() is not None:
            raise ValueError("Central archive integrity check failed")
    if not archive.is_file() or not check_path.is_file():
        raise ValueError("Missing central bundle artifact")


def write_overview(root: Path, out: Path, catalog: list[dict], maps: list[dict],
                   packages: list[dict], meta: dict) -> None:
    prefix = Path(os.path.relpath(out, root / "docs")).as_posix() + "/"
    def n(value: int) -> str:
        return f"{value:,}".replace(",", ".")
    def link(path: str, label: str | None = None) -> str:
        return f"[{label or Path(path).stem}](../{path})"
    def cell(value: str) -> str:
        return value.replace("|", "\\|").replace("\n", " ")
    lines = [
        "# Originalfunktionen und Umsetzungsstand",
        "", f"Stand: **21.10.2026 · Release 0.53.0 · Quellstand `{meta['source_commit'][:7]}`**.",
        "",
        "Die erste Mission besitzt einen zusammenhängenden ausgewählten Spielablauf über GEO_01A/B/C bis zum Fabrikeinstieg. "
        "Der Port rekonstruiert dafür einzelne Originalsysteme und verwendet begrenzte Rust-Spielhosts. "
        "Eine vollständige Originalengine, die weitere Kampagne und ein Multiplayer-Spielbetrieb sind noch nicht vorhanden.",
        "",
        "Diese Bestandsaufnahme umfasst die Funktionsbereiche des installierten Originals sowie **jede der "
        f"{n(meta['original_functions'])} exportierten UFunction-Definitionen**. Sie macht fehlende Einzelbewertungen sichtbar. "
        "Das ist kein Nachweis, dass sämtliche internen nativen Routinen bekannt oder bewertet sind.",
        "",
        "## Umfang und Bewertungsregeln", "",
        f"- Frisch gelesen: **{meta['inventory_files']} Dateien unter GameData**, **{meta['original_packages']} Unreal-Pakete**, "
        f"**{n(meta['original_classes'])} Klassen**, **{meta['original_states']} States**, **{meta['original_maps']} Karten**; keine Lesefehler.",
        f"- Zusätzlich: **{n(meta['recovered_source_files'])} extrahierte ScriptText-Dateien** und **{n(meta['native_exported_symbols'])} "
        "native Exportsymbole** aus den mitgelieferten PE-Dateien, einschließlich Scalar-Varianten, Editor und Fremdbibliotheken.",
        f"- **{meta['feature_rows']} Funktions-/Systemzeilen** bilden den fachlichen Katalog. Teilfunktionen und Infrastruktur sind "
        "unterschiedlich groß und überschneiden sich bewusst; ihre Anzahl liefert keinen seriösen Fertigstellungsprozentsatz.",
        "- **I** heißt im genannten Umfang integriert. Es bedeutet weder Ganzfunktionsparität noch eine vollständige Handyabnahme. "
        "**T** umfasst bewusst begrenzte oder vereinfachte Spielhosts. **B** bezeichnet technische Rekonstruktionen mit expliziten Hostgrenzen.",
        "- **O** heißt: im geprüften aktuellen Spielpfad keine Umsetzung belegt. **U** heißt: nicht einzeln bewertet; daraus wird kein Fehlen abgeleitet. "
        "**E** bezeichnet den Android-/Rust-Ersatz einer Originalplattformfunktion oder notwendige Portinfrastruktur.",
        "- Ein gelesener Default, eine vorhandene Rust-Datei oder ein gleichnamiges Symbol begründet allein keinen positiven Umsetzungsstatus. "
        "Das gilt auch für Klassen ohne eigene UFunction-Exports: Ihr wichtiges Verhalten kann vollständig nativ oder geerbt sein.",
        "- Originalpakete enthalten auch Editor-, Debug- und Xbox-/plattformabhängige Klassen. Der technische Index führt sie mit; "
        "die tatsächliche PC-Spielverfügbarkeit dieser Funktionen wird nicht pauschal behauptet.",
        "",
        "| Kürzel | Bedeutung | Katalogzeilen |", "|---|---|---:|",
    ]
    for status, label in STATUS.items():
        lines.append(f"| {status} | {label} | {meta['feature_status_counts'].get(status, 0)} |")
    lines += ["", "## Aktuell belegte Lieferung und Prüftiefe", "",
        "APK und Szenenpaket von 0.53.0 sind vorhanden; ihre SHA-256-Werte wurden für diese Übersicht erneut mit "
        "`source-and-delivery.json` verglichen. Die geprüften Laufzeit- und Builddateien entsprechen dem dort gebundenen Releasequellstand; reine Dokumentationsänderungen bleiben davon getrennt.",
        "",
        "Die vorhandenen Abschlussberichte nennen **je 1.175 Debug-/Release-Bibliothekstests, 30 Szenenstufen, 25 Missions-Checkpointproben, 39 "
        "Spieleransichten und Android-x86_64-Prüfungen**. Diese Übersicht führt den Gesamtbau nicht erneut aus. "
        "Der neue Scan, die Releasehashprüfung, die Existenzprüfung sämtlicher verlinkter Port-/Nachweisdateien, "
        "die exakten Funktionszuordnungen und die Vollständigkeit der erzeugten Tabellen wurden hier ausgeführt.",
        "",
        "Der automatische Missionslauf benutzt reguläre aufgezeichnete Eingaben. APK-Übergangstests verwenden zusätzlich echte "
        "Replay-Endspielstände als Einstieg. Diese Prüfungen belegen keinen vollständigen freien Touch-Durchlauf auf einem physischen "
        "ARM64-Handy. Die älteren Nutzerrückmeldungen belegen reale Lücken früherer Versionen; für 0.53.0 sind sie gezielt nachzuprüfen.",
        "",
        "Maßgeblich sind " + link("docs/DC15_ENERGY.md", "DC-15s / 0.53.0") + ", " + link("docs/KARMA_INTERACTIONS.md", "Karma / 0.52.0") + " und " +
        link("build/releases/0.53.0/verification-summary.json", "Release-Prüfbericht") + ". "
        "Frühere Dokumente beschreiben eingefrorene Zwischenstände. Aktuelle Korrekturen haben bei Widersprüchen Vorrang.",
        "",
        "## Funktionsmatrix", "",
        "Die letzte Spalte verlinkt Portdateien und fachliche Nachweise. Originalklassen/-funktionen stehen jeweils in der "
        "Umfangsspalte und zusätzlich maschinenlesbar im Katalog. `I` gilt stets nur für den angegebenen Szenen-/Systemumfang.",
    ]
    current = None
    sections = list(dict.fromkeys(x["section"] for x in catalog))
    for section in sections:
        anchor = "".join(c for c in section.lower() if c.isalnum() or c in " -").replace(" ", "-")
        lines.append(f"- [{section}](#{anchor})")
    for x in catalog:
        if x["section"] != current:
            current = x["section"]
            lines += ["", "### " + current, "", "| ID / Originalfunktion | Stand | Belegter Umfang und offene Grenze | Belege |", "|---|:---:|---|---|"]
        refs = "; ".join(link(p) for p in x["evidence"])
        code = link(x["code"][0], "Code")
        detail = f"{x['scope']} **Offen/Grenze:** {x['limit']} Originalbezug: `{x['original']}`."
        lines.append(f"| {x['id']} · {cell(x['feature'])} | **{x['status']}** | {cell(detail)} | {code}; {refs} |")

    lines += ["", "## Sämtliche installierten Karten", "",
        "Eine CTM-Datei ist keine gleich große Mission. Briefings, System-/Multiplayerkarten und Spielabschnitte sind getrennt "
        "aufgeführt. Nur A/B/C besitzen den ausgewählten durchgängigen Missionsablauf; GEO_03A ist ein begehbarer Eintritt.",
        "", "| Originaldatei | Gruppe | Stand | Portumfang |", "|---|---|:---:|---|"]
    for row in maps:
        lines.append(f"| `{row['map']}` | {row['original_group']} | {row['status']} | {row['scope']} · {row['feature_id']} |")

    lines += ["", "## Technischer Funktionsindex", "",
        f"[{n(meta['original_functions'])} Originalfunktionen]({prefix}original-functions.csv) enthalten jede in den frischen "
        "Pakettabellen gespeicherte `Core.Function`-Definition mit qualifiziertem Namen, Paket, Exportindex, Klasse, optionalem State, "
        "Quelltextverfügbarkeit und Umsetzungsbewertung. Der Index zählt Definitionen; geerbte Verwendung und Aufrufhäufigkeit zählen nicht zusätzlich.",
        "",
        f"**{meta['individual_function_evidence_links']} Funktionen** besitzen eine bewusst manuelle Zuordnung zu einem belegten Teilpfad oder semantischen Portadapter. "
        f"**{n(meta['individually_unrated_functions'])}** sind **nicht einzeln bewertet**. Das bedeutet nicht, dass sie alle fehlen. "
        "Auch eine zugeordnete Funktion gilt nicht automatisch in ihrer Gesamtheit als originalgetreu portiert. "
        "Die Auswahl ist keine vollständige Liste bereits geleisteter Detailrekonstruktionen; sie ist eine konservative Einzelbelegliste.",
        "",
        f"[Native Exportsymbole]({prefix}native-exports.csv) führen Code, Daten und Forwarder getrennt. "
        "Ausführbarer PE-Abschnitt kennzeichnet ein Codesymbol, beweist aber keine eigene logische Funktion: Aliase, Thunks, Konstruktoren, "
        "Compilerhilfen, Plattformvarianten und Fremdbibliotheken kommen mehrfach vor. Unexportierte interne Funktionen fehlen in PE-Exporttabellen "
        "grundsätzlich. Einzelfunktionen ohne Nachweis bleiben ungeprüft; die Symbolanzahl ist kein Portierungsnenner.",
        "",
        "| Paketgruppe mit UFunctions | Klassen | Funktionsdefinitionen | States |", "|---|---:|---:|---:|"]
    for row in packages:
        if row["function_count"]:
            lines.append(f"| `{row['path']}` | {row['class_count']} | {row['function_count']} | {row['state_count']} |")
    lines += ["", "## Dateien und Aktualisierung", "",
        f"- [Funktionsmatrix als CSV]({prefix}features.csv) und [JSON]({prefix}features.json).",
        f"- [Alle UFunctions]({prefix}original-functions.csv), [Klassen]({prefix}original-classes.csv), [States]({prefix}original-states.csv) und [Pakete samt SHA-256]({prefix}original-packages.csv).",
        f"- [Alle nativen Exportsymbole]({prefix}native-exports.csv), [Binärdateien samt SHA-256]({prefix}native-binaries.csv) und [dokumentierte interne Adressen ohne PE-Exporttreffer]({prefix}known-internal-addresses.csv).",
        f"- [Alle Karten]({prefix}maps.csv), [Rust-Modulbestand und deklarierte Grenzen]({prefix}port-modules.csv), [Audit mit Zählungen und Releasehashes]({prefix}audit.json).",
        "- [Redaktioneller Katalog](original-function-status/features.psv) und [Generator](../scripts/Generate-OriginalFunctionStatus.py). CSV-Dateien verwenden UTF-8-BOM und Semikolon als Trennzeichen.",
        "",
        "Zentraler Einstieg ist [docs/original-function-status/README.md](original-function-status/README.md). "
        "Bericht, Detailtabellen, Audit, Katalog und Generator liegen im versionierbaren Dokumentationsbereich des Projekts. "
        "Der Rohscan der Originalinstallation und historische Buildnachweise bleiben lokale Analyseartefakte; "
        "sie sind zum Lesen der Übersicht und der Statusindizes nicht erforderlich. "
        "Die Quelltextpfade in den Tabellen verweisen auf diesen optionalen lokalen Rohscan.",
        "", "```powershell",
        ".\\target\\release\\rc-inspect.exe 'D:\\SteamLibrary\\steamapps\\common\\Star Wars Republic Commando\\GameData' analysis/function-status-20261021/original-scan",
        "python scripts/Generate-OriginalFunctionStatus.py --scan analysis/function-status-20261021/original-scan",
        "```", "",
        "Bei einem neuen Release zuerst Katalog, exakte Einzelbelege und die gepinnte Release-/Commitzuordnung im Generator aktualisieren. "
        "Erst danach neu erzeugen. Ein bloßer neuer Build oder steigende Testzahl ändern den Status keiner Funktion automatisch.",
        "",
        "## Größte verbleibende Funktionslücken", "",
        "1. Freie Bild-/Hör-/Touch-Abnahme von 0.53.0 auf dem Handy: Intro, Bacta, Barrieren, Wände/Bunker, Dialoge, Schaden und A/B/C-Übergänge.",
        "2. Vollständiger Spieler-Waffenumfang: DC-17-Aufsätze, Waffenwechsel, Spielernahkampf, weitere Granaten und Fremdwaffen.",
        "3. Einheitliche originale Kampf-/KI-/Squadregeln über alle drei Karten; vollständige Charakter-/Spieleranimation und Handzeichen.",
        "4. Material-/Licht-/FX-Abdeckung und Texturqualität, insbesondere bisher grobe Wracks und ausgelassene Effektflächen.",
        "5. Musik, Raumklang, Material-Foley und vollständige situationsabhängige Sprachausgabe.",
        "6. Weitere Kampagnenabschnitte ab GEO_03A; danach RAS und Kashyyyk mit ihren zusätzlichen Gegner-/Interaktionssystemen.",
        "7. Profil-/Kapitel-/Optionsumfang und physische Leistungs-/Langzeitabnahme. Multiplayer ist ein eigener größerer Funktionsblock.",
        "",
    ]
    (root / "docs/ORIGINAL_FUNCTION_STATUS.md").write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--scan", type=Path, required=True)
    p.add_argument("--out", type=Path, default=Path("docs/original-function-status/data/2026-10-21"),
                   help="Central versioned data directory (default: dated docs snapshot)")
    args = p.parse_args()
    root = Path(__file__).resolve().parents[1]
    meta = generate(root, (root / args.scan).resolve(), (root / args.out).resolve())
    print(json.dumps(meta, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
