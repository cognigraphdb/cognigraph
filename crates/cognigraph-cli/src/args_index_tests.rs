//! CG-86: index subcommands.

use super::{Command, parse};

fn parse_ok(args: &[&str]) -> Command {
    parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .unwrap()
        .command
}

#[test]
fn index_commands_parse() {
    assert_eq!(
        parse_ok(&["index", "list", "users"]),
        Command::IndexList {
            collection: "users".into()
        }
    );
    assert_eq!(
        parse_ok(&["index", "ensure", "users", "email"]),
        Command::IndexEnsure {
            collection: "users".into(),
            fields: vec!["email".into()],
            name: None,
            unique: true,
            sparse: false,
        }
    );
    assert_eq!(
        parse_ok(&[
            "index",
            "ensure",
            "users",
            "oauth.provider,oauth.id",
            "--name",
            "oauth",
            "--sparse",
            "--non-unique"
        ]),
        Command::IndexEnsure {
            collection: "users".into(),
            fields: vec!["oauth.provider".into(), "oauth.id".into()],
            name: Some("oauth".into()),
            unique: false,
            sparse: true,
        }
    );
    assert_eq!(
        parse_ok(&["index", "drop", "users", "email_unique"]),
        Command::IndexDrop {
            collection: "users".into(),
            name: "email_unique".into()
        }
    );
}

#[test]
fn index_commands_reject_malformed_input() {
    for args in [
        &["index", "ensure", "users"][..],
        &["index", "ensure", "users", ""][..],
        &["index", "ensure", "users", "a,,b"][..],
        &["index", "ensure", "users", "a", "--name"][..],
        &["index", "ensure", "users", "a", "--bogus"][..],
        &["index", "drop", "users"][..],
        &["index", "frobnicate", "users"][..],
    ] {
        assert!(
            parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err(),
            "{args:?}"
        );
    }
}

#[test]
fn import_with_the_dump_flag_is_the_offline_importer_and_plain_import_is_unchanged() {
    let words: Vec<String> = ["import", "--from-arangodump", "d", "--output", "s"]
        .iter()
        .map(|w| w.to_string())
        .collect();
    match parse(&words).unwrap().command {
        Command::ImportArangodump(options) => {
            assert_eq!(options.dump, std::path::PathBuf::from("d"));
            assert_eq!(options.output, std::path::PathBuf::from("s"));
        }
        other => panic!("{other:?}"),
    }
    let words = vec!["import".to_string(), "snapshot.json".to_string()];
    assert_eq!(
        parse(&words).unwrap().command,
        Command::Import {
            file: Some("snapshot.json".into())
        }
    );
    let words: Vec<String> = ["import", "--from-arangodump", "d"]
        .iter()
        .map(|w| w.to_string())
        .collect();
    assert!(parse(&words).unwrap_err().contains("--output"));
}
