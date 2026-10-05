import struct, random, itertools
from sim import simulate, MAX
def f32(x): return struct.unpack('f',struct.pack('f',x))[0]
FLOATS={1.0,0.5,1024.0,0.0625,3.0,7.0,0.125,2.0,16.0}
FC={v:f32(v) for v in FLOATS}
def render(n,edges,ver,ls,rs):
    row,lane,carried=simulate(n,edges,ver)
    fls,frs=FC[ls],FC[rs]
    X=[f32(f32(float(l))*fls) for l in lane]; Y=[f32(f32(float(r))*frs) for r in row]
    out=[]
    for e,(s,t,d) in enumerate(edges):
        if s==t: out.append(None); continue
        a,b=row[s],row[t]
        early,late=(s,t) if a<b else (t,s)
        cx=f32(f32(float(carried[e]))*fls); pts=[]
        if carried[e]!=lane[early]: pts.append((cx,f32(f32(float(min(a,b))+0.5)*frs)))
        if carried[e]!=lane[late]:  pts.append((cx,f32(f32(float(max(a,b))-0.5)*frs)))
        if early!=s: pts.reverse()
        out.append([(X[s],Y[s])]+pts+[(X[t],Y[t])])
    return row,lane,carried,X,Y,out
def on_seg(px,py,ax,ay,bx,by):
    if (px==ax and py==ay) or (px==bx and py==by): return False
    if f32(f32(bx-ax)*f32(py-ay))-f32(f32(by-ay)*f32(px-ax))!=0.0: return False
    return min(ax,bx)<=px<=max(ax,bx) and min(ay,by)<=py<=max(ay,by)
def sweep(n,edges,ver,ls,rs):
    row,lane,carried,X,Y,chains=render(n,edges,ver,ls,rs)
    nodes={}
    for v in range(n): nodes.setdefault((X[v],Y[v]),[]).append(v)
    for e,ch in enumerate(chains):
        if ch is None: continue
        s,t,_=edges[e]
        for k in range(len(ch)-1):
            ax,ay=ch[k]; bx,by=ch[k+1]
            for (px,py),vs in nodes.items():
                if (px,py)==(X[s],Y[s]) or (px,py)==(X[t],Y[t]): continue
                if on_seg(px,py,ax,ay,bx,by):
                    return dict(n=n,edges=edges,ls=ls,rs=rs,edge=e,seg=(ax,ay,bx,by),node=(px,py),row=row,lane=lane,carried=carried)
    return None
SP=((1.0,1.0),(0.5,2.0),(1024.0,0.0625),(0.125,0.125),(3.0,7.0))
print("--- exhaustive n<=3, all states x 5 spacings ---",flush=True)
bad=0;cnt=0
for n in (2,3):
    pairs=[(i,j) for i in range(n) for j in range(n) if i!=j]
    for combo in itertools.product(range(4),repeat=len(pairs)):
        e=[]
        for (i,j),st in zip(pairs,combo):
            if st==1: e.append((i,j,False))
            elif st==2: e.append((i,j,True))
            elif st==3: e.append((j,i,True))
        for sp in SP:
            cnt+=1
            r=sweep(n,e,[float(n-i) for i in range(n)],*sp)
            if r: print("FAIL",r,flush=True); bad=1; break
        if bad: break
    if bad: break
print("exhaustive n<=3: %d renders, bad=%d"%(cnt,bad),flush=True)
print("--- 150k random n<=8 ---",flush=True)
random.seed(11); b2=0
for t in range(150000):
    n=random.randint(2,8); m=random.randint(0,16)
    e=[(random.randrange(n),random.randrange(n),random.random()<0.5) for _ in range(m)]
    ver=[random.choice([0.0,0.0,float(random.randrange(5))]) for _ in range(n)]
    r=sweep(n,e,ver,*random.choice(SP))
    if r: print("FAIL random",r,flush=True); b2=1; break
print("random bad:",b2,flush=True)
