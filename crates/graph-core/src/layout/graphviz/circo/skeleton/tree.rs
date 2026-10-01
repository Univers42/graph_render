//! The spanning forest of the kept edges, and the long path read off it
//! (`blockpath.c:263-369`).
//!
//! A depth-first pass over the thinned block records each node's tree parent, and then every
//! **leaf** measures its distance to every ancestor. The node holding the deepest *pair* of
//! paths is the branch the circle starts from, and the two leaves below it give the order:
//! one leaf walked up to the branch, then the other walked up and reversed. On a path-shaped
//! block that pair of walks *is* the whole block, which is why a 5-cycle needs no residual
//! pass; where the tree branches, `DISTTWO` is what supplies the second walk.

use super::Work;

/// One node's tree parent and tree degree.
pub(super) struct Tree {
    pub(super) parents: Vec<u32>,
    pub(super) degrees: Vec<u32>,
}

/// `spanning_tree` (`blockpath.c:344-369`): depth-first over the kept edges, in the
/// reference's order, starting from every unvisited node in block order.
pub(super) fn spanning_tree(work: &Work) -> Tree {
    let nodes = work.degree.len();
    let kept: Vec<Vec<u32>> = (0..nodes as u32).map(|node| work.kept_row(node)).collect();
    let mut parents = vec![u32::MAX; nodes];
    let mut rows: Vec<Vec<u32>> = (0..nodes).map(|_| Vec::new()).collect();
    let mut visited = vec![false; nodes];
    for start in 0..nodes as u32 {
        if !visited[start as usize] {
            parents[start as usize] = u32::MAX;
            descend(start, &kept, &mut visited, &mut parents, &mut rows);
        }
    }
    Tree {
        parents,
        degrees: rows.iter().map(|row| row.len() as u32).collect(),
    }
}

/// The reference's recursive `dfs` (`blockpath.c:321-339`) as a frame stack, so a deep block
/// cannot overflow the call stack.
fn descend(
    start: u32,
    kept: &[Vec<u32>],
    visited: &mut [bool],
    parents: &mut [u32],
    rows: &mut [Vec<u32>],
) {
    let mut stack = vec![(start, 0usize)];
    visited[start as usize] = true;
    while !stack.is_empty() {
        let last = stack.len() - 1;
        let node = stack[last].0;
        let step = kept[node as usize].get(stack[last].1).copied();
        stack[last].1 += 1;
        match step {
            None => {
                stack.pop();
            }
            Some(other) if !visited[other as usize] => {
                visited[other as usize] = true;
                parents[other as usize] = node;
                rows[node as usize].push(other);
                rows[other as usize].push(node);
                stack.push((other, 0));
            }
            Some(_) => {}
        }
    }
}

/// `find_longest_path` (`blockpath.c:263-318`): every leaf measures its distance up, then the
/// node with the deepest *pair* of paths names the start of the circle order.
pub(super) fn longest_path(tree: &Tree) -> Vec<u32> {
    let nodes = tree.parents.len();
    if nodes == 1 {
        return vec![0];
    }
    let mut leaves = Best::of(nodes);
    for node in 0..nodes as u32 {
        if tree.degrees[node as usize] == 1 {
            measure(node, node, 0, &mut leaves, tree);
        }
    }
    let mut path = Vec::new();
    let mut common = None;
    let mut longest = 0;
    for node in 0..nodes as u32 {
        let length = leaves.longest(node) + leaves.second(node);
        if length > longest {
            common = Some(node);
            longest = length;
        }
    }
    let Some(common) = common else {
        return path;
    };
    climb(leaves.best(common), common, tree, &mut path);
    path.push(common);
    if leaves.second(common) > 0 {
        let mut second = Vec::new();
        climb(leaves.runner_up(common), common, tree, &mut second);
        second.reverse();
        path.extend(second);
    }
    path
}

/// `LEAFONE`/`DISTONE` and `LEAFTWO`/`DISTTWO`, per node.
struct Best {
    best: Vec<u32>,
    runner_up: Vec<u32>,
    longest: Vec<u32>,
    second: Vec<u32>,
}

impl Best {
    fn of(nodes: usize) -> Self {
        Self {
            best: vec![u32::MAX; nodes],
            runner_up: vec![u32::MAX; nodes],
            longest: vec![0; nodes],
            second: vec![0; nodes],
        }
    }

    fn best(&self, node: u32) -> u32 {
        self.best[node as usize]
    }

    fn longest(&self, node: u32) -> u32 {
        self.longest[node as usize]
    }

    fn runner_up(&self, node: u32) -> u32 {
        self.runner_up[node as usize]
    }

    fn second(&self, node: u32) -> u32 {
        self.second[node as usize]
    }
}

/// `measure_distance` (`blockpath.c:224-260`): a leaf's distance to every ancestor, keeping
/// the best two at each. The reference recurses up one chain, so this loops; `change` is the
/// leaf the current call has already demoted, and skipping a demotion for it is what keeps
/// one branch from being counted twice.
fn measure(node: u32, mut ancestor: u32, mut distance: u32, leaves: &mut Best, tree: &Tree) {
    let mut change = u32::MAX;
    loop {
        let parent = tree.parents[ancestor as usize];
        if parent == u32::MAX {
            return;
        }
        distance += 1;
        let at = parent as usize;
        if leaves.longest[at] == 0 {
            leaves.best[at] = node;
            leaves.longest[at] = distance;
        } else if distance > leaves.longest[at] {
            if leaves.best[at] != change {
                if leaves.second[at] == 0 || leaves.runner_up[at] != change {
                    change = leaves.best[at];
                }
                leaves.runner_up[at] = leaves.best[at];
                leaves.second[at] = leaves.longest[at];
            }
            leaves.best[at] = node;
            leaves.longest[at] = distance;
        } else if distance > leaves.second[at] {
            leaves.runner_up[at] = node;
            leaves.second[at] = distance;
            return;
        } else {
            return;
        }
        ancestor = parent;
    }
}

/// `for (n = LEAF(common); n != common; n = TPARENT(n))`: the leaf up to, but not
/// including, the branch node.
fn climb(leaf: u32, common: u32, tree: &Tree, out: &mut Vec<u32>) {
    let mut node = leaf;
    while node != common && node != u32::MAX {
        out.push(node);
        node = tree.parents[node as usize];
    }
}
