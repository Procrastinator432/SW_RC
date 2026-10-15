"""Derive selected one-shot gameplay chains from the original actor inventory."""
import json
from pathlib import Path
root=Path("analysis/mission17-map2-gameplay")
rows=json.loads((root/"original-map-inventory.json").read_text())
by={r["original"]:r for r in rows}
def props(r): return {v["name"]:v["value"].get("value") for v in r["properties"]["values"]}
def val(r,name,default=None):return r["resolved"].get(name,{}).get("value",{}).get("value",props(r).get(name,default))
def point(n):return val(by[n],"Location")
scripts=[]; omitted=[];sounds=[]
for ident in [516,655,635,438,630,152,569,632,199,171,400,829,830,191,525,210,150,1284,1286,1288,1290,1292,1359]:
 r=by[ident];actions=[]
 for n,_ in r["actions"]:
  x=by[n];d=props(x);op=x["class"].split("ACTION_")[-1]
  a=dict(original=n,op="",text="",number=0,seconds=0.)
  if val(x,"bDisabled",False):omitted.append(dict(original=n,reason="original bDisabled"));continue
  if op=="WaitForEvent":a.update(op="wait",text=d["ExternalEvent"])
  elif op=="TriggerEvent":a.update(op="emit",text=d["Event"])
  elif op=="ObjectiveUpdate":a.update(op="objective",number=d.get("ObjectiveNum",0))
  elif op=="ShowActorObjectiveTracker":a.update(op="tracker",text=d.get("ActorTag",""))
  elif op=="WaitForTimer":a.update(op="timer",seconds=d["PauseTime"])
  elif op=="PlaySound":
   source=d["Sound"]["path"]
   if source not in sounds:sounds.append(source)
   a.update(op="sound",text=source,number=sounds.index(source)+1,seconds=-1. if val(x,"bWaitForFinish",False) else 0.)
  elif op=="ChangeSquadMember" and d.get("bAddMember",True):a.update(op="join")
  else: omitted.append(dict(original=n,action=op,reason="bounded host or presentation not executed by selected event interpreter"));continue
  actions.append(a)
 scripts.append(dict(original=ident,actions=actions))
triggers=[]
for r in rows:
 d=props(r)
 if r["class"]=="Engine.Trigger" and d.get("Event") and d.get("bInitiallyActive",True):
  triggers.append(dict(original=r["original"],event=d["Event"],position=point(r["original"]),radius=val(r,"CollisionRadius",40.),height=val(r,"CollisionHeight",40.)))
factories=[]
for r in rows:
 if r["class"]!="CTGame.PawnFactory":continue
 d=props(r);prototype=val(r,"Prototype")["path"]
 health=next(x["resolved"]["Ok"]["value"]["value"] for x in r["prototype_defaults"] if x["property"]=="Health")
 factories.append(dict(original=r["original"],event=val(r,"Tag"),position=point(r["original"]),spawn_points=[point(x["original"]) for x in rows if x["class"]=="CTGame.SpawnPoint" and val(x,"Tag")==val(r,"Tag")],health=int(health),warrior="Warrior" in prototype,capacity=val(r,"Capacity",1),interval=val(r,"SpawnInterval",1.),death_event=val(r,"PawnEventFromFactory","") or ""))
# Runtime host uses hold-to-command and remote detonation, keeping original actor events.
interactions=[]
for n,label,pre,under,event,demo in [
 (536,"Scorch: Tuer sprengen","Dialog40BehindDoor","BrokenDoorDemoUnderway","DoorTo40Destroyed",True),
 (386,"Fixer: Schildkonsole slicen","ActivateSecurityShieldHack","EntranceShieldHackUnderway","EntranceShieldHacked",False),
 (202,"Fixer: Sicherheitstuer slicen","DialogSecureDoor","StartingHangarDoorHack","HangarDoorHacked",False),
 (163,"Scorch: Jaeger sprengen","DialogDestroyFighters","FighterDemoChargeUnderway","Fighter01Destroyed",True),
 (184,"Scorch: Kisten sprengen","DestroyCrates","BarricadeDemoUnderway","BarricadeDestroyed",True)]:
 interactions.append(dict(original=n,actor=by[props(by[n]).get("PropToDestroy",{}).get("index",n)]["name"],label=label,position=point(n),prerequisite=pre,underway=under,event=event,demolition=demo,seconds=3. if demo else 6.))
