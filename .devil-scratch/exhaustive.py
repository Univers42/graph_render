from sim import check, MAX
import itertools
def gen(n):
    pairs=[(i,j) for i in range(n) for j in range(n) if i!=j]
    for combo in itertools.product(range(4), repeat=len(pairs)):
        e=[]
        for (i,j),st in zip(pairs,combo):
            if st==1: e.append((i,j,False))
            elif st==2: e.append((i,j,True))
            elif st==3: e.append((j,i,True))
        yield e
total=0; bad=0
for n in (2,3,4):
    for e in gen(n):
        version=[float(n-i) for i in range(n)]
        total+=1
        r=check(n,e,version)
        if r:
            print("FAIL",r); bad+=1; break
    print("n=%d done, cumulative %d graphs, bad=%d"%(n,total,bad))
    if bad: break
print("TOTAL",total,"BAD",bad)
