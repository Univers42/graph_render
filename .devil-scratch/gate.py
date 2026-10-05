from sim import simulate, check, MAX
import random
# Reproduce the HASH GATE model: seeded_model -> all edges UNDIRECTED (kind != Relation),
# all versions 0.0, preferential attachment, degree 8. Rows fall back to index order.
# Question 5: how wide does the drawing get on that model?
def gate_graph(n, seed):
    rnd = random.Random(12345 ^ seed)
    # preferential attachment approximation of synthetic_edges
    edges=[]
    targets=list(range(min(3,n)))
    repeated=list(range(min(3,n)))
    for v in range(3, n):
        chosen=set()
        for _ in range(2):
            chosen.add(repeated[rnd.randrange(len(repeated))])
        for t in chosen:
            edges.append((v,t,False))   # UNDIRECTED
        repeated.extend([v]*2); repeated.extend(chosen)
    return edges
print("seed  n     width  width/n")
for n in (50,100,200,400,601):
    w=0
    for seed in range(8):
        e=gate_graph(n,seed)
        version=[0.0]*n
        row,lane,carried=simulate(n,e,version)
        assert check(n,e,version) is None, "invariant broken on the gate model!"
        w=max(w, max(lane)+1)
    print("%-5d %-5d %-6d %.2f"%(seed,n,w,w/n))
