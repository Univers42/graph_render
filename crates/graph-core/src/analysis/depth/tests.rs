use super::*;

/// A hand-built stand-in for p3's `layout::hierarchy::Hierarchy`, which is not on this
/// branch's base. It is deliberately *only* the structure [`Roots`] asks for — no
/// repair, no cycle-breaking, no multi-parent drop — so pinning the depth convention
/// here cannot smuggle a second repair into shipped code. At the merge step the tests
/// read `Hierarchy::of(&topology)` and this file loses its reason to exist.
struct Forest {
    node_count: u32,
    children: Vec<Vec<u32>>,
    roots: Vec<u32>,
}

impl Roots for Forest {
    fn node_count(&self) -> u32 {
        self.node_count
    }

    fn roots(&self) -> &[u32] {
        &self.roots
    }

    fn virtual_root(&self) -> Option<u32> {
        (self.roots.len() >= 2).then_some(self.node_count)
    }

    fn children(&self, v: u32) -> &[u32] {
        &self.children[v as usize]
    }
}

/// A `Roots` whose `virtual_root` is whatever the test says it is, so the two ways a
/// source can break the trait's contract are expressible. Both are p3's job to honour;
/// depth only has to react to the breakage rather than to it silently.
struct SaysVirtualRoot(Forest, Option<u32>);

impl Roots for SaysVirtualRoot {
    fn node_count(&self) -> u32 {
        self.0.node_count()
    }

    fn roots(&self) -> &[u32] {
        self.0.roots()
    }

    fn virtual_root(&self) -> Option<u32> {
        self.1
    }

    fn children(&self, v: u32) -> &[u32] {
        self.0.children(v)
    }
}

/// A well-formed tree: `edges` are `(parent, child)`, children listed in the order the
/// edges arrive, and the roots derived the way p3 derives them — every node with no
/// parent, ascending dense index.
fn forest(node_count: u32, edges: &[(u32, u32)]) -> Forest {
    let mut children = vec![Vec::new(); node_count as usize];
    for &(parent, child) in edges {
        children[parent as usize].push(child);
    }
    let mut has_parent = vec![false; node_count as usize];
    for &(_, child) in edges {
        has_parent[child as usize] = true;
    }
    let roots: Vec<u32> = (0..node_count)
        .filter(|&v| !has_parent[v as usize])
        .collect();
    Forest {
        node_count,
        children,
        roots,
    }
}

/// An arbitrary forest, including a partial or cyclic one: explicit children rows and
/// explicit roots, never derived.
fn rows(node_count: u32, children: &[&[u32]], roots: &[u32]) -> Forest {
    let mut filled: Vec<Vec<u32>> = children.iter().map(|row| row.to_vec()).collect();
    filled.resize(node_count as usize, Vec::new());
    Forest {
        node_count,
        children: filled,
        roots: roots.to_vec(),
    }
}

#[test]
fn a_single_root_is_depth_zero_and_the_tree_counts_out_from_there() {
    let f = forest(4, &[(0, 1), (0, 2), (1, 3)]);
    assert_eq!(f.roots(), [0], "one parent per node, so one root");
    let d = bfs_depth(&f);
    assert_eq!(d.levels(), [0, 1, 1, 2]);
    assert_eq!(d.max(), 2);
    assert_eq!(d.of(3), 2, "of() and levels() are the same column");
}

#[test]
fn two_roots_hang_off_the_virtual_root_so_both_sit_at_depth_one() {
    // p3 D-H step 6-7: >= 2 roots are children of one hidden virtual root at index n.
    let f = forest(4, &[(0, 1), (2, 3)]);
    assert_eq!(f.roots(), [0, 2]);
    assert_eq!(f.virtual_root(), Some(4));
    let d = bfs_depth(&f);
    assert_eq!(d.levels(), [1, 2, 1, 2]);
    assert_eq!(d.max(), 2);
    assert_eq!(
        d.levels().len(),
        4,
        "the virtual root is index 4 and gets no column of its own"
    );
}

#[test]
fn an_isolated_node_is_its_own_root_and_sits_at_depth_one() {
    let f = forest(3, &[]);
    assert_eq!(
        f.roots(),
        [0, 1, 2],
        "no hierarchy edge: every node is a root"
    );
    let d = bfs_depth(&f);
    assert_eq!(d.levels(), [1, 1, 1]);
    assert_eq!(d.max(), 1);
}

#[test]
fn the_levels_do_not_depend_on_the_order_the_children_are_listed_in() {
    let a = rows(5, &[&[3, 1, 2], &[4], &[], &[], &[]], &[0]);
    let b = rows(5, &[&[2, 1, 3], &[4], &[], &[], &[]], &[0]);
    assert_eq!(
        bfs_depth(&a),
        bfs_depth(&b),
        "children arrive, nothing else"
    );
    assert_eq!(bfs_depth(&a).levels(), [0, 1, 1, 1, 2]);
}

#[test]
fn a_declared_root_is_depth_zero_whatever_the_forests_own_roots_are() {
    let f = forest(4, &[(0, 1), (0, 2), (1, 3)]);
    assert_eq!(f.roots(), [0], "the detected root is 0");
    let d = depth_from(&f, &[1, 0]);
    assert_eq!(d.levels(), [0, 0, 1, 1], "both declared roots are depth 0");
    assert_eq!(d.max(), 1);
}

