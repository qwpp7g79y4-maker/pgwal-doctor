//! The English and Dutch READMEs keep the same section structure, and the
//! contributing guide points to Issues.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn read(name: &str) -> Option<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name);
    fs::read_to_string(path).ok()
}

/// Headings of level 1 to 3, outside fenced code blocks.
fn headings(text: &str) -> Vec<String> {
    let mut in_fence = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if (1..=3).contains(&hashes) && line[hashes..].starts_with(' ') {
            found.push(line[hashes..].trim().to_string());
        }
    }
    found
}

fn level2_count(text: &str) -> usize {
    let mut in_fence = false;
    let mut count = 0;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence && line.starts_with("## ") {
            count += 1;
        }
    }
    count
}

#[test]
fn readmes_have_the_same_number_of_sections() {
    let en = read("README.md").expect("README.md");
    let nl = read("README.nl.md").expect("README.nl.md");
    assert_eq!(
        level2_count(&en),
        level2_count(&nl),
        "README.md and README.nl.md must have the same number of `##` sections"
    );
}

#[test]
fn readme_headings_are_unique() {
    for name in ["README.md", "README.nl.md"] {
        let text = read(name).expect(name);
        let all = headings(&text);
        let unique: BTreeSet<_> = all.iter().map(|h| h.to_lowercase()).collect();
        assert_eq!(all.len(), unique.len(), "{name} repeats a heading: {all:?}");
    }
}

#[test]
fn options_and_usage_are_never_one_heading() {
    for name in ["README.md", "README.nl.md"] {
        let text = read(name).expect(name);
        for h in headings(&text) {
            assert!(
                !h.contains('/') || !h.to_lowercase().contains("usage"),
                "{name}: split the heading `{h}` into separate headings"
            );
        }
    }
}

/// CONTRIBUTING.md arrives with the documents; when present it asks for
/// contributions through GitHub Issues and sends security reports to SECURITY.md.
#[test]
fn contributing_points_to_issues_and_security() {
    let Some(text) = read("CONTRIBUTING.md") else {
        return;
    };
    assert!(
        text.contains("Issues"),
        "CONTRIBUTING.md must mention Issues"
    );
    assert!(
        text.contains("SECURITY.md"),
        "CONTRIBUTING.md must point to SECURITY.md"
    );
}
