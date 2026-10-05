import struct
def f32(x): return struct.unpack('f',struct.pack('f',x))[0]
CEIL=9_200_000
print("Can a node y (r*rs) equal a BEND y ((r+0.5)*rs) in f32, for r <= ceiling?")
print("i.e. can a polyline's half-row offset land exactly on a vertex row?")
for rs in (1.0,0.0625,3.0,7.0,16.0,100.0,1024.0):
    fs=f32(rs)
    hits=[]
    # node rows are r*rs; bend rows are (r+0.5)*rs. Test the equality
    # (r+0.5)*rs == r2*rs  <=>  f32 values collide
    for r in range(0, CEIL, max(1,CEIL//20000)):   # 20k probes across the range
        b=f32(f32(float(r)+0.5)*fs)
        # nearest node rows: r and r+1
        for r2 in (r,r+1):
            if f32(f32(float(r2))*fs)==b: hits.append((r,r2,b))
        if len(hits)>=3: break
    print("  rs=%-8g collisions found: %s"%(rs,hits[:3] if hits else "none in 20k probes"))
print()
print("And the reverse: does the ROUND-ROBIN of f32 make (r+0.5)*rs == (r'+0.5)*rs for r != r'?")
for rs in (3.0,7.0,100.0):
    fs=f32(rs); seen={}; dup=None
    for r in range(0, 2_000_000):
        b=f32(f32(float(r)+0.5)*fs)
        if b in seen: dup=(seen[b],r,b); break
        seen[b]=r
    print("  rs=%-8g first duplicate bend row: %s"%(rs,dup if dup else "none in 2e6 rows"))
