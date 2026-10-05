//! Random single-op batches against the store and the Model, the documents compared after every
//! step. A refused batch is fine as long as both sides refuse it (`Twin::apply` asserts that).

use crate::support::rng::SplitMix64;

use super::*;

/// Record ids inside `tracker`, small so references often resolve and often do not.
const IDS: [&str; 4] = ["a", "b", "c", "d"];
/// Record ids of `other.thing`, the cross-plugin link's targets.
const THINGS: [&str; 3] = ["t1", "t2", "t3"];
/// Scalar cells: integers, fractions, exponents, strings, maps, bools and null.
const SCALARS: [&str; 7] = [
    "1",
    "0.1",
    "-2.5e-7",
    "\"txt\"",
    "{\"k\":[1,null]}",
    "true",
    "null",
];
/// `tracker`'s collections.
const COLLECTIONS: [&str; 3] = ["task", "c", "note"];

/// One of `from`.
fn pick<'a>(rng: &mut SplitMix64, from: &[&'a str]) -> &'a str {
    from[rng.below(from.len() as u64) as usize]
}

/// A single reference: null, a bare id, or a one-element list of one.
fn reference(rng: &mut SplitMix64, pool: &[&str]) -> String {
    match rng.below(3) {
        0 => "null".to_owned(),
        1 => format!(r#""{}""#, pick(rng, pool)),
        _ => format!(r#"["{}"]"#, pick(rng, pool)),
    }
}

/// `blocks`: zero to three ids, repeats allowed.
fn blocks(rng: &mut SplitMix64) -> String {
    let n = rng.below(4);
    let ids: Vec<String> = (0..n)
        .map(|_| format!(r#""{}""#, pick(rng, &IDS)))
        .collect();
    format!("[{}]", ids.join(","))
}

/// A task's cells, each reference field present half the time.
fn task_cells(rng: &mut SplitMix64, at: u32) -> String {
    let mut cells = vec![format!(r#""name":"N{at}""#)];
    if rng.below(2) == 0 {
        cells.push(format!(r#""blocks":{}"#, blocks(rng)));
    }
    if rng.below(2) == 0 {
        cells.push(format!(r#""up":{}"#, reference(rng, &IDS)));
    }
    if rng.below(2) == 0 {
        cells.push(format!(r#""link":{}"#, reference(rng, &THINGS)));
    }
    if rng.below(2) == 0 {
        cells.push(format!(r#""note":{}"#, pick(rng, &SCALARS)));
    }
    cells.join(",")
}

/// An upsert into one of `tracker`'s collections.
fn tracker_upsert(rng: &mut SplitMix64, at: u32) -> Batch {
    let coll = pick(rng, &COLLECTIONS);
    let id = pick(rng, &IDS);
    let cells = if coll == "task" {
        task_cells(rng, at)
    } else {
        format!(r#""name":"N{at}""#)
    };
    batch_of(&[(coll, id, at, cells.as_str())], &[])
}

/// A delete of a record `tracker` stores, or `None` when it stores none.
fn tracker_delete(rng: &mut SplitMix64, model: &Model) -> Option<Batch> {
    let stored: Vec<(String, String)> = model
        .records()
        .filter_map(|s| {
            let coll = s.record.collection.strip_prefix("tracker.")?;
            Some((coll.to_owned(), s.record.id.clone()))
        })
        .collect();
    if stored.is_empty() {
        return None;
    }
    let (coll, id) = &stored[rng.below(stored.len() as u64) as usize];
    Some(batch_of(&[], &[(coll.as_str(), id.as_str())]))
}

/// A thing written or deleted, which resolves or dangles every link to it.
fn thing_op(rng: &mut SplitMix64, at: u32) -> Batch {
    let id = pick(rng, &THINGS);
    if rng.below(3) == 0 {
        batch_of(&[], &[("thing", id)])
    } else {
        batch_of(&[("thing", id, at, r#""name":"T""#)], &[])
    }
}

/// One random op, applied on both sides.
async fn step(twin: &mut Twin, rng: &mut SplitMix64, at: u32) {
    let (plugin, batch) = match rng.below(4) {
        0 | 1 => ("tracker", tracker_upsert(rng, at)),
        2 => match tracker_delete(rng, &twin.model) {
            Some(batch) => ("tracker", batch),
            None => ("tracker", tracker_upsert(rng, at)),
        },
        _ => ("other", thing_op(rng, at)),
    };
    twin.apply(plugin, batch).await;
}

/// Thirty ops per seed, the document equal to the Model's after each one.
#[tokio::test]
async fn random_ops_materialize_canonically() {
    for seed in 0..32u64 {
        let mut twin = Twin::new(&format!("random_{seed}"), 1 + seed % 3).await;
        twin.register("tracker", TRACKER).await;
        twin.register("other", OTHER).await;
        let mut rng = SplitMix64::seeded(seed);
        for at in 1..=30u32 {
            step(&mut twin, &mut rng, at).await;
            assert_eq!(
                twin.read().await,
                twin.model.to_json(),
                "seed {seed} step {at}"
            );
        }
    }
}