#[test]
fn a_declared_root_that_descends_from_another_declared_root_stays_at_depth_zero() {
    let f = forest(3, &[(0, 1), (1, 2)]);
    let d = depth_from(&f, &[0, 1]);
    assert_eq!(d.levels(), [0, 0, 1], "declared is absolute, not relative");
}

#[test]
fn a_node_no_declared_root_reaches_is_unreached_not_depth_zero() {
    let f = rows(3, &[&[1], &[2], &[]], &[2]);
    let d = depth_from(&f, &[2]);
    assert_eq!(d.levels(), [UNREACHED, UNREACHED, 0]);
    assert_eq!(
        d.max(),
        0,
        "a reached root alone is depth 0 even when its neighbours are unreached"
    );
    assert_eq!(d.of(0), UNREACHED);
    assert_eq!(
        UNREACHED,
        u32::MAX,
        "the sentinel is u32::MAX, not a plausible depth"
    );
}

#[test]
fn a_node_two_declared_roots_reach_takes_the_shorter_of_the_two_depths() {
    let f = rows(4, &[&[1, 2, 3], &[], &[], &[]], &[0, 3]);
    assert_eq!(depth_from(&f, &[0, 3]).levels(), [0, 1, 1, 0]);
}

#[test]
fn max_is_the_deepest_level_and_not_the_number_of_nodes() {
    let chain = forest(4, &[(0, 1), (1, 2), (2, 3)]);
    assert_eq!(
        bfs_depth(&chain).max(),
        3,
        "four nodes, three levels below the root"
    );
    let wide = forest(4, &[(0, 1), (0, 2), (0, 3)]);
    assert_eq!(bfs_depth(&wide).max(), 1, "four nodes, one level");
}

#[test]
fn an_empty_graph_has_no_levels_and_a_maximum_of_zero() {
    let f = forest(0, &[]);
    assert_eq!(f.roots(), [] as [u32; 0], "no node, so no root");
    assert_eq!(f.virtual_root(), None);
    assert_eq!(bfs_depth(&f).levels(), [] as [u32; 0]);
    assert_eq!(bfs_depth(&f).max(), 0);
    assert_eq!(depth_from(&f, &[]).levels(), [] as [u32; 0]);
    assert_eq!(depth_from(&f, &[]).max(), 0);
    // Declaring root 0 of a graph with no node is a caller bug, not an empty answer:
    // see a_declared_root_out_of_range_panics, which is the same refusal.
}

#[test]
fn a_cycle_in_the_children_is_walked_once_and_the_first_shortest_depth_wins() {
    // The trait promises a repaired tree, but depth() must terminate on anything the
    // caller hands it rather than loop forever or write past the column.
    let f = rows(3, &[&[1], &[2], &[0]], &[0]);
    assert_eq!(bfs_depth(&f).levels(), [0, 1, 2]);
    let long_way = rows(4, &[&[1], &[2], &[3], &[1]], &[0]);
    assert_eq!(
        bfs_depth(&long_way).levels(),
        [0, 1, 2, 3],
        "3 is reached at depth 3 through 2 before the cycle offers it a shorter path"
    );
}

#[test]
#[should_panic(expected = "root 9 of 4")]
fn a_declared_root_out_of_range_panics() {
    depth_from(&forest(4, &[(0, 1)]), &[9]);
}

#[test]
#[should_panic(expected = "child 5 of node 0 is past node 3")]
fn a_child_index_past_the_last_node_panics() {
    // The same refusal on the other side of the walk: an out-of-range child would
    // otherwise be written into a 3-entry column, so `walk` checks before `claim`.
    let f = rows(3, &[&[5], &[], &[]], &[0]);
    bfs_depth(&f);
}

// No `expected`: the message is the standard library's, and pinning another crate's
// wording would make this test a tripwire for a toolchain bump, not for depth.
#[test]
#[should_panic]
fn a_depth_lookup_past_the_last_node_panics() {
    bfs_depth(&forest(2, &[(0, 1)])).of(2);
}

#[test]
#[should_panic(expected = "virtual root")]
fn two_roots_with_no_virtual_root_are_refused_in_debug() {
    // The `debug_assert` in `bfs_depth` is the only thing standing between a source
    // that drops p3's virtual root and a column that reads as if it were never there.
    let f = SaysVirtualRoot(rows(4, &[&[1], &[], &[3], &[]], &[0, 2]), None);
    bfs_depth(&f);
}

#[test]
fn a_lone_root_sits_at_depth_zero_even_when_the_source_names_a_virtual_root() {
    // `under_virtual_root` is `is_some() && roots.len() >= 2`: the second clause is
    // what keeps one root at 0, and it is the only thing pinning that.
    let f = SaysVirtualRoot(rows(3, &[&[1], &[2], &[]], &[0]), Some(3));
    assert_eq!(bfs_depth(&f).levels(), [0, 1, 2]);
}

#[test]
fn declaring_no_root_reaches_nothing_at_all() {
    let f = forest(3, &[(0, 1)]);
    let d = depth_from(&f, &[]);
    assert_eq!(
        d.levels(),
        [UNREACHED; 3],
        "an empty source list is not depth 0"
    );
    assert_eq!(
        d.max(),
        0,
        "nothing was reached, so there is no deepest level"
    );
}

#[test]
fn repeated_runs_agree_bit_for_bit() {
    let f = forest(6, &[(0, 1), (0, 2), (2, 3), (4, 5)]);
    assert_eq!(bfs_depth(&f), bfs_depth(&f));
    assert_eq!(depth_from(&f, &[1, 4]), depth_from(&f, &[1, 4]));
}