gates=[]
def gate(n,event,offset=None):gates.append(dict(original=n,actor=by[n]["name"],event=event,snapshot=[],offset=offset or [0.,0.,0.]))
for n in [538]:gate(n,"DoorTo40Destroyed")
for n in [378]:gate(n,"UnblockEntranceShield")
for n in [200,201,202,206]:gate(n,"HangarDoorHacked",props(by[n]).get("KeyPos"))
for n in [165,313,314,315,381,520,312]:gate(n,"Fighter01Destroyed")
for n in [186,189,298,94,299,300,190]:gate(n,"BarricadeDestroyed")
for n in [60,90,155,384,422,891,976]:
 d=props(by[n]);event="OpenDoor."+str(n)
 marker=by[d["MyMarker"]["index"]]
 triggers.append(dict(original=marker["original"],event=event,position=point(marker["original"]),radius=max(180.,val(marker,"CollisionRadius",40.)),height=160.))
 gate(n,event,d["KeyPos"])
 actions=[dict(original=n,op="wait",text=event,number=0,seconds=0.)]
 if d.get("OpeningEvent"):actions.append(dict(original=n,op="emit",text=d["OpeningEvent"],number=0,seconds=0.))
 scripts.append(dict(original=n,actions=actions))
# Original door release triggers SpawnHangar01; hangar02 factories are entered via original mover.
scripts.append(dict(original=200,actions=[dict(original=2569,op="wait",text="HangarDoorHacked",number=0,seconds=0.),dict(original=200,op="emit",text="SpawnHangar01",number=0,seconds=0.)]))
# Hangar02 entrance mover's OpeningEvent is carried above; retain all original declarations.
objectives=[next(x["value"]["value"] for x in r["values"] if x["name"]=="Objective") for r in by[142]["nested"][0]["struct_array"]["Ok"]]
a=dict(targets={val(r,"Tag"):point(r["original"]) for r in rows if val(r,"Location") and val(r,"Tag")},squad_fire_sound=0,demolition_sound=0,objectives=objectives,scripts=scripts,triggers=triggers,factories=factories,counters=[dict(original=r["original"],tag=val(r,"Tag"),event=val(r,"Event"),count=props(r)["NumToCount"]) for r in rows if r["class"]=="Gameplay.Counter"],interactions=interactions,gates=gates,
 stations=[point(r["original"]) for r in rows if "BactaDispenser" in r["class"]],
 ammunition=[point(r["original"]) for r in rows if "BlasterBox" in r["class"]],
 fixer_position=point(10),scorch_position=point(11),exit=point(567),exit_radius=val(by[567],"CollisionRadius",600.),destination="GEO_01C.ctm#playerstart",droid_mesh=None,warrior_mesh=None,fixer_mesh=None)
(root/"gameplay-spec.json").write_text(json.dumps(a,indent=2))
(root/"script-extraction-receipt.json").write_text(json.dumps(dict(scripts=len(scripts),actions=sum(len(s["actions"]) for s in scripts),factories=len(factories),sounds=sounds,omitted=omitted,policy="Selected one-shot event host; 3s demolition/6s hold slice, clear-floor factory spawning, bounded combat. Omitted animation/priority/HurtActor/FX actions are explicit; physical release bindings retained separately."),indent=2))
print(len(scripts),"scripts",len(factories),"factories",len(sounds),"voices",len(gates),"gates")
