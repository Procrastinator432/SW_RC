# Serialisierte Klassendefaults

Stand 6. Oktober 2026, Analyse-Erweiterung nach Viewer 0.7. Die Android-App bleibt bei 0.7; der Rust-Paketleser und die PC-Szenenerzeugung lösen inzwischen skalare Defaults einschließlich Properties-Overrides auf.

`rc-package::classes::read` liest UClass-Payloads anhand der lokalen `core.dll`. UClass enthält keine normale UObject-Property-Liste am Anfang. Die Reihenfolge ist SuperField, ScriptText, nullterminierte Children-Referenzen, FriendlyName, versionsabhängiges CppText, Line/TextPos und weiteres Metadatenwort, Script, UState-Masken/Flags, ClassFlags/GUID, Dependencies, Namenlisten, ClassWithin/ConfigName und schließlich TaggedProperties.

ScriptSize bezeichnet logische Bytes. Objekt-/Namensreferenzen belegen logisch vier Bytes, werden auf Platte aber als CompactIndex gespeichert. Deshalb traversiert der Reader die beobachteten Class-Expressions mit Größen- und Rekursionsgrenzen, ohne sie auszuführen. Unbekannte Tokens werden abgewiesen. Das ist kein allgemeiner UnrealScript-Interpreter. Objekt-/Namensindizes, Übereinstimmung von SuperField/FriendlyName und vollständiger Payload-Verbrauch werden geprüft.

## Prüfung

System und Properties zusammen: 3.170 Klassen und 3.170 vollständig gelesene Default-Listen, 25.119 Tags, keine Lesefehler, fehlenden Eltern oder Vererbungszyklen. Bericht: [defaults-validation.json](../analysis/classes/defaults-validation.json). Der Audit besteht nun mit Exitcode 0.

Der bisherige Fehler in `engine.Pawn` ist behoben. Dessen `DamageMultipliers`-Array besitzt die Inner-Property `Core.StructProperty` mit Ziel `Pawn.DamageMultiplier`. Deshalb bestimmt die nullterminierte TaggedProperties-Liste des Elements die Grenze. Deklariert sind 15 Bytes, tatsächlich verbraucht werden 16 einschließlich des Abschlusses. Beide Größen bleiben im Bericht als `declared_bytes` und `bytes` erhalten. Das Element enthält `BoneName=Head`, `Multiplier=2.0`. Die 547 anschließenden Bytes werden jetzt als weitere Pawn-Defaults gelesen.

Der Metadatenreader folgt UArrayProperty -> Inner UStructProperty -> UStruct. Er prüft ArrayDim, PropertyFlags, Category, optionalen RepOffset und Zielreferenzen anhand der originalen Serializer. Lokale Elternklassen werden berücksichtigt; importierte Eltern-Metadaten werden nicht aus anderen Paketen herbeigeraten. Nicht unterstützte Arrays bleiben Raw. Vector/Rotator/Color und ältere Paketversionen werden nicht als Tagged-Struct-Arrays behandelt. Struct-Elementzahlen sind begrenzt und abgeschnittene Listen werden abgewiesen.

Vier Struct-Array-Defaults wurden strukturell dekodiert: zwei bei DwarfSpiderDroid, eines bei SuperBattleDroid und das Pawn-Array. Nur beim Pawn-Array unterscheiden sich angegebene und verbrauchte Größe. Die native TaggedProperties-Routine ruft den Property-Serializer auf und meldet Größenabweichungen; sie setzt den Stream nicht blind auf das deklarierte Ende zurück.

36 Rust-Tests und Clippy über alle Targets bestehen. Neue Tests prüfen sämtliche Präfix-Trunkierungen von Property-Metadaten und typisierten Arrays, Metadaten-Grenzen, den abweichenden Größenfall und die erhaltene folgende Property. Die letzte Android-Emulatorprüfung betrifft unverändert Viewer 0.7; die Default-Auflösung wird am PC ausgeführt; eine damit erzeugte entry-Szene wurde zusätzlich im bestehenden Android-Viewer geprüft.

## Nutzbare Befunde und Grenzen

- `engine.StaticMeshActor`: `bCollideActors`, `bBlockActors`, `bBlockPlayers` jeweils True; `CollisionRadius` und `CollisionHeight` jeweils 1.0. Karteninstanzen können diese Werte überschreiben; die bekannten 68 explizit nicht blockierenden geo_01a-Actors bleiben solche Overrides.
- `engine.PlayerStart`: unter anderem `bSinglePlayerStart`, `bCoopStart`, `bEnabled`, `bPrimaryStart` und `bDirectional` True. Fehlende Rotation/Location wird anhand der Felddefinition und der belegten Nullinitialisierung aufgelöst; Karteninstanzwerte überschreiben diese Defaults.
- Die Engine lädt zusätzlich `Properties.<Klassenname>Defaults`. Diese UClass-Exports liegen in GameData/Properties, nicht in der Lokalisierungsdatei System/properties.det. Deshalb wurde der Katalog um 1.868 dortige Klassen erweitert.
- `properties.PlayerCommandoDefaults` enthält beispielsweise MaxHealth 200, ShieldRechargeRate 20 und ShieldRechargeDelay 3 sowie Ausrüstungs-Overrides. Das sind gelesene Daten, noch kein implementiertes Gameplay.

