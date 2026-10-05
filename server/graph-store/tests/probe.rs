//! Temporary probe, removed after it answered.
#![cfg(feature = "db-tests")]

mod support;

#[tokio::test]
async fn probe_param() {
    let (mut client, _, _) = support::db::fresh_pair("probe_at_cast").await;
    let at = "2026-10-05T09:12:44.120Z".to_owned();
    for sql in [
        "INSERT INTO change_headers (ws, seq, plugin, at, kind, bytes, ops) \
         VALUES ($1, $2, $3, $4::timestamptz, $5, $6, $7)",
        "INSERT INTO change_headers (ws, seq, plugin, at, kind, bytes, ops) \
         VALUES ($1, $2, $3, CAST($4 AS text)::timestamptz, $5, $6, $7)",
        "INSERT INTO change_headers (ws, seq, plugin, at, kind, bytes, ops) \
         VALUES ($1, $2, $3, $4::text::timestamptz, $5, $6, $7)",
        "INSERT INTO change_headers (ws, seq, plugin, at, kind, bytes, ops) \
         VALUES ($1, $2, $3, to_timestamp($4, 'YYYY-MM-DD\"T\"HH24:MI:SS.MSZ'), $5, $6, $7)",
    ] {
        let r = client
            .execute(
                sql,
                &[&"ws", &1i64, &"p", &at, &"batch", &7i64, &0i32],
            )
            .await;
        match r {
            Ok(_) => println!("probe ok: {sql}"),
            Err(e) => println!("probe err: {sql} -> {:?}", e.into_source()),
        }
    }
}