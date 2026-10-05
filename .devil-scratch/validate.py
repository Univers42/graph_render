from sim import simulate, check, MAX
def geom(n, edges, version):
    row,lane,carried=simulate(n,edges,version)
    x=[float(l) for l in lane]; y=[float(r) for r in row]
    offs=[0]; pts=[]
    for e,(s,t,d) in enumerate(edges):
        if s!=t:
            rs,rt=row[s],row[t]
            early,late=(s,t) if rs<rt else (t,s)
            lx=float(carried[e]); p=[]
            if carried[e]!=lane[early]: p.append((lx, float(min(rs,rt))+0.5))
            if carried[e]!=lane[late]:  p.append((lx, float(max(rs,rt))-0.5))
            if early!=s: p.reverse()
            for a,b in p: pts += [a,b]
        offs.append(len(pts)//2)
    notes=[i for i,(s,t,d) in enumerate(edges) if d and s!=t and row[s]>row[t]]
    return x,y,offs,pts,notes

def show(name, n, edges, version, exp):
    x,y,offs,pts,notes=geom(n,edges,version)
    got=dict(x=x,y=y,offs=offs,pts=pts,notes=notes)
    ok=all(got[k]==v for k,v in exp.items())
    print(("PASS " if ok else "FAIL ")+name)
    if not ok:
        for k,v in exp.items(): print("    %-5s want %-40s got %s"%(k,v,got[k]))
    return ok

V=lambda **kw:[float(x) for x in kw.get('v',[])]
allok=True
# a_chain_is_one_lane_and_one_row_per_vertex
allok &= show("a_chain", 4, [(0,1,True),(1,2,True),(2,3,True)], [4.,3.,2.,1.],
    dict(x=[0.,0.,0.,0.], y=[0.,1.,2.,3.], offs=[0,0,0,0], notes=[]))
# a_branch_and_its_merge_take_two_lanes : m merges b (first) and c; nodes a,b,c,m
# node order given as [a,b,c,m]; arcs mb, mc, ba, ca with versions a=1,b=2,c=3,m=4
allok &= show("a_branch_merge", 4, [(3,1,True),(3,2,True),(1,0,True),(2,0,True)], [1.,2.,3.,4.],
    dict(x=[0.,0.,1.,0.], y=[3.,2.,1.,0.], offs=[0,0,1,1,2], pts=[1.,0.5,1.,2.5], notes=[]))
# a_directed_cycle_is_broken_at_the_lowest_index_and_noted
allok &= show("cycle", 3, [(0,1,True),(1,2,True),(2,0,True)], [0.,0.,0.],
    dict(x=[0.,0.,0.], y=[0.,1.,2.], offs=[0,0,0,2], pts=[1.,1.5,1.,0.5], notes=[2]))
# parallel_and_undirected_edges_are_routed_without_notes : a=1,b=2 -> b newer, row b=0
allok &= show("parallel_undirected", 2, [(0,1,False),(1,0,True),(1,0,True)], [1.,2.],
    dict(y=[1.,0.], notes=[]))
print()
print("offs for parallel_undirected:", geom(2,[(0,1,False),(1,0,True),(1,0,True)],[1.,2.])[2])
print()
print("ALL PLAN ASSERTIONS REPRODUCED:", bool(allok))
