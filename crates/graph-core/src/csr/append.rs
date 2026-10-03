//! A CSR that grows: one span (`start`, `len`, `cap`) per row over one `values` buffer,
//! so a value lands in its row without rebuilding the whole adjacency.
//!
//! A row with room writes in place; a full row at the tail of `values` pushes; any other
//! full row moves to the tail with `cap = max(4, 2 * len)`, leaving its old slots dead.
//! Once dead slots (abandoned or reserved) outnumber live values, the buffer is rewritten
//! in row order with no slack, which is exactly the layout [`Csr::from_pairs`] builds.
//! Rows keep arrival order, as `Csr` does: appends land after the row's existing values.
//!
//! **Caveat:** the growth policy trades memory for moves. After any append `values` holds
//! at most twice the live count, and `Vec`'s own doubling can reserve as much again, so a
//! topology that only grows can hold up to 4x the frozen value bytes (`SAFE_LIVE` bounds the
//! `4·live + 4` peak of one move). Every row costs a 12 B span against `Csr`'s 4 B offset,
//! grown or not (`docs/decisions/delta-abi.md`, "Memory"). A compaction walks every row, so on a graph with far more rows than
//! values (many isolated nodes) it costs O(rows) for O(live) appends since the last one.

use super::Csr;
use crate::arena::CapacityError;

/// Where one row lives in `values`: `len` live values from `start`, room for `cap`.
#[derive(Debug, Clone, Copy, Default)]
struct Span {
    start: u32,
    len: u32,
    cap: u32,
}

/// An append-only adjacency with [`Csr`]'s readers.
#[derive(Debug, Clone, Default)]
pub struct AppendCsr {
    values: Vec<u32>,
    spans: Vec<Span>,
    live: u32,
}

const OVERFLOW: CapacityError = CapacityError { what: "adjacency" };

impl AppendCsr {
    /// The most live values with which no append can overflow: a moving row's new room
    /// plus the buffer before it is at most `4 * live + 4` slots, kept within `u32`. A
    /// caller that checks its total against this first never meets a half-done batch.
    pub const SAFE_LIVE: u64 = (u32::MAX as u64 - 4) / 4;

    /// [`Csr::from_pairs`], with no slack: every row's `cap` is its `len`.
    pub fn from_pairs<I>(rows: u32, pairs: I) -> Result<Self, CapacityError>
    where
        I: Iterator<Item = (u32, u32)> + Clone,
    {
        Ok(Self::from(Csr::from_pairs(rows, pairs)?))
    }

    /// Adds one empty row after the last.
    pub fn push_row(&mut self) -> Result<(), CapacityError> {
        if self.spans.len() >= u32::MAX as usize {
            return Err(OVERFLOW);
        }
        let start = self.values.len() as u32;
        self.spans.push(Span {
            start,
            len: 0,
            cap: 0,
        });
        Ok(())
    }

    /// Appends `value` to row `row`, after its existing values. A row index `>= rows` is a
    /// caller bug and panics; an overflow of `u32` refuses and changes nothing.
    pub fn append(&mut self, row: u32, value: u32) -> Result<(), CapacityError> {
        let live = self.live.checked_add(1).ok_or(OVERFLOW)?;
        let span = self.spans[row as usize];
        let tail = self.values.len() as u32;
        if span.len < span.cap {
            self.values[(span.start + span.len) as usize] = value;
        } else if span.start + span.cap == tail {
            tail.checked_add(1).ok_or(OVERFLOW)?;
            self.values.push(value);
            self.spans[row as usize].cap += 1;
        } else {
            self.relocate(row, value)?;
        }
        self.spans[row as usize].len += 1;
        self.live = live;
        if self.values.len() - self.live as usize > self.live as usize {
            self.compact();
        }
        Ok(())
    }

    /// Moves full row `row` to the tail with room to grow, and writes `value` after it.
    fn relocate(&mut self, row: u32, value: u32) -> Result<(), CapacityError> {
        let span = self.spans[row as usize];
        let cap = grown_cap(span.len)?;
        let start = self.values.len() as u32;
        start.checked_add(cap).ok_or(OVERFLOW)?;
        let old = span.start as usize..(span.start + span.len) as usize;
        self.values.extend_from_within(old);
        self.values.push(value);
        self.values.resize((start + cap) as usize, 0);
        self.spans[row as usize] = Span { start, cap, ..span };
        Ok(())
    }

    /// Rewrites `values` in row order with no slack. Never grows the buffer, so it cannot
    /// overflow.
    fn compact(&mut self) {
        let mut values = Vec::with_capacity(self.live as usize);
        for span in &mut self.spans {
            let start = values.len() as u32;
            values.extend_from_slice(&self.values[span.start as usize..][..span.len as usize]);
            *span = Span {
                start,
                len: span.len,
                cap: span.len,
            };
        }
        self.values = values;
    }

    /// Row `row`'s values, in arrival order.
    pub fn row(&self, row: u32) -> &[u32] {
        let span = self.spans[row as usize];
        &self.values[span.start as usize..][..span.len as usize]
    }

    /// Number of rows.
    pub fn rows(&self) -> u32 {
        self.spans.len() as u32
    }

    /// Total values across every row.
    pub fn len(&self) -> usize {
        self.live as usize
    }

    /// True when no row holds a value.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Bytes in use by the spans and `values`, slack and dead slots included; a `Vec`'s
    /// unused capacity is not counted.
    pub fn byte_len(&self) -> usize {
        self.spans.len() * size_of::<Span>() + self.values.len() * size_of::<u32>()
    }
}

impl From<Csr> for AppendCsr {
    fn from(csr: Csr) -> Self {
        let spans = csr
            .offsets
            .windows(2)
            .map(|w| Span {
                start: w[0],
                len: w[1] - w[0],
                cap: w[1] - w[0],
            })
            .collect();
        let live = csr.values.len() as u32;
        Self {
            values: csr.values,
            spans,
            live,
        }
    }
}

/// The room a moved row gets: `max(4, 2 * len)`, refused past `u32`.
fn grown_cap(len: u32) -> Result<u32, CapacityError> {
    Ok(len.checked_mul(2).ok_or(OVERFLOW)?.max(4))
}

#[cfg(test)]
mod tests;
