//! The last-seen map: per workspace, the highest `(epoch, head_seq)` this process has committed
//! to or served.
//!
//! §5.3: entries only rise; the map holds at most `GRAPH_HUB_LAST_SEEN` workspaces; the least
//! recently used entry is evicted first. A `BTreeMap` and a `VecDeque`, never a `HashMap`: the
//! snapshot is in fixed order and eviction is by recency, neither of which a hash map can give.

use std::collections::{BTreeMap, VecDeque};

/// A workspace's highest committed `(epoch, head_seq)`.
type Entry = (u64, u64);

/// The detector's last-seen map, with its recency list.
#[derive(Debug)]
pub struct LastSeen {
    /// The entries, keyed by workspace id and ordered by it, so a snapshot is reproducible.
    map: BTreeMap<String, Entry>,
    /// Most recently raised at the back; the front is what [`LastSeen::evict`] pops.
    order: VecDeque<String>,
    /// `GRAPH_HUB_LAST_SEEN`: the most entries this map holds.
    cap: usize,
}

impl LastSeen {
    /// An empty map holding at most `cap` entries.
    pub fn new(cap: usize) -> LastSeen {
        LastSeen {
            map: BTreeMap::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    /// The whole map, in id order.
    ///
    /// §5.3 step 2 takes this *before* any read, so a workspace committed between this and the
    /// read is a bump rather than a silent miss.
    pub fn snapshot(&mut self) -> BTreeMap<String, Entry> {
        self.map.clone()
    }

    /// Record `(epoch, seq)` for `ws`, if it rises above what is there.
    ///
    /// A lowering is ignored: after a bump the fresh epoch is above every earlier entry, so
    /// "only rises" costs nothing and keeps a stale writer from walking the epoch back.
    pub fn raise(&mut self, ws: &str, epoch: u64, seq: u64) {
        let higher = match self.map.get(ws) {
            None => true,
            Some(&(have_epoch, have_seq)) => (epoch, seq) > (have_epoch, have_seq),
        };
        if !higher {
            return;
        }
        if self.map.insert(ws.to_string(), (epoch, seq)).is_none() {
            self.order.push_back(ws.to_string());
        } else if let Some(at) = self.order.iter().position(|id| id == ws) {
            self.order.remove(at);
            self.order.push_back(ws.to_string());
        }
        self.evict();
    }

    /// The entry for `ws`, if the map holds one.
    pub fn get(&self, ws: &str) -> Option<Entry> {
        self.map.get(ws).copied()
    }

    /// Drop the least recently used entries until the map is within `cap`.
    pub fn evict(&mut self) {
        while self.map.len() > self.cap {
            match self.order.pop_front() {
                Some(ws) => {
                    self.map.remove(&ws);
                }
                None => break,
            }
        }
    }

    /// How many entries the map holds.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the map holds nothing.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Forget everything, after a bump has committed.
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}