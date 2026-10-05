import struct, random
from sim import simulate, MAX
def f32(x): return struct.unpack('f',struct.pack('f',x))[0]
def render(n, edges, version, ls=1.0, rs=1.0):
    row,lane,carried=simulate(n,edges,version)
    X=[f32(f32(float(l))*f32(ls)) for l in lane]
    Y=[f32(f32(float(r))*f32(rs)) for r in row]
    out=[]
    for e,(s,t,d) in enumerate(edges):
        if s==t: out.append([(X[s],Y[s])]); continue
        rs_,rt_=row[s],row[t]
        early,late=(s,t) if rs_<rt_ else (t,s)
        cx=f32(f32(float(carried[e]))*f32(ls)); pts=[]
        if carried[e]!=lane[early]: pts.append((cx, f32(f32(float(min(rs_,rt_))+0.5)*f32(rs))))
        if carried[e]!=lane[late]:  pts.append((cx, f32(f32(float(max(rs_,rt_))-0.5)*f32(rs))))
        if early!=s: pts.reverse()
        chain=[(X[s],Y[s])]+pts+[(X[t],Y[t])]
        out.append(chain)
    return row,lane,carried,X,Y,out

def on_seg(px,py,ax,ay,bx,by):
    # exact-f32 collinearity + within bounding box
    if px==ax and py==ay: return False   # endpoint is the edge's own node
    if px==bx and py==by: return False
    cross=(f32(bx-ax)*f32(py-ay))-(f32(by-ay)*f32(px-ax))
    if cross!=0.0: return False
    return (min(ax,bx)<=px<=max(ax,bx)) and (min(ay,by)<=py<=max(ay,by))

def sweep(n, edges, version, ls, rs):
    row,lane,carried,X,Y,chains=render(n,edges,version,ls,rs)
    nodes={}
    for v in range(n): nodes.setdefault((X[v],Y[v]),[]).append(v)
    for e,ch in enumerate(chains):
        for k in range(len(ch)-1):
            ax,ay=ch[k]; bx,by=ch[k+1]
            for (px,py),vs in nodes.items():
                for v in vs:
                    if v==edges[e][0] or v==edges[e][1]: continue
                    if on_seg(px,py,ax,ay,bx,by):
                        return ("NODE ON SEGMENT", dict(n=n,edges=edges,version=version,ls=ls,rs=rs,
                              edge=e, seg=(ax,ay,bx,by), node=(px,py), victim=v,
                              row=row,lane=lane,carried=carried))
    return None

print("--- exhaustive n<=4, all states, unit + non-unit spacings ---")
import itertools
bad=0; cnt=0
for n in (2,3,4):
    pairs=[(i,j) for i in range(n) for j in range(n) if i!=j]
    for combo in itertools.product(range(4),repeat=len(pairs)):
        e=[]
        for (i,j),st in zip(pairs,combo):
            if st==1: e.append((i,j,False))
            elif st==2: e.append((i,j,True))
            elif st==3: e.append((j,i,True))
        for (ls,rs) in ((1.0,1.0),(0.5,2.0),(1024.0,0.0625)):
            cnt+=1
            r=sweep(n,e,[float(n-i) for i in range(n)],ls,rs)
            if r:
                print("FAIL",r); bad=1; break
        if bad: break
    print("n=%d done (%d renders)"%(n,cnt))
    if bad: break
print("exhaustive bad:",bad)

print()
print("--- 200k random n<=8, several spacings ---")
random.seed(11); bad2=0
for t in range(200000):
    n=random.randint(2,8); m=random.randint(0,16)
    e=[(random.randrange(n),random.randrange(n),random.random()<0.5) for _ in range(m)]
    ver=[random.choice([0.0,0.0,float(random.randrange(5))]) for _ in range(n)]
    ls,rs=random.choice([(1.0,1.0),(0.0625,1024.0),(1024.0,0.0625),(3.0,7.0),(0.125,0.125)])
    r=sweep(n,e,ver,ls,rs)
    if r: print("FAIL random",r); bad2=1; break
print("random bad:",bad2)
