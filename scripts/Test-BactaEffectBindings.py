"""Check extracted original emitter templates and their ambient binding in the built scene."""
import argparse
import json
import struct
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('original',type=Path)
p.add_argument('scene',type=Path)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
original=json.loads((a.original/'original-bindings.json').read_text())
bases=json.loads((a.original/'emitter-base-defaults.json').read_text())
assert {b['class'] for b in bases}=={'Emitter','ParticleEmitter','SpriteEmitter'}
effects={e['class']:e for e in original['effects']}
assert set(effects)=={'CTEffects.BactaZap_Dispenser','CTEffects.BactaZap_Dispenser_Body'}
rows=[]
for name,expected in [('CTEffects.BactaZap_Dispenser',set(range(757,768))),('CTEffects.BactaZap_Dispenser_Body',set(range(2579,2582)))]:
    effect=effects[name]
    assert effect['parent']=='Engine.Emitter'
    assert {x['original'] for x in effect['emitters']}==expected
    assert any(v['name']=='Emitters' for v in effect['properties']['values'])
    for template in effect['emitters']:
        assert template['class']=='Engine.SpriteEmitter'
        scalars=template['resolved_scalars']
        assert all('Ok' in r for r in scalars.values()), (name,template['original'],scalars)
        assert scalars['MaxParticles']['Ok']['value']['value']>0
        assert scalars['Texture']['Ok']['value']['value']['index']!=0
        for nested in template['nested']:
            value=next(v['value'] for v in template['properties']['values'] if v['name']==nested['name'] and v['array_index']==nested['slot'])
            native_vector=value['type'] in ('Vector','Rotator') and len(nested['raw'])==12
            assert nested['raw'] and (native_vector or 'Ok' in nested['struct'] or 'Ok' in nested['array']), (name,template['original'],nested['name'])
        rows.append(dict(effect=name,original=template['original'],nested_fields=len(template['nested'])))
dispenser=effects['CTEffects.BactaZap_Dispenser']
ambient=dispenser['defaults']['AmbientSound']['Ok']['value']['value']['path']
assert ambient=='GEO_Sound.GEO_03.ele_GEO_genEnrgy_lp_01'
assert effects['CTEffects.BactaZap_Dispenser_Body']['defaults']['AmbientSound']['Ok']['value']['value']['index']==0
raw=a.scene.read_bytes()
assert raw[:8]==b'RCPLAY13'
opening=json.loads(raw[12:12+struct.unpack_from('<I',raw,8)[0]])
role=opening['bacta']['ambient_loop']
binding=next(b for b in opening['sound_bindings'] if b['role']==role)
assert binding['source']==ambient and opening['provenance']['bacta']['ambient']['attachment']=='Eye'
report=dict(passed=True,templates=len(rows),base_classes=len(bases),nested_fields=sum(r['nested_fields'] for r in rows),ambient_source=ambient,ambient_role=role,body_has_no_ambient=True,rows=rows,limits='Original data extraction and audio identity; visual simulation/rendering remains unimplemented')
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='rows'}))
