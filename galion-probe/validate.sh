#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. Validate a probe JSON against schema/probe-result.schema.json.
# std-lib python only (no jsonschema dep): checks required keys, types, enums, and internal consistency.
# Usage: ./validate.sh probe-<host>-<ts>.json
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
python3 - "$HERE/schema/probe-result.schema.json" "$1" <<'PY'
import json,sys,statistics as st,re
sc=json.load(open(sys.argv[1])); d=json.load(open(sys.argv[2])); err=[]
for k in sc["required"]:
    if k not in d: err.append(f"missing {k}")
T={"integer":int,"number":(int,float),"string":str,"array":list,"object":dict}
for k,p in sc["properties"].items():
    if k not in d: continue
    v=d[k]
    if "type" in p and not isinstance(v,T[p["type"]]): err.append(f"{k}: type"); continue
    if "const" in p and v!=p["const"]: err.append(f"{k}: != {p['const']}")
    if "enum" in p and v not in p["enum"]: err.append(f"{k}: not in {p['enum']}")
    if "pattern" in p and not re.search(p["pattern"],v): err.append(f"{k}: pattern")
    if "minimum" in p and v<p["minimum"]: err.append(f"{k}: < min")
    if "exclusiveMinimum" in p and v<=p["exclusiveMinimum"]: err.append(f"{k}: <= 0")
    if "minItems" in p and len(v)<p["minItems"]: err.append(f"{k}: too few items")
r=d.get("bw_runs_gbs",[])
if len(r)>=2:
    m=st.mean(r); cv=100*st.stdev(r)/m
    if abs(m-d["bw_mean_gbs"])>0.02: err.append("bw_mean_gbs inconsistent with runs")
    if abs(cv-d["bw_cv_pct"])>0.05: err.append("bw_cv_pct inconsistent with runs")
    t=d["thresholds"]; exp="PASS" if cv<t["bw_cv_max_pct"] and m>=t["bw_min_gbs_nt"] else "FAIL"
    if exp!=d["g0"]: err.append(f"g0={d['g0']} but thresholds imply {exp}")
for e in err: print("SCHEMA FAIL:",e)
print("SCHEMA OK" if not err else f"{len(err)} error(s)"); sys.exit(1 if err else 0)
PY
