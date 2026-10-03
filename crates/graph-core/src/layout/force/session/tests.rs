//! The session's tests: `m1a`–`m1e` are the five determinism cases the milestone names,
//! `live` is everything else the API does, `frozen_path` pins the one acceptance path that
//! is not a range check, `support` is what they share.

mod carry;
mod digest;
mod frozen;
mod frozen_path;
mod golden;
mod live;
mod m1a;
mod m1b;
mod m1c;
mod m1d;
mod m1e;
mod pins;
mod setters;
mod support;
mod verbs;
mod warm;
