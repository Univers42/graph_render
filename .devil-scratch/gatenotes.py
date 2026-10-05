import math
M=0xFFFFFFFF
class R:
    def __init__(s,seed): s.s=seed&M
    def f64(s):
        s.s=(s.s+0x6D2B79F5)&M; a=s.s
        t=((a^(a>>15))*(1|a))&M
        t=((t+((t^(t>>7))*(61|t)))&M)^t
        return ((t^(t>>14))&M)/4294967296.0
    def pick(s,n): return math.floor(s.f64()*n)
def records(count):
    rnd=R(0x9E3779B9); edges=[]
    def push(a,b,k):
        if a!=b: edges.append([a,b,k])
    def earlier(i):
        x=rnd.f64(); y=rnd.f64(); return math.floor(x*y*i)
    for i in range(1,count):
        push(i,earlier(i),0)
        if rnd.f64()<0.5: push(i,earlier(i),0)
    for _ in range(math.floor(count*0.05)):
        push(rnd.pick(count),rnd.pick(count),3)
    return edges
import sys; sys.path.insert(0,'.')
from sim import simulate
DIR={0:True,1:False,2:False,3:False,4:False}
print("seed n=601: directed edges, reversed (note 5) edges, width")
tot_d=0; tot_r=0
for seed in range(20):
    e=records(601)
    rnd=R(seed)
    for x in e: x[2]=rnd.pick(5)
    d=[(a,b,DIR[k]) for a,b,k in e]
    row,lane,carried=simulate(601,d,[0.0]*601)
    nd=sum(1 for x in d if x[2]); nr=sum(1 for i,(a,b,dd) in enumerate(d) if dd and a!=b and row[a]>row[b])
    tot_d+=nd; tot_r+=nr
    if seed<6: print("  %-3d %-5d %-6d %.2f"%(seed,nd,nr,(max(lane)+1)/601))
print("over 20 gate seeds: total directed=%d, total note-5 (reversed)=%d"%(tot_d,tot_r))
