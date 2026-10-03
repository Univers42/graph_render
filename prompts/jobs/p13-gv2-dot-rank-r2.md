# Job p13-gv2-dot-rank, round 2 (same session, same worktree): finish the rank pass

Your round 1 ended `status: partial` after a client timeout (rc 124). Its last line found a bug: "the
interior-node branch hardcoded the wrong sign". `rank_tests.rs` has an uncommitted change.

Decision on your node-width question: your recommendation is accepted. Leave `max(0.75 in, text + 2*0.11 in)` in this
job. The width only feeds the x-coordinate simplex, so `p13-gv2-dot-position` corrects it against the
oracle using your measured fit, `node = 1.37952 * label_box + 0.30669` in, which is the ellipse circumscribing the
label box plus the default margin. Write that fit and its four oracle rows into
`docs/measurements/p13-gv2-dot.md` under a "for dot-position" heading, so that job starts from it.

Finish what round 1 listed as not done:
1. The rank pass itself: network simplex ranking, matching the oracle's `-Tplain` ranks. Fix the interior-node sign bug
   first, and add a test that would have caught it.
2. The six closed-case rank tests and the 20 fixture-seed rank tests.
3. The 1000-seed rank agreement against the Graphviz oracle. Paste the count and the exact command, and
   write the number into `docs/measurements/p13-gv2-dot.md`. A disagreement is a finding with its
   seed, not a reason to stop.

Done when: quick.rows green, the agreement count pasted with its command, and a commit past 82434c3.
