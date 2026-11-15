use chrono::{NaiveDate, NaiveDateTime};

use crate::time::TimeSpan;
use crate::tray::{TrayHostAction, TrayHostLease, TrayHostRecovery};

fn t0() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(12, 0, 0).unwrap()
}

fn after(span: TimeSpan) -> NaiveDateTime {
    t0() + span.to_chrono()
}

mod tray_host_recovery_tests {
    use super::*;

    #[test]
    fn acquires_when_wanted_and_missing() {
        assert_eq!(TrayHostAction::Acquire, TrayHostRecovery::new().decide(true, false, false, t0()));
    }

    #[test]
    fn keeps_running_host() {
        assert_eq!(TrayHostAction::None, TrayHostRecovery::new().decide(true, true, true, t0()));
    }

    #[test]
    fn reacquires_dead_host() {
        assert_eq!(TrayHostAction::Reacquire, TrayHostRecovery::new().decide(true, true, false, t0()));
    }

    #[test]
    fn releases_when_no_longer_wanted() {
        let mut r = TrayHostRecovery::new();
        assert_eq!(TrayHostAction::Release, r.decide(false, true, true, t0()));
        assert_eq!(TrayHostAction::None, r.decide(false, false, false, t0()));
    }

    #[test]
    fn failed_acquire_is_retried_after_backoff_not_blocked_forever() {
        let mut r = TrayHostRecovery::new();
        r.report_acquire(false, t0());

        assert_eq!(TrayHostAction::None, r.decide(true, false, false, after(TimeSpan::from_secs(1))));
        assert_eq!(TrayHostAction::Acquire, r.decide(true, false, false, after(TrayHostRecovery::FIRST_RETRY_DELAY)));
    }

    #[test]
    fn dead_host_is_released_but_not_reacquired_during_backoff() {
        let mut r = TrayHostRecovery::new();
        r.report_acquire(false, t0());
        assert_eq!(TrayHostAction::Release, r.decide(true, true, false, after(TimeSpan::from_secs(1))));
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let mut r = TrayHostRecovery::new();
        for _ in 0..20 {
            r.report_acquire(false, t0());
        }

        let almost = after(TrayHostRecovery::MAX_RETRY_DELAY) - TimeSpan::from_secs(1).to_chrono();
        assert_eq!(TrayHostAction::None, r.decide(true, false, false, almost));
        assert_eq!(TrayHostAction::Acquire, r.decide(true, false, false, after(TrayHostRecovery::MAX_RETRY_DELAY)));
    }

    #[test]
    fn success_and_reset_clear_backoff() {
        let mut r = TrayHostRecovery::new();
        r.report_acquire(false, t0());
        r.report_acquire(true, t0());
        assert_eq!(0, r.failures());
        assert_eq!(TrayHostAction::Acquire, r.decide(true, false, false, t0()));

        r.report_acquire(false, t0());
        r.reset_failures();
        assert_eq!(TrayHostAction::Acquire, r.decide(true, false, false, t0()));
    }

    #[test]
    fn not_wanted_clears_backoff() {
        let mut r = TrayHostRecovery::new();
        r.report_acquire(false, t0());
        r.decide(false, false, false, t0());
        assert_eq!(TrayHostAction::Acquire, r.decide(true, false, false, t0()));
    }
}

mod tray_host_lease_tests {
    use super::*;

    #[test]
    fn last_release_signals_close() {
        let mut l = TrayHostLease::new();
        l.acquire();
        l.acquire();
        assert!(!l.release());
        assert!(l.release());
        assert_eq!(0, l.users());
    }

    #[test]
    fn extra_release_never_goes_negative() {
        let mut l = TrayHostLease::new();
        assert!(!l.release());
        l.acquire();
        assert_eq!(1, l.users());
    }

    #[test]
    fn reset_clears_users() {
        let mut l = TrayHostLease::new();
        l.acquire();
        l.reset();
        assert_eq!(0, l.users());
    }
}
