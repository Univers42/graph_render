//! State shared by every export in `super`: the live handle table, and the one out-buffer
//! framed return values are published into (`crate::wire`, shared with the hash-gate shims).
//! One copy of each, so a value `build.rs` inserts is exactly what `columns.rs` reads back.

use crate::handle::Handles;
use std::cell::RefCell;

pub(super) use crate::wire::publish;

thread_local! {
    pub(super) static HANDLES: RefCell<Handles> = const { RefCell::new(Handles::new()) };
}
