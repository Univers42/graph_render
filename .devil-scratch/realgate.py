# Faithful port of synthetic_records + remix + apply_degree_weights_against's edge effect,
# to measure what layout.dag.lanes will actually be handed by the hash gate.
import math
M=0xFFFFFFFF
class R:
    def __init__(s,seed): s.s=seed&M
    def f64(s):
        s.s=(s.s+0x6D2B79F5)&M
        a=s.s
        t=((a^(a>>15))*(1|a))&M
        t=((t+((t^(t>>7))*(61|t)))&M)^t
        return ((t^(t>>14))&M)/4294967296.0
    def pick(s,n): return math.floor(s.f64()*n)
SEED=0x9E3779B9
def synthetic_records(count):
    rnd=R(SEED)
    nodes=[dict(version=0.0) for _ in range(count)]
    edges=[]
    def push(a,b,kind):
        if a==b: return
        edges.append([a,b,kind])   # kind 0=Relation(directed), 3=NoteLink(undirected)
    def earlier(i):
        x=rnd.f64(); y=rnd.f64()
        return math.floor(x*y*i)
    for i in range(1,count):
        push(i,earlier(i),0)
        if rnd.f64()<0.5: push(i,earlier(i),0)
    extras=math.floor(count*0.05)
    for _ in range(extras):
        push(rnd.pick(count),rnd.pick(count),3)
    return nodes,edges
def remix(seed,edges):
    rnd=R(seed)
    for e in edges:
        k=rnd.pick(5)          # EdgeKind::ALL[rnd.pick(5)] -- redraws KIND only
        e[2]=k
    return edges
KIND_DIR={0:True,1:False,2:False,3:False,4:False}  # directed: kind == Relation
import sys
sys.path.insert(0,'.')
from sim import simulate, check
print("n      m     directed  width  width/n  reversed_notes")
for count in (50,100,200,400,601):
    nodes,edges=synthetic_records(count)
    edges=remix(3,edges)
    d=[(a,b,KIND_DIR[k]) for (a,b,k) in edges]
    nd=sum(1 for e in d if e[2])
    row,lane,carried=simulate(count,d,[0.0]*count)
    bad=check(count,d,[0.0]*count)
    notes=sum(1 for i,(a,b,dd) in enumerate(d) if dd and a!=b and row[a]>row[b])
    w=max(lane)+1
    print("%-6d %-5d %-9d %-6d %-8.2f %d  %s"%(count,len(d),nd,w,w/count,notes,"INVARIANT BROKEN" if bad else "ok"))
