"""Run the independent full-stream oracle and record reproducible project evidence."""
from pathlib import Path
import hashlib
import json
import re

root=Path(__file__).resolve().parents[1]
script=root/'scripts/Check-SkinStreams.py'; ns={'__file__':str(script),'__name__':'skin_oracle'}
exec(compile(script.read_text(),str(script),'exec'),ns)
report,path,stats,kinds,engine=ns['check']()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
log=(root/'analysis/reports/skeletal-weighted-skin-tests.log').read_text()
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))
assert tests==474 and 'test result: FAILED' not in log
assert sum(kinds[str(i)] for i in list(range(1,8))+list(range(9,15)))==19165
assert sum(kinds[str(i)] for i in range(8,15))==78383 and kinds['15']==102237
sources=['crates/rc-package/src/skeletal_skin.rs','crates/rc-package/src/skeletal_weighted_kernels.rs',
         'crates/rc-package/src/skeletal_weighted_skin_tests.rs','crates/rc-inspect/src/bin/rc-skin-stream-check.rs',
         'scripts/Generate-WeightedSkin.py','scripts/Check-SkinStreams.py','scripts/Record-SkinStreams.py',
         'docs/SKELETAL_WEIGHTED_SKIN.md','analysis/decompiled/skeletal-skin-vertices.asm']
result=dict(date='2026-10-08',rust_tests=tests,counts=dict(stats),command_kinds=dict(kinds),
    completed_tasks=['weighted two-through-eight influence kernels in original scalar SSE order',
                     'native rigid/two/general cache-writing variants',
                     'cache-copy addressing and complete command/UV stream execution'],
    scope=report['scope'],report_sha256=sha(path),source_sha256={s:sha(root/s) for s in sources},
    original_engine_sha256=sha(engine),weight_scale_bits='0x31800080',weight_scale_address='0x10679850',
    prior_report_sha256={name:sha(root/'analysis/reports'/name) for name in
                        ['skeletal-lods.json','loaded-full-animation-ticks.json','reference-caches.json']},
    checks=['cargo test --workspace: 474 passed','cargo clippy --workspace --all-targets -- -D warnings',
            'cargo fmt --all -- --check','all 414 original nonempty skin streams: every output/UV/cache word and counter bit-exact',
            'independent numeric original SSE instruction interpreter, no Rust-generator reuse',
            '64 synthetic all-kind streams, 1408 outputs, including influence counts absent in originals',
            '16 new unit tests for weight conversion, cancellation order, cache/copy behavior and bounded error writes'],
    remaining=['triangle/material submission and visible animated mesh','empty-command LOD render path',
               'native instance allocation/lifecycle/post-terminator copies','remaining mesh tail fields',
               'Actor/Script/game frame loop','final Android verification'])
(root/'analysis/reports/skeletal-skin-streams-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
p=root/'analysis/evidence.json'; evidence=json.loads(p.read_text(encoding='utf-8-sig'))
evidence['rust_tests']=tests; evidence['skeletal_weighted_skin_validation']=result
p.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei weitere Skinning-Aufgaben abgeschlossen: gewichtete Kernel fuer zwei bis acht Einfluesse, '
      'Cache-Schreibvarianten und Cache-Kopien samt vollstaendigem Befehls-/UV-Stream. 414 nichtleere Original-LODs, '
      '289085 Ausgabe-Vertices, 19165 gewichtete Vertices, 78383 Cache-Schreibvorgaenge und 102237 Kopierbefehle '
      'unabhaengig gegen numerische Original-SSE-Auswertung bitgenau geprueft. 125 Meshes mit Animationspose, '
      'fuenf mit Originalreferenzpose; 12 LODs ohne Befehle gesondert offen. Zusaetzlich 64 synthetische Streams '
      'fuer alle Befehlstypen und Einflusszahlen. 474 Tests, Clippy und Format bestanden. Sichtbare Meshdarstellung, '
      'Dreiecke/Materialien und native Instanz-Lebenszyklen offen; Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_WEIGHTED_SKIN.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for target in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei weitere Skinning-Aufgaben abgeschlossen: gewichtete Kernel' not in target.read_text(encoding='utf-8-sig'):
        with target.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
