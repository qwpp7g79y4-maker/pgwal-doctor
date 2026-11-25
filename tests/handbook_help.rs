//! The built binary's `--help` and the documented example agree with the handbook.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

/// Cargo 1.99 exports `CARGO_BIN_EXE_pgwal-doctor`. Older Cargo exports
/// `CARGO_BIN_EXE_pgwal_doctor` (hyphens turned into underscores). Either one
/// is the real binary path set for this integration test.
fn doctor() -> Command {
    let path = option_env!("CARGO_BIN_EXE_pgwal_doctor")
        .or(option_env!("CARGO_BIN_EXE_pgwal-doctor"))
        .expect("CARGO_BIN_EXE_pgwal_doctor");
    Command::new(path)
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Whitespace-delimited tokens that start with `--`, with trailing punctuation removed.
fn long_options(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_start_matches(['(', '[', '"', '\'', '`']);
        let Some(body) = token.strip_prefix("--") else {
            continue;
        };
        let mut name = String::from("--");
        for c in body.chars() {
            if c.is_ascii_alphanumeric() || c == '-' {
                name.push(c);
            } else {
                break;
            }
        }
        if name.len() > 2 {
            found.insert(name);
        }
    }
    found
}

fn help_stdout(args: &[&str]) -> String {
    let out = doctor().args(args).output().expect("run pgwal-doctor");
    assert_eq!(
        out.status.code(),
        Some(0),
        "args {args:?} stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("help is utf-8")
        .replace("\r\n", "\n")
}

/// Command names listed under a `Commands:` heading, the way clap prints them.
fn subcommands_named_by(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_commands = false;
    for line in help.lines() {
        if line == "Commands:" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.is_empty() || !line.starts_with(' ') {
            break;
        }
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            names.push(name.to_string());
        }
    }
    names
}

#[test]
fn help_matches_man_page_and_unknown_option_is_not_a_panic() {
    let help = help_stdout(&["--help"]);
    assert!(help.contains("Usage: pgwal-doctor [OPTIONS]"));

    let mut options = long_options(&help);
    for name in subcommands_named_by(&help) {
        let sub = help_stdout(&[&name, "--help"]);
        options.extend(long_options(&sub));
    }

    let man =
        std::fs::read_to_string(manifest().join("man/pgwal-doctor.1")).expect("read man page");
    for section in [
        ".SH NAME",
        ".SH SYNOPSIS",
        ".SH DESCRIPTION",
        ".SH OPTIONS",
        ".SH EXIT STATUS",
        ".SH EXAMPLES",
    ] {
        assert!(man.contains(section), "missing {section}");
    }
    let documented = long_options(&man);
    assert_eq!(
        documented,
        options,
        "man page long options differ from --help\nonly in man: {:?}\nonly in help: {:?}",
        documented.difference(&options),
        options.difference(&documented)
    );

    let unknown = doctor()
        .arg("--not-a-real-option")
        .output()
        .expect("run pgwal-doctor");
    assert_ne!(unknown.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn documented_example_exits_as_readme_says() {
    let root = manifest();
    let readme = std::fs::read_to_string(root.join("examples/README.md")).expect("examples readme");
    let (command, expected) = example_from_readme(&readme);
    let mut parts = command.split_whitespace();
    assert_eq!(parts.next(), Some("pgwal-doctor"));
    let args: Vec<&str> = parts.collect();
    assert!(
        root.join("examples/quiet-datadir/pg_wal").is_dir(),
        "example fixture is missing"
    );

    let out = doctor()
        .current_dir(&root)
        .args(&args)
        .output()
        .expect("run documented example");
    assert_eq!(
        out.status.code(),
        Some(expected),
        "stdout {}\nstderr {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("1 segments"),
        "example did not read the fixture"
    );
}

fn example_from_readme(readme: &str) -> (String, i32) {
    let mut command = None;
    let mut in_sh = false;
    let mut code = None;
    for line in readme.lines() {
        let trimmed = line.trim();
        if trimmed == "```sh" {
            in_sh = true;
            continue;
        }
        if in_sh {
            if trimmed == "```" {
                in_sh = false;
                continue;
            }
            if command.is_none() && !trimmed.is_empty() && !trimmed.starts_with('#') {
                command = Some(trimmed.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Expected exit code:") {
            code = rest.trim().trim_end_matches('.').parse().ok();
        }
    }
    (
        command.expect("examples/README.md command"),
        code.expect("examples/README.md exit code"),
    )
}
