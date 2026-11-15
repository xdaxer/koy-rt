use crate::shell::{WatchdogAction, WatchdogPolicy, WatchdogWaitResult};

#[test]
fn decide_koy_olurse_veya_yoksa_gorev_cubugu_geri_gelir() {
    assert_eq!(WatchdogAction::RestoreTaskbar, WatchdogPolicy::decide(WatchdogWaitResult::ProcessExited));
    assert_eq!(WatchdogAction::RestoreTaskbar, WatchdogPolicy::decide(WatchdogWaitResult::ProcessNotFound));
}

#[test]
fn decide_temiz_cikista_ve_hatada_dokunmaz() {
    assert_eq!(WatchdogAction::None, WatchdogPolicy::decide(WatchdogWaitResult::StopRequested));
    assert_eq!(WatchdogAction::None, WatchdogPolicy::decide(WatchdogWaitResult::Failed));
}

#[test]
fn try_parse_process_id_gecerli_arguman() {
    assert_eq!(Some(4242), WatchdogPolicy::try_parse_process_id(&["--pid", "4242"]));
}

#[test]
fn try_parse_process_id_buyuk_kucuk_harf_ve_baska_argumanlari_tolere_eder() {
    assert_eq!(Some(17), WatchdogPolicy::try_parse_process_id(&["--x", "--PID", "17"]));
}

#[test]
fn try_parse_process_id_gecersiz_arguman_false_doner() {
    let cases: [&[&str]; 5] = [&["--pid"], &["--pid", "abc"], &["--pid", "0"], &["--pid", "-5"], &["4242"]];
    for args in cases {
        // C#: false ve processId == 0; burada None.
        assert_eq!(None, WatchdogPolicy::try_parse_process_id(args), "{args:?}");
    }
}

#[test]
fn try_parse_process_id_argumansiz_false_doner() {
    let empty: [&str; 0] = [];
    assert_eq!(None, WatchdogPolicy::try_parse_process_id(&empty));
}

#[test]
fn build_arguments_try_parse_ile_uyumlu() {
    let built = WatchdogPolicy::build_arguments(321);
    let args: Vec<&str> = built.split(' ').collect();

    assert_eq!(Some(321), WatchdogPolicy::try_parse_process_id(&args));
}

#[test]
fn stop_event_name_pidye_ozel() {
    assert_eq!(r"Local\Koy.Watchdog.Stop.99", WatchdogPolicy::stop_event_name(99));
    assert_ne!(WatchdogPolicy::stop_event_name(1), WatchdogPolicy::stop_event_name(2));
}
