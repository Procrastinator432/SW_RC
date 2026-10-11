# Granatenfallrechnung und ursprüngliche Animationsfreigabe

Fortsetzung nach der Geschützgrundlage unter `analysis/mission4-flight`. Die letzte spielbare Lieferung bleibt **0.21.0** mit `analysis/mission3-playable/geo_01a-grenades.rcplay`. Diese Stufe bereitet Physik und Waffenablauf vor; sie enthält keine neue APK und noch keinen spielbaren Granatenwurf.

## Originale Actor-Fallrechnung

`rc-package/actor_falling.rs` übernimmt den vorbereiteten SSE-Rechenkern von `AActor::physFalling`, Adresse10490c63–10490d71 in der lokalen Originaldatei `engine.dll`. Der Actor verwendet die aktuelle PhysicsVolume, seine Masse und die Ergebnisse von `GetNetBuoyancy`. Die Geschwindigkeit entsteht aus gedämpfter alter Geschwindigkeit plus Beschleunigung und gewichteter Gravitation. Die ZoneVelocity beeinflusst den Bewegungsschritt, wird dabei aber nicht dauerhaft zur Actor-Geschwindigkeit addiert.

Solange `bBounce` gesetzt ist, überspringt der Originalcode an104913cd die spätere Geschwindigkeitsrekonstruktion und Terminalgeschwindigkeitsbegrenzung. Auch `MaxSpeed` aus `physProjectile` gehört nicht in diese Rechnung: Das Originalprojektil besitzt `Physics=PHYS_Falling`. Die bisherige Pawn-Fallphysik ist ebenfalls kein Ersatz.

Die native Schrittaufteilung ist separat übernommen: höchstens acht Iterationen je Aufruf einschließlich bereits verbrauchter Iterationen. Bei mehr als0.05s Restzeit wird die kleinere Zeit aus halber Restzeit und0.05s verwendet. Nach einem Bounce werden nur bei den ersten beiden Kontakten die unbenutzten Bewegungsanteile zurückgegeben; weitere Kontakte erhalten keinen Zeitrefund. Nach Erreichen des Iterationslimits bleibt Zeit unberechnet, wie im Original.

`Record-ActorFalling.py` führt die exportierten SSE-Instruktionen mit vorbereiteten Werten aus. `rc-actor-falling-check` vergleicht **4096 Fälle bitgenau**, jeweils Geschwindigkeit und Verschiebung. Binärdatei und Instruktionsliste sind durch SHA256 gebunden. Das belegt diesen Rechenkern. MoveActor, GetNetBuoyancy, Volume-Auswahl, tatsächliche Kontakte, Landen, Rutschen und die Umschaltung nach RestSpeed sind noch Hostgrenzen. Die Arithmetik setzt gültige vorbereitete endliche Werte und positive Masse voraus; der integrierende Host muss diese gewährleisten.

## DC-15s-Wurf und Inventar

Die Originalverknüpfung ist `hudarms.HudArmsDC15s` → `hudarms.HudArmsDC15sSet:ThrowGrenade`. Die Sequenz besitzt **29 Frames bei30 FPS**. Ihr Objekt `AnimNotify_ReleaseGrenade6`, Klasse `Engine.AnimNotify_ReleaseGrenade`, liegt bei normalisierter Zeit **0.6000000238418579**. Die erste Soundnotify liegt bei0.00003448276038 (`GEN_Sound.Foley_Movement.fol_GEN_grenadeThrow_01`), die zweite bei0.921724140644 (`GEN_Sound.Foley_Movement.fol_GEN_wepGrab_02`). Diese beiden Foley-Referenzen sind aufgelöst, ihre Wiedergabe wird erst mit dem Szenenhost verbunden.

`rc-game/grenade_throw.rs` bildet den ausgewählten Spieler-Waffenstatus ab: Der Beginn verbraucht keine Munition. Die einmalige Überschreitung der Release-Notify verbraucht eine Granate und liefert `ReleaseProjectile`. Der Status blockiert Schuss, AltFire, Nachladen und erneutes Werfen bis zum AnimEnd von Kanal0. Leerer Vorrat liefert das Leervorratsereignis. Zoom-/AmmoType-/HasAnim-Prüfung, ObtainAmmo, Netzwerkrollen, erzwungener Waffenwechsel und KI-Sonderfälle sind nicht Bestandteil dieses begrenzten Status. Die echte Unterdrückung der Game-Eingaben folgt bei der Integration; der Status liefert dafür das Gate.

`rc-grenade-release-assets` liest Sequenz, Notify-Objekte und texturiertes Waffenmesh aus den Originalpaketen und prüft drei Geometriezeitpunkte. `rc-grenade-release-check` treibt den Status mit der bestehenden originalen Channel-Fortschreibung. Bei60Hz gibt jeder der fünf Würfe anTick35 genau eine Granate frei und endet anTick57. Der sechste Versuch liefert das Leervorratsereignis. Wiedereintritt, verfrühter Verbrauch und doppelte Freigabe werden geprüft. Das ist noch kein Wurfnachweis durch die Originalkarte.

## Prüfung und nächste Arbeit

**1022 Bibliothekstests** bestehen: neun neue Fälle für Fallarithmetik, Zeitschritte, Bounce-Zeitbudget, Freigabe, Vorratsverbrauch und Waffenstatus. Zusätzlich bestehen4096 native Rechenvergleiche und der Lauf mit fünf Original-Wurfanimationen. Clippy über alle Targets, Formatierung und Diff-Prüfung sind im Ordner abgelegt; vorherige Liefer- und Grundlagenbelege bleiben unverändert.

Als Nächstes: aktuelle PhysicsVolume/GetNetBuoyancy und ruhende Granaten anbinden, den Flug an blockierenden Welt-/Actor-Kontakten abschneiden, Kontakt621 erst auf tatsächlich erreichter Flugbahn auslösen und sichtbare Explosionsziele ermitteln. Danach folgen Wurftaste/JNI, Live-Mesh und Foley/Bounce/Explosion, Bunker-/Hatch-/Kollisionsübergänge und der zusammenhängende GPU-/CPU-Android-Lauf. Erst daraus entsteht die neue spielbare APK mit passender Szene.
