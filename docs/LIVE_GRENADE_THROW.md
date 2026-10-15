# Originaler Granatenwurf im spielbaren Host

Stand: 12. Oktober 2026. Zwischenlieferung **0.22.0**, VersionCode25.

## Ausprobieren

- APK: `analysis/mission4-live-throw/republic-commando-0.22.0.apk` (ARM64 und x86_64).
- Passende Szene: `analysis/mission4-live-throw/geo_01a-throw.rcplay`.
- Nach Bacta, den fünf Droiden und der Granatenaufnahme bleibt der Abschnitt offen. Dem Wegmarker zur Geschützstellung folgen. **GRANATE** einmal antippen; mit Tastatur **G**. Links bewegen, rechts Blick drehen. Ein höher gezielter Wurf über die vordere Kante erreicht den originalen GrenadeProj-Trigger.
- Halten wirft nicht wiederholt. Eine neue Betätigung löst den nächsten Wurf aus. Ein leerer Vorrat spielt das originale Leersignal.
- Ältere Szenen behalten ihren bisherigen Abschluss. Die neue Szene benötigt App0.22.0 und kann nicht in0.21.0 geladen werden.

Dies ist der spielbare Wurfabschnitt, **noch kein vollständiger Geschützabschluss**. Bei einem gültigen Granatenkontakt erscheint „Geschütz getroffen“. Bunker/Hatch und ihre Kollision werden noch nicht umgebaut; der Abschnitt zeigt deshalb keinen Sieg und öffnet den weiteren Missionsweg noch nicht.

## Implementierter Ablauf

`RCPLAY05` umschließt genau eine `RCPLAY04`-Mission. Die begrenzten Zusatzdaten enthalten Original-Animationsrelease, Projektilregeln, Volumeneigenschaften, Wegpunkte, Texturmesh und Sounds. Das ursprüngliche Missionsmetadata bleibt bytegleich. Im visuellen Unterpaket ersetzt das bereits extrahierte DC-15s-Mesh die alte Clipauswahl durch dieselben Grundclips samt originalem ThrowGrenade-Clip. Die reparierten Arena-Oberflächen bleiben erhalten.

Das Game nimmt eine Wurfflanke entgegen und spielt den echten, nicht schleifenden Waffensequenzkanal. Inventarverbrauch und Erzeugung des Projektils erfolgen erst am Release-Notify bei0.6000000238418579. Bei60Hz fällt der Release aufTick35, das Ende aufTick57. Feuer und Nachladen sind bis einschließlich dieses Endticks gesperrt; während einer laufenden Nachladung startet kein Wurf. Kurze Eingaben ohne Simulationstick bleiben vorgemerkt; Pause und Neustart entfernen vorgemerkte Eingaben. Eine laufende Wurfanimation bleibt beim Pausieren erhalten.

Der Release verwendet das dann aktuelle Auge, die Engine-Rotationsmatrix, den originalen FPGrenadeOffset und die aktuelle Pawn-Geschwindigkeit. Bereits bestehende Projektile bewegen sich zuerst; ein in diesem Tick erzeugtes Projektil startet mitAlter0. Diese Reihenfolge ist eine explizite Hostentscheidung. Das frühere trockene Kollisions-/Fallmodul begrenzt alle Flugsegmente an Weltflächen. Nur tatsächlich durchlaufene Segmente können Trigger621 auslösen. Bis zu32 gleichzeitig aktive Projektile sind zugelassen.

Die352 texturierten Originaldreiecke werden mit Welt-Tiefe vor dem Waffenpass gezeichnet. Texturen werden beim Laden einmal an die Szene gebunden. CPU und GPU erhalten dieselben dynamischen Flächen. Das Live-Releasebild zeigt1189 vom Projektil veränderte Pixel; die gespeicherte CPU-Ansicht wurde visuell geprüft. Welt-/Texturpaket bleiben während des Laufs unverändert.

