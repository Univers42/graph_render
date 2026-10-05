import heapq, random, itertools, sys
MAX=0xFFFFFFFF

def simulate(n, edges, version):
    """edges: list of (s,t,directed) in admission order. Returns (row, lane, carried) or raises."""
    # ---- rows.rs ----
    arcs=[(s,t) for (s,t,d) in edges if d and s!=t]
    offs=[0]*(n+1); pend=[0]*n
    for (s,t) in arcs:
        offs[s+1]+=1; pend[t]+=1
    for v in range(n): offs[v+1]+=offs[v]
    fill=offs[:]; tgts=[0]*offs[n]
    for (s,t) in arcs:
        tgts[fill[s]]=t; fill[s]+=1
    heap=[]  # max-heap on (version, -vertex)
    def push(v): heapq.heappush(heap,(-version[v], v))
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
            w=tgts[k]
            if pend[w]==0: raise AssertionError("pending underflow at %d"%w)
            pend[w]-=1
            if pend[w]==0 and row[w]==MAX: push(w)
    # ---- assign.rs ----
    kept=[i for i,(s,t,d) in enumerate(edges) if s!=t]
    def ends(i):
        s,t,_=edges[i]
        return (s,t) if row[s]<row[t] else (t,s)
    foffs=[0]*(n+1)
    for i in kept: foffs[ends(i)[0]+1]+=1
    for v in range(n): foffs[v+1]+=foffs[v]
    ffill=foffs[:]; fe=[(0,0)]*foffs[n]
    for i in kept:
        a,b=ends(i); fe[ffill[a]]=(i,b); ffill[a]+=1
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
        keep=mn[v]; l=head[v]; seen=set()
        while l!=MAX:
            if l in seen: raise AssertionError("CYCLE in reserved list of v=%d lane=%d"%(v,l))
            seen.add(l)
            nx=nxt[l]
            if l!=keep: give(l)
            l=nx
        if len(seen)!=len(set(seen)): raise AssertionError("dup")
        head[v]=MAX; mn[v]=MAX; return keep
    lane=[MAX]*n; carried=[MAX]*len(edges)
    for v in order:
        out=fe[foffs[v]:foffs[v+1]]
        lv=rsettle(v)
        if lv==MAX: lv=take()
        lane[v]=lv
        for k,(e,late) in enumerate(out):
            if k==0:
                rpush(late,lv); carried[e]=lv
            else:
                sh=mn[late]
                if sh!=MAX: carried[e]=sh
                else:
                    f=take(); rpush(late,f); carried[e]=f
        if not out: give(lv)
        # internal invariant: pool must hold no duplicate lane
        if len(free)!=len(set(free)): raise AssertionError("DUPLICATE lane in free pool: %r"%(sorted(free),))
        # internal invariant: a lane in a reservation must not also be in the pool
        reserved=set()
        for u in range(n):
            l=head[u]
            while l!=MAX: reserved.add(l); l=nxt[l]
        clash=reserved & set(free)
        if clash: raise AssertionError("LANE IN BOTH reservation and free pool: %r"%(sorted(clash),))
    return row,lane,carried

def check(n, edges, version):
    row,lane,carried=simulate(n,edges,version)
    pos={}
    for v in range(n): pos[(lane[v],row[v])]=v
    for i,(s,t,d) in enumerate(edges):
        if s==t: continue
        lo,hi=min(row[s],row[t]),max(row[s],row[t])
        l=carried[i]
        for r in range(lo+1,hi):
            if (l,r) in pos:
                return ("VERTEX ON EDGE", dict(n=n,edges=edges,version=version,i=i,lane=lane,row=row,carried=carried,
                        victim=pos[(l,r)],lane_l=l,row_r=r))
    # geometry must be monotone in y per polyline half
    return None
