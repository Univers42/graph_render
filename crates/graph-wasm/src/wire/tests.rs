use super::{publish, publish_stage};
use crate::errors::{self, Code};

/// C4: a `0` return is ambiguous on its own, so every `0` that is a refusal names its
/// reason. On this host the out-buffer's address never fits `u32`, so `0` here is the
/// refusal and must carry a code, not the `None` a previous success left behind.
#[test]
fn a_zero_from_publish_always_carries_a_nonzero_code() {
    errors::clear();
    let address = publish(vec![1, 2, 3]);
    assert!(
        address != 0 || errors::get() != Code::None as u32,
        "publish returned 0 with no error code"
    );
}

#[test]
fn a_refused_gate_stage_names_its_reason() {
    errors::clear();
    assert_eq!(publish_stage(None), 0);
    assert_eq!(errors::get(), Code::LayoutFailed as u32);
}

#[test]
fn a_published_gate_stage_never_leaves_a_stale_code() {
    errors::set(Code::InvalidHandle);
    let address = publish_stage(Some(vec![7]));
    let code = errors::get();
    assert!(address != 0 || code != Code::None as u32);
    assert_ne!(
        code,
        Code::InvalidHandle as u32,
        "a stale code survived the call"
    );
}
