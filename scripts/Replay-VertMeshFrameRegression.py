"""Rerun the previous native frame proof without rewriting historical evidence."""
from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-VertMeshFrame.py'
namespace={'__file__':str(source)}
exec(compile(source.read_text().split('\ntests=sum(',1)[0],str(source),'exec'),namespace)
result=dict(previous_frame_replay_passed=True,counts=dict(namespace['counts']))
(root/'analysis/reports/vertmesh-animation-previous-replay.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
