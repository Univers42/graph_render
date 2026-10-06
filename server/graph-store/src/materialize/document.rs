//! The open document: one snapshot, read a page at a time.

use std::collections::VecDeque;

use graph_contract::hub::Cursor;
use graph_contract::ingest::{DOC_SEPARATOR, doc_tail};
use tokio_postgres::Client;

use super::declared::{Dangling, Declared};
use crate::error::StoreError;

/// One page of records after `($2, $3)` in `(qcoll, id)` byte order, each with the references it
/// holds that the snapshot does not resolve: at most `$4` rows, and only as many as fit `$5` bytes
/// of row cost, except that the first row is always kept so the walk always moves.
///
/// `links` is in the default collation and `records` in `"C"`, so every comparison between them
/// names the collation it means: Postgres refuses an implicit mix of the two.
///
/// Caveat: a page cut by bytes still scans `$4` keys and their lengths (not their texts, which
/// stay toasted), so a workspace of 1 MiB records reads about 512 keys for each one it returns.
/// The unresolved-reference arrays are not costed; they name only targets the record's own text
/// names, so a page can exceed `$5` by about its texts again when every reference dangles.
const PAGE: &str = "SELECT p.qcoll, p.id, p.text, \
       COALESCE(d.tq, '{}'::text[]), COALESCE(d.tid, '{}'::text[]) \
     FROM (SELECT s.ws, s.qcoll, s.id, s.text, row_number() OVER w n, sum(s.c) OVER w cost \
       FROM (SELECT r.ws, r.qcoll, r.id, r.text, \
               octet_length(r.text) + octet_length(r.qcoll) + octet_length(r.id) + $6 c \
             FROM records r \
             WHERE r.ws = $1 AND (r.qcoll, r.id) > ($2::text COLLATE \"C\", $3::text COLLATE \"C\") \
             ORDER BY r.qcoll, r.id LIMIT $4) s \
       WINDOW w AS (ORDER BY s.qcoll, s.id ROWS UNBOUNDED PRECEDING)) p \
     LEFT JOIN LATERAL ( \
       SELECT array_agg(l.target_qcoll) tq, array_agg(l.target_id) tid FROM links l \
       WHERE l.ws = p.ws AND l.src_qcoll = (p.qcoll COLLATE \"default\") \
         AND l.src_id = (p.id COLLATE \"default\") \
         AND NOT EXISTS (SELECT 1 FROM records t WHERE t.ws = l.ws \
           AND t.qcoll = (l.target_qcoll COLLATE \"C\") AND t.id = (l.target_id COLLATE \"C\")) \
     ) d ON true \
     WHERE p.n = 1 OR p.cost <= $5 \
     ORDER BY p.qcoll, p.id";

/// Bytes a row costs on top of its three strings: the driver's row and its column ranges, the
/// `VecDeque` entry and the two key copies.
///
/// Caveat: estimated from the types' sizes, not measured; a page of tiny records is under-costed
/// when the allocator rounds each string up, and `hub-memory` measures the whole read instead.
const ROW_COST: i32 = 512;

/// What `open` read before the first record.
pub(crate) struct Opened {
    /// The workspace.
    pub(crate) ws: String,
    /// The snapshot's `(epoch, head_seq)`.
    pub(crate) cursor: Cursor,
    /// The snapshot's `doc_bytes`.
    pub(crate) doc_bytes: u64,
    /// Rows one page reads, at least 1.
    pub(crate) fetch_rows: i64,
    /// Row cost one page reads beyond its first row.
    pub(crate) page_bytes: i64,
}

/// One record row of a page.
struct Row {
    qcoll: String,
    id: String,
    text: String,
    dangling: Dangling,
}

/// A workspace's document at one cursor, streamed as `head`, every `next`, then `tail`.
pub struct Document {
    client: Client,
    opened: Opened,
    declared: Declared,
    head: String,
    page: VecDeque<Row>,
    after: (String, String),
    exhausted: bool,
    first: bool,
    done: bool,
}

impl Document {
    /// A document over the transaction `client` holds open.
    pub(crate) fn new(client: Client, opened: Opened, declared: Declared) -> Document {
        Document {
            head: declared.head(),
            client,
            opened,
            declared,
            page: VecDeque::new(),
            after: (String::new(), String::new()),
            exhausted: false,
            first: true,
            done: false,
        }
    }

    /// Everything before the first record.
    pub fn head(&self) -> &str {
        &self.head
    }

    /// The next record's piece, with the separator before it, or `None` once every record is
    /// written; the snapshot's transaction is committed then.
    pub async fn next(&mut self) -> Result<Option<String>, StoreError> {
        if self.page.is_empty() && !self.exhausted {
            self.fetch().await?;
        }
        let Some(row) = self.page.pop_front() else {
            self.finish().await?;
            return Ok(None);
        };
        let piece = self.declared.piece(&row.qcoll, row.text, &row.dangling)?;
        let mut out = String::new();
        if !self.first {
            out.push_str(DOC_SEPARATOR);
        }
        self.first = false;
        out.push_str(&piece);
        Ok(Some(out))
    }

    /// Everything after the last record.
    pub fn tail(&self) -> String {
        doc_tail(&self.opened.ws)
    }

    /// The `(epoch, head_seq)` this document is the state at.
    pub fn cursor(&self) -> Cursor {
        self.opened.cursor
    }

    /// The snapshot's `doc_bytes`: the document's exact length when nothing is pruned, and an
    /// upper bound of it otherwise.
    pub fn doc_bytes(&self) -> u64 {
        self.opened.doc_bytes
    }

    /// Read the page after the last row read.
    async fn fetch(&mut self) -> Result<(), StoreError> {
        let rows = self
            .client
            .query(
                PAGE,
                &[
                    &self.opened.ws,
                    &self.after.0,
                    &self.after.1,
                    &self.opened.fetch_rows,
                    &self.opened.page_bytes,
                    &ROW_COST,
                ],
            )
            .await?;
        // A short page may be a byte cut, so only an empty one ends the walk.
        self.exhausted = rows.is_empty();
        for row in rows {
            let targets: Vec<String> = row.get(3);
            let ids: Vec<String> = row.get(4);
            self.page.push_back(Row {
                qcoll: row.get(0),
                id: row.get(1),
                text: row.get(2),
                dangling: targets.into_iter().zip(ids).collect(),
            });
        }
        if let Some(last) = self.page.back() {
            self.after = (last.qcoll.clone(), last.id.clone());
        }
        Ok(())
    }

    /// Commit the snapshot once; a second call does nothing.
    async fn finish(&mut self) -> Result<(), StoreError> {
        if !self.done {
            self.client.batch_execute("COMMIT").await?;
            self.done = true;
        }
        Ok(())
    }
}