Originalaudio:

| Rolle | Originalsample | Verwendung |
|---|---|---|
| Wurffoley | GEN_Sound.Foley_Movement.fol_GEN_grenadeThrow_01 | Originalnotify0.00003448276038 |
| Waffe greifen | GEN_Sound.Foley_Movement.fol_GEN_wepGrab_02 | Originalnotify0.921724140644 |
| Abprall | GEN_Sound.Impacts_Explos.imp_GEN_grenadeBounce_01 | Geprüfte Script-Abprallregel |
| Vorrat leer | GEN_Sound.Interface.int_GEN_selGrnEmpty_01 | Neue Betätigung bei leerem Vorrat |
| Bestätigung | GEO_Voice.GEO_01.CA1G1716 | Vorbereitet; noch nicht abgespielt, da Bunkerzerstörung offen |

Die bisherigen13 Rollen einschließlich DC-15s-Schuss bleiben erhalten. Thermaldetonator-Explosionssound/-effekt und sichtbarer Flächenschaden sind noch offen: Der Zünder endet korrekt und entfernt das Projektil, erzeugt aber noch keinen vollständigen Explosionsauftritt.

## Nachweise

- Vollständiger Rust-Lauf vom Bacta-Einstieg über alle bisherigen Ziele zur Geschützstellung, ohne Teleport oder direkt gesetzten Missionszustand. Der grobe Lauf lässt den Pawn zunächst ausrollen; langsame tatsächliche Steuereingaben führen ihn zurück zum letzten Wegpunkt. Standpunkt beim Wurf etwa `[4040.269,-6379.026,-2005.094]`, Nullgeschwindigkeit. Der Wurf zielt0.4rad höher als das Triggerzentrum.
- Release35/Ende57, Waffensperre gegen gleichzeitiges Feuer/Nachladen, fünf Verbräuche und anschließendes Leersignal, kein Wiederholungswurf durch Halten, einmaliges Zünderende des ersten Wurfs, echter Triggerkontakt, positive Live-Sichtbarkeit, atomarer Game-Rollback bei ungültigen Flugvolumen, Pause und exakter Zustands-/Bildneustart.
- Derselbe zusammenhängende Lauf über die Desktop-ABI: Eingabebit5, Waffensperre, fünf Foley-Paare, Abprall/Leer, Originaltriggerkontakt, kein vorzeitiger Sieg, Puffergrenzen und Neustart. JNI reicht denselben Bitmaskenwert durch; Android besitzt eine eigene Pointerrolle und Kurzpressen-Warteschlange für GRANATE.
- Ältere Arena-Szene endet unverändert anTick5030 mit fünf Droiden und fünf Granaten. Neue Standardfelder werden bei fehlender Erweiterung nicht serialisiert; alte Replays bleiben im bisherigen Format.
- **1033 Bibliothekstests** und Clippy für alle Targets bestanden. APK für ARM64/x86_64 erfolgreich gebaut. Android-Lauf/GPU-Abnahme dieser Version ausdrücklich noch ausstehend; keine neue Emulator- oder Geräteprüfung.

Szenen-/APK-Hashes, Logdateien und Quellenarchiv sind in `analysis/mission4-live-throw/validation-manifest.json` gebunden. Frühere Lieferungen bleiben unverändert.

## Nächster Abschnitt

Originale Bunker-, Hatch- und Kollisionsobjekte binden; die verzögerten Effekte ausScripts325/754 sichtbar anwenden, CA1G1716 erst im passenden Ablauf starten und den Geschützabschnitt korrekt beenden. Vor dessen Lieferung Thermaldetonator-Explosion mit Originalaudio/-darstellung und sichtbarer Schadensauswahl ergänzen. Danach den ganzen Abschnitt auf Android prüfen. Vollständige Karma-/Projektilfilter-/KI-/Originalkampfrekonstruktion und die übrige erste Mission bleiben separate offene Arbeit.
