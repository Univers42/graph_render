import random
from sim import simulate, check, MAX
# Targeted adversary: build merge-heavy DAGs (fan-in up to 50), parallel edges, mixed
# directedness, and chains that force lane reuse. Attack the ONE-ROW-ONE-LANE invariant.
random.seed(99)
def build(n, fanin, npar, seed):
    r=random.Random(seed)
    e=[]
    # a chain backbone so rows are spread
    for i in range(1,n): e.append((i,i-1,True))         # directed arcs point "up"
    # merges: many sources into one target
    for t in range(0,n,max(1,n//12)):
        for _ in range(fanin):
            e.append((r.randrange(n),t,r.random()<0.7))
    # parallel edges
    for _ in range(npar):
        a,b=r.randrange(n),r.randrange(n)
        e.append((a,b,True)); e.append((a,b,False))
    return e
worst=0; bad=0; tested=0
for seed in range(4000):
    n=random.Random(seed).randint(4,10)
    e=build(n, random.Random(seed+1).randint(1,8), random.Random(seed+2).randint(0,4), seed)
    ver=[float((i*7)%5) for i in range(n)]
    tested+=1
    try:
        row,lane,carried=simulate(n,e,ver)
    except AssertionError as ex:
        print("INTERNAL INVARIANT BREAK seed",seed,ex); bad=1; break
    r=check(n,e,ver)
    if r: print("NODE-ON-EDGE seed",seed,r); bad=1; break
    w=max(lane)+1; worst=max(worst,w/n)
print("adversarial graphs tested:",tested,"bad:",bad,"max width/n: %.2f"%worst)

# And the plan's OWN history() generator, verbatim, at larger n.
def history(n, seed):
    state=seed
    def nxt(bound):
        nonlocal state
        state=(state*6364136223846793005+1442695040888963407)&0xFFFFFFFFFFFFFFFF
        return ((state>>33)%max(bound,1))
    ids=["v%d"%i for i in range(n)]
    nodes=[(i,float(n-i)) for i in range(n)]
    edges=[]
    for i in range(0,max(0,n-1)):
        parents = 50 if i%97==0 else 1+nxt(2)
        for k in range(parents):
            p=min(i+1+nxt(40), n-1)
            edges.append((i,p,True))
    return nodes,edges
bad2=0
for seed in range(1,40):
    nodes,edges=history(3000,seed)
    ver=[v for _,v in nodes]
    bad2 = check(len(nodes),edges,ver) is not None
    if bad2: print("history() INVARIANT BROKEN at seed",seed); break
    row,lane,carried=simulate(len(nodes),edges,ver)
    if seed==1: print("history n=3000 seed1: width=%d (%.2f n), notes=%d"%(max(lane)+1,(max(lane)+1)/3000,
          sum(1 for i,(a,b,d) in enumerate(edges) if d and row[a]>row[b])))
print("plan history() invariant holds for seeds 1..39:", not bad2)
