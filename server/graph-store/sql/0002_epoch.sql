-- The epoch clock, the database's identity, and the one function that draws an epoch (spec H15,
-- §5.3).
--
-- WHY microseconds: `last = greatest(last + 1, clock_timestamp() * 1e6)` puts an epoch near
-- 1.79e15 in 2026, below `2^53 - 1` and so exactly representable on the wire. A millisecond
-- reading would put it near 1.8e12 and, worse, would let two hubs draw the same epoch after a
-- restore. The run-ahead is microseconds too: N epochs drawn inside one microsecond leave `last`
-- N microseconds ahead. Pinned by `epoch_is_microseconds` and `epoch_run_ahead_is_microseconds`.

CREATE TABLE epoch_clock (
  one bool PRIMARY KEY CHECK (one),
  last bigint NOT NULL
);

INSERT INTO epoch_clock(one, last) VALUES (true, 0);

CREATE TABLE hub_meta (
  one bool PRIMARY KEY CHECK (one),
  system_identifier bigint NOT NULL,
  timeline text NOT NULL,
  datoid oid NOT NULL
);

-- One function, not four: the trigger functions call this once per distinct workspace.
CREATE OR REPLACE FUNCTION hub_next_epoch() RETURNS bigint LANGUAGE sql AS $$
  UPDATE epoch_clock SET last = greatest(last + 1,
    (extract(epoch FROM clock_timestamp()) * 1000000)::bigint)
  RETURNING last
$$;