Der reine Klassenbericht behält direkt serialisierte Defaults separat. Der neue [resolved-validation.json](../analysis/classes/resolved-validation.json) enthält die aufgelösten skalaren Werte und ihren Ursprung: Nullinitialisierung, Klassendelta, Properties-Override oder Instanzdelta. Objektpfade aus positiven Class-Paketreferenzen werden mit dem deklarierenden Paket qualifiziert; der Index bleibt ein lokaler Index dieses Quellpakets.

## Skalare Auflösung und geprüfte Reihenfolge

`UObject::InitClassDefaultObject` und `InitProperties` kopieren die bereits initialisierten Elternwerte und setzen die neu hinzugekommenen Felder auf Null. Danach liest UClass seine TaggedProperties. Im normalen Spielbetrieb wird `Properties.<Klassenname>Defaults` geladen. Eine Klasse, deren Name `<Elternklasse>Defaults` ist, schreibt ihre Tags im Nicht-Editor-Modus direkt in das Defaultobjekt dieser Elternklasse. Eltern-Overrides wirken damit auch in erbenden Klassen, bevor deren eigene Deltas und Overrides gelesen werden. Config und Lokalisierung folgen zuletzt.

Der Rust-Katalog bildet diese Reihenfolge für unterstützte skalare Felder ab. Nullwerte erfordern eine tatsächlich gelesene Felddefinition samt Typ und ArrayDim; Werte mit unpassendem Typ werden abgewiesen. Compound-Arrays/Structs werden nicht zusammengeführt. Config-/Localized-Flags werden mitgeführt; `instance` lehnt solche Werte ab, solange deren externe Auflösung fehlt. Der Resolver führt weder Constructor-/Script-Ereignisse noch Spawn-Auswahl aus und ist kein vollständiges UObject-Laufzeitsystem.

Die Tests prüfen Eltern-Override vor Kind-Delta, Kind-Override vor Enkelvererbung und abschließende Instanzwerte; außerdem Nullinitialisierung, ArrayDim, Typfehler, Config-Grenzen, Zyklen, fehlende Eltern, falsche Override-Eltern und Object-Paketherkunft.

## Startpunkte und Kollisionsbefunde

Alle 755 exakten Engine.PlayerStart-Exports in 79 Karten besitzen jetzt auflösbare Transforms. Die 666 zuvor explizit gelesenen Anchors bleiben bitgenau gleich; 89 kommen hinzu. `rc-scene` lädt dafür die benachbarten System-/Properties-Pakete bei der PC-Szenenerzeugung und schreibt die Anchors in das bestehende Snapshot-v3-Format. Direktes Android-CTM-Laden bleibt bei expliziten Transform-Tags, weil diese zusätzlichen Pakete dort noch nicht importiert werden.

`entry.PlayerStart0`: Location `(-64, 0, -44)` stammt aus der Karte, Rotation `(0, 0, 0)` aus der belegten Nullinitialisierung des Actor-Feldes. Die erzeugte [entry-defaults-Szene](../analysis/scenes/entry-defaults/scene.json) enthält 12 Dreiecke und einen Anchor. Im API-35-Emulator: Startpunkt anwählbar, Bewegung sichtbar, exakte Rückkehr, Reset und erneutes Laden; Crash-Puffer leer. Rendervergleich: 522.056 veränderte Pixel beim Startwechsel, 38.658 bei Bewegung, jeweils 0 bei Rückkehr/Reset/Recovery.

Die 334 aufgenommenen geo_01a-StaticMeshActors haben nach Auflösung der drei Kollisionsflags 266-mal True/True/True und 68-mal False/False/False. Das bestimmt noch keine Kollisionsform und implementiert keine Mesh-Kollision.

PlayerCommando: Radius 40, CollisionHeight 84 und BaseEyeHeight 60; MaxHealth 200, ShieldRechargeRate 20 und ShieldRechargeDelay 3 kommen aus Properties.PlayerCommandoDefaults. Diese Daten werden noch nicht als Spielerphysik oder Gameplay ausgeführt. Augenhöhen-Anwendung, Spawn-Auswahl, Mesh-Kollision und komplette Defaultobjekte bleiben offen.

## Reproduzieren

```powershell
cargo run -p rc-inspect --bin rc-class-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System' analysis/classes/defaults-validation.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\Properties'
cargo run -p rc-inspect --bin rc-defaults-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/classes/resolved-validation.json
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Lokale Belege

- [UClass/UField/UState/UStruct, TaggedProperties und SerializeExpr](../analysis/decompiled/class-serializers.c): core.dll 0x10126200, 0x1011d420, 0x10123b70, 0x10123630, 0x10125590, 0x10120d00.
- [GUID-, Dependency- und Namenlisten-Serializer](../analysis/decompiled/class-fields.c): 0x1011e300, 0x10124de0, 0x10122600.
- [UObject::Serialize](../analysis/decompiled/object-serializer.c): 0x101566d0, UClass-Sonderfall ohne Object-Tags.

- [Array-/Struct-Property-Metadaten und SerializeItem](../analysis/decompiled/array-property.c): core.dll 0x101658e0, 0x101640c0, 0x10165d10, 0x10167270, 0x101678d0.

- [Defaultobjekt-Initialisierung](../analysis/decompiled/default-initialization.c): core.dll 0x10157640 und 0x10157320.
