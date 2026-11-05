#!/usr/bin/env python3
# usage: bench.py LABEL -- cmd...   prints JSON line: wall_s, cpu_s (user+sys of children), maxrss_mb
import sys, time, subprocess, resource, json, os
i=sys.argv.index('--'); label=sys.argv[1]; cmd=sys.argv[i+1:]
t=time.perf_counter(); p=subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT); w=time.perf_counter()-t
r=resource.getrusage(resource.RUSAGE_CHILDREN)
print(json.dumps({'label':label,'rc':p.returncode,'wall_s':round(w,3),'cpu_s':round(r.ru_utime+r.ru_stime,3),'maxrss_mb':round(r.ru_maxrss/1024,1)}))
sys.stderr.write(p.stdout.decode(errors='replace')[-3000:])
