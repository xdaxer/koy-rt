use crate::notch::{NotchState, NotchStateTransitions, NotchTrigger};

use NotchState::{Collapsed, Expanded, Peek};
use NotchTrigger::*;

fn check(rows: &[(NotchState, NotchTrigger, NotchState)]) {
    for (initial, trigger, expected) in rows {
        assert_eq!(*expected, NotchStateTransitions::transition(*initial, *trigger), "{initial:?} {trigger:?}");
    }
}

#[test]
fn collapsed_transitions() {
    check(&[
        (Collapsed, MouseEnter, Peek),
        (Collapsed, TrackChanged, Peek),
        (Collapsed, Click, Expanded),
        (Collapsed, MouseLeave, Collapsed),
        (Collapsed, AutoPeekTimeout, Collapsed),
        (Collapsed, MouseLeaveDelayTimeout, Collapsed),
    ]);
}

#[test]
fn peek_transitions() {
    check(&[
        (Peek, Click, Expanded),
        (Peek, MouseLeave, Collapsed),
        (Peek, AutoPeekTimeout, Collapsed),
        (Peek, MouseEnter, Peek),
        (Peek, TrackChanged, Peek),
    ]);
}

#[test]
fn expanded_transitions() {
    check(&[
        (Expanded, Click, Collapsed),
        (Expanded, MouseLeaveDelayTimeout, Collapsed),
        (Expanded, MouseEnter, Expanded),
        (Expanded, MouseLeave, Expanded),
        (Expanded, TrackChanged, Expanded),
    ]);
}
