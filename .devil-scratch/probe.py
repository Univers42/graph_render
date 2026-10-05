from sim import simulate, check, MAX
import random
# 1) does settle() ever return a lane that is simultaneously min for two vertices,
#    or that the caller is about to reuse while an edge still carries it?
# The suspicion: `settle` frees non-keep lanes, and the KEEP lane stays reserved for v
# until v's row. But carry(k==0) pushes `lane` into reserved[late] AFTER settle — fine.
# The real hole: a lane freed by settle(v) at row(v) may still be the carried lane of an
# edge whose LATER endpoint is in a row > row(v)?  settle only frees lanes in reserved[v],
# whose edges all END at v. So safe. Probe empirically anyway.
random.seed(7)
stats={"width":[]}
worst=0; worstcase=None
for trial in range(200000):
    n=random.randint(2,7)
    m=random.randint(0,14)
    e=[]
    for _ in range(m):
        s=random.randrange(n); t=random.randrange(n)
        e.append((s,t,random.random()<0.5))
    version=[random.choice([0.0,0.0,float(random.randrange(4))]) for _ in range(n)]
    r=check(n,e,version)
    if r:
        print("FAIL random",r); break
    row,lane,carried=simulate(n,e,version)
    w=max(lane)+1
    stats["width"].append(w/n)
    if w/n>worst: worst=w/n; worstcase=(n,e,version,w)
print("no failure in 200k random graphs up to n=7")
print("max width/n = %.2f  (n=%d, width=%d)"%(worst,worstcase[0],worstcase[3]))
