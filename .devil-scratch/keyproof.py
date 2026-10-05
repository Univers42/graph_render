import struct, random, itertools
def tob(x): return struct.unpack('q',struct.pack('d',x))[0]
def total_key(x):                      # the PLAN's oracle key, verbatim semantics
    b=tob(x)
    sh = -1 if b<0 else 0              # Rust `bits >> 63` on i64 is an ARITHMETIC shift
    m = (sh & 0xFFFFFFFFFFFFFFFF) >> 1 # `as u64` then `>> 1`
    r = b ^ m
    return r-(1<<64) if r>=(1<<63) else r
# Rust's own total_cmp, per std docs, IS this same transform. Verify the three
# properties std documents, plus monotonicity over the whole ordered float line.
vals=[0.0,-0.0,1.0,-1.0,2.0,-2.0,0.5,-0.5,float('inf'),float('-inf'),float('nan'),
      -float('nan'),5e-324,-5e-324,1e-320,-1e-320,3.0,-3.0,1e300,-1e300]
print("std-documented properties:")
print("  -0.0 < 0.0      :", total_key(-0.0) < total_key(0.0))
print("  -inf < -1 < -0  :", total_key(float('-inf')) < total_key(-1.0) < total_key(-0.0))
print("  0 < 5e-324 < 1  :", total_key(0.0) < total_key(5e-324) < total_key(1.0))
print("  inf < +NaN      :", total_key(float('inf')) < total_key(float('nan')))
print("  -NaN < -inf     :", total_key(float('-nan')) < total_key(float('-inf')))
# Monotonicity: for every pair, sign(total_key(a)-total_key(b)) must equal the
# IEEE-754 totalOrder predicate. Build a reference totalOrder independently:
def ref_total_cmp(a,b):
    ba,bb=tob(a),tob(b)
    if ba<0: ba = -(ba & 0x7FFFFFFFFFFFFFFF)      # a DIFFERENT formulation of totalOrder
    if bb<0: bb = -(bb & 0x7FFFFFFFFFFFFFFF)
    return (ba>bb)-(ba<bb)
random.seed(5)
pat=[struct.unpack('d',struct.pack('Q',random.getrandbits(64)))[0] for _ in range(60000)]
pat+=vals+[0.0]*50
bad=0
for a in pat:
    for b in random.sample(pat,40):
        want=ref_total_cmp(a,b)
        got=(total_key(a)>total_key(b))-(total_key(a)<total_key(b))
        if want!=got: bad+=1
print()
print("disagreements with an INDEPENDENT totalOrder reference over %d comparisons: %d"%(len(pat)*40,bad))
print()
print("=> the plan's total_key is order-isomorphic to f64::total_cmp" if bad==0 else "=> MISMATCH")
