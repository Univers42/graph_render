# Does "a lane is in at most one reservation at a time" (assign.rs:554-555) actually hold?
# That is what makes `next: Vec<u32>` indexed BY LANE safe. Test it directly.
import itertools, random
MAX=0xFFFFFFFF
def run(n, edges, version):
    arcs=[(s,t) for (s,t,d) in edges if d and s!=t]
    offs=[0]*(n+1); pend=[0]*n
    for (s,t) in arcs: offs[s+1]+=1; pend[t]+=1
    for v in range(n): offs[v+1]+=offs[v]
    fill=offs[:]; tg=[0]*offs[n]
    for (s,t) in arcs: tg[fill[s]]=t; fill[s]+=1
    import heapq
    heap=[]
    def push(v): heapq.heappush(heap,(-version[v],v))
    for v in range(n):
        if pend[v]==0: push(v)
    row=[MAX]*n; order=[]; scan=0
    while len(order)<n:
        if heap: v=heapq.heappop(heap)[1]
        else:
            while row[scan]!=MAX: scan+=1
            v=scan
        row[v]=len(order); order.append(v)
        for k in range(offs[v],offs[v+1]):
            w=tg[k]; pend[w]-=1
            if pend[w]==0 and row[w]==MAX: push(w)
    kept=[i for i,(s,t,d) in enumerate(edges) if s!=t]
    def ends(i):
        s,t,_=edges[i]; return (s,t) if row[s]<row[t] else (t,s)
    fo=[0]*(n+1)
    for i in kept: fo[ends(i)[0]+1]+=1
    for v in range(n): fo[v+1]+=fo[v]
    ff=fo[:]; fe=[(0,0)]*fo[n]
    for i in kept:
        a,b=ends(i); fe[ff[a]]=(i,b); ff[a]+=1
    free=[]; width=0
    def take():
        nonlocal width
        if free: return heapq.heappop(free)
        width+=1; return width-1
    def give(l): heapq.heappush(free,l)
    head=[MAX]*n; nxt=[]; mn=[MAX]*n
    def rpush(v,l):
        while len(nxt)<=l: nxt.append(MAX)
        nxt[l]=head[v]; head[v]=l; mn[v]=min(mn[v],l)
    def rsettle(v):
        keep=mn[v]; l=head[v]
        while l!=MAX:
            nx=nxt[l]
            if l!=keep: give(l)
            l=nx
        head[v]=MAX; mn[v]=MAX; return keep
    lane=[MAX]*n; carried=[MAX]*len(edges)
    owner={}   # lane -> the vertex whose list holds it
    for v in order:
        out=fe[fo[v]:fo[v+1]]
        lv=rsettle(v)
        if lv==MAX: lv=take()
        lane[v]=lv
        for k,(e,late) in enumerate(out):
            if k==0:
                # push may collide with an existing member of reserved[late]
                l=head[late]; seen=[]
                while l!=MAX: seen.append(l); l=nxt[l]
                if lv in seen:
                    raise AssertionError("PUSH COLLISION lane %d already in reserved[%d]"%(lv,late))
                rpush(late,lv); carried[e]=lv
            else:
                sh=mn[late]
                if sh!=MAX: carried[e]=sh
                else:
                    f=take(); rpush(late,f); carried[e]=f
        if not out: give(lv)
        # EXPLICIT: lane in at most one list, and a reserved lane is never in the pool
        memb={}
        for u in range(n):
            l=head[u]
            while l!=MAX:
                if l in memb:
                    raise AssertionError("LANE IN TWO LISTS: %d in reserved[%d] and reserved[%d]"%(l,memb[l],u))
                memb[l]=u; l=nxt[l]
        if memb.keys() & set(free):
            raise AssertionError("RESERVED LANE ALSO FREE: %r"%(sorted(memb.keys()&set(free)),))
        if len(free)!=len(set(free)):
            raise AssertionError("DUPLICATE IN POOL")
    return row,lane,carried,width

bad=0; cnt=0
for n in (2,3,4):
    pairs=[(i,j) for i in range(n) for j in range(n) if i!=j]
    for combo in itertools.product(range(4),repeat=len(pairs)):
        e=[]
        for (i,j),st in zip(pairs,combo):
            if st==1: e.append((i,j,False))
            elif st==2: e.append((i,j,True))
            elif st==3: e.append((j,i,True))
        cnt+=1
        try: run(n,e,[float(n-i) for i in range(n)])
        except AssertionError as ex:
            print("FAIL n=%d %s"%(n,ex)); print("  edges:",e); bad=1; break
    if bad: break
    print("n=%d exhaustive done (%d graphs)"%(n,cnt),flush=True)
print("single-reservation invariant:", "HOLDS" if not bad else "BROKEN", "over",cnt,"graphs")
