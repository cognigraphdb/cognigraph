use super::*;

fn parsed(words: &[&str]) -> Result<Options, String> {
    parse(words)
}

#[test]
fn full_and_minimal_forms() {
    let options = parsed(&[
        "--from-arangodump",
        "dump",
        "--output",
        "s.redb",
        "--dry-run",
        "--report",
        "r.json",
        "--max-record-bytes",
        "10",
        "--max-expanded-bytes",
        "20",
        "--max-files",
        "3",
    ])
    .unwrap();
    assert_eq!(options.dump, PathBuf::from("dump"));
    assert_eq!(options.output, PathBuf::from("s.redb"));
    assert!(options.dry_run);
    assert_eq!(options.report, Some(PathBuf::from("r.json")));
    assert_eq!(
        options.limits,
        Limits {
            max_record_bytes: 10,
            max_expanded_bytes: 20,
            max_files: 3
        }
    );
    let minimal = parsed(&["--output", "s", "--from-arangodump", "d"]).unwrap();
    assert!(!minimal.dry_run);
    assert_eq!(minimal.limits, Limits::default());
}

#[test]
fn every_malformed_form_is_a_message_not_a_panic() {
    for (words, message) in [
        (&["--from-arangodump", "d"][..], "needs --output"),
        (&["--output", "s"][..], "needs a dump directory"),
        (
            &["--from-arangodump"][..],
            "--from-arangodump expects a value",
        ),
        (
            &["--from-arangodump", "d", "--output"][..],
            "--output expects a value",
        ),
        (
            &["--from-arangodump", "d", "--output", "--dry-run"][..],
            "--output expects a value",
        ),
        (
            &[
                "--from-arangodump",
                "d",
                "--output",
                "s",
                "--max-files",
                "0",
            ][..],
            "positive integer",
        ),
        (
            &[
                "--from-arangodump",
                "d",
                "--output",
                "s",
                "--max-record-bytes",
                "-1",
            ][..],
            "positive integer",
        ),
        (
            &[
                "--from-arangodump",
                "d",
                "--output",
                "s",
                "--max-files",
                "x",
            ][..],
            "positive integer",
        ),
        (
            &[
                "--from-arangodump",
                "d",
                "--from-arangodump",
                "e",
                "--output",
                "s",
            ][..],
            "more than once",
        ),
        (
            &["--from-arangodump", "d", "--output", "s", "--force"][..],
            "unknown flag `--force`",
        ),
        (
            &["--from-arangodump", "d", "--output", "s", "extra"][..],
            "unexpected argument `extra`",
        ),
    ] {
        let error = parsed(words).unwrap_err();
        assert!(error.contains(message), "{words:?}: {error}");
    }
}
