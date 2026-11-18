//! The crash handler writes a report file for a panic. (A worker-thread panic
//! takes the same path as a main-thread one, minus the dialog, which a test
//! can't dismiss.) Own test binary: it replaces the global panic hook.

#[test]
fn a_panic_leaves_a_crash_report() {
    let dir = tempfile::tempdir().unwrap();
    lumo_ui::install_crash_handler(Some(dir.path().to_path_buf()));

    let result = std::thread::Builder::new()
        .name("lumo-test-worker".into())
        .spawn(|| panic!("simulated failure for the crash test"))
        .unwrap()
        .join();
    assert!(result.is_err());

    let reports: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let report = std::fs::read_to_string(&reports[0]).unwrap();
    for needle in [
        "Lumo crash report",
        env!("CARGO_PKG_VERSION"),
        "lumo-test-worker",
        "simulated failure for the crash test",
        "crash_test.rs",
    ] {
        assert!(
            report.contains(needle),
            "report lacks {needle:?}:\n{report}"
        );
    }
}
