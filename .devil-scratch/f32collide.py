import struct
def f32(x): return struct.unpack('f',struct.pack('f',x))[0]
CEIL=9_200_000   # TOPOLOGY_CEILING, crates/graph-cli/src/capabilities/registry.rs:24
# tunable! ranges: 0.0625 .. 1024.0, default 1.0
SPACINGS=[0.0625,0.125,0.25,0.5,1.0,2.0,3.0,7.0,16.0,100.0,1024.0]
print("Does x = lane*ls or y = row*rs collapse two DISTINCT indices onto one f32,")
print("for lane,row up to TOPOLOGY_CEILING=%d ?"%CEIL)
print()
print("%-9s %-28s %-28s"%("spacing","x: first colliding lane","y: first colliding row"))
for s in SPACINGS:
    fs=f32(s)
    # x: adjacent-index collision
    fx=None
    prev=f32(f32(0.0)*fs)
    lo,hi=1,CEIL
    # binary search on the monotone predicate
    def coll(i):
        return f32(f32(float(i))*fs)==f32(f32(float(i-1))*fs)
    if coll(hi): 
        a,b=1,hi
        while a<b:
            m=(a+b)//2
            if coll(m): b=m
            else: a=m+1
        fx=a
    fy=None
    def colr(i):
        return f32(f32(float(i))*fs)==f32(f32(float(i-1))*fs)
    if colr(hi):
        a,b=1,hi
        while a<b:
            m=(a+b)//2
            if colr(m): b=m
            else: a=m+1
        fy=a
    print("%-9g %-28s %-28s"%(s, fx if fx else "none <= ceiling",
                                   fy if fy else "none <= ceiling"))
