//! Findings and the text / JSON report.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Ok,
    Warn,
    NeedsRoot,
    NotPresent,
    Error,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "Ok",
            Self::Warn => "Warn",
            Self::NeedsRoot => "NeedsRoot",
            Self::NotPresent => "NotPresent",
            Self::Error => "Error",
        }
    }

    /// Warn, NeedsRoot, and Error are the results that set exit code 1.
    pub fn is_actionable(self) -> bool {
        matches!(self, Self::Warn | Self::NeedsRoot | Self::Error)
    }
}

/// One row in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub status: Status,
    pub bytes: Option<u64>,
    pub count: Option<u64>,
    pub evidence: String,
    pub hint: String,
}

impl Finding {
    pub fn new(
        check: impl Into<String>,
        status: Status,
        bytes: Option<u64>,
        count: Option<u64>,
        evidence: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            check: check.into(),
            status,
            bytes,
            count,
            evidence: evidence.into(),
            hint: hint.into(),
        }
    }
}

/// Collected findings, sorted by bytes descending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Checks that hit `NeedsRoot` at least once, sorted and deduplicated.
    pub needs_root: Vec<String>,
}

impl Report {
    pub fn from_findings(mut findings: Vec<Finding>) -> Self {
        findings.sort_by(cmp_findings);
        findings.dedup();
        let mut needs_root: Vec<String> = findings
            .iter()
            .filter(|finding| finding.status == Status::NeedsRoot)
            .map(|finding| finding.check.clone())
            .collect();
        needs_root.sort();
        needs_root.dedup();
        Self {
            findings,
            needs_root,
        }
    }

    /// `0` when nothing actionable was found, `1` when something was.
    pub fn exit_code(&self) -> i32 {
        if self
            .findings
            .iter()
            .any(|finding| finding.status.is_actionable())
        {
            1
        } else {
            0
        }
    }

    pub fn render_text(&self) -> String {
        let mut out = String::from("pgwal-doctor\n");
        if self.findings.is_empty() {
            out.push_str("\nNo findings.\n");
            return out;
        }
        out.push('\n');
        for finding in &self.findings {
            let bytes = match finding.bytes {
                Some(value) => value.to_string(),
                None => "-".to_string(),
            };
            let count = match finding.count {
                Some(value) => value.to_string(),
                None => "-".to_string(),
            };
            out.push_str(&format!(
                "[{status}] {check} bytes={bytes} count={count}\nevidence: {evidence}\nhint: {hint}\n\n",
                status = finding.status.as_str(),
                check = escape(&finding.check),
                evidence = escape(&finding.evidence),
                hint = escape(&finding.hint),
            ));
        }
        if !self.needs_root.is_empty() {
            out.push_str(&format!(
                "More access needed for: {}.\n",
                self.needs_root.join(", ")
            ));
        }
        out
    }

    pub fn render_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("report values are json-safe");
        text.push('\n');
        text
    }
}

fn cmp_findings(a: &Finding, b: &Finding) -> Ordering {
    match (a.bytes, b.bytes) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| a.check.cmp(&b.check))
    .then_with(|| a.evidence.cmp(&b.evidence))
}

/// Escape control characters so one path cannot break the text report.
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if other.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(other)));
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_is_text_and_json() {
        let report = Report::from_findings(Vec::new());
        assert_eq!(report.render_text(), "pgwal-doctor\n\nNo findings.\n");
        assert_eq!(report.exit_code(), 0);
        let value: serde_json::Value = serde_json::from_str(&report.render_json()).expect("json");
        assert!(value["findings"].as_array().expect("array").is_empty());
    }

    #[test]
    fn needs_root_renders_in_text_and_json() {
        let report = Report::from_findings(vec![Finding::new(
            "wal_dir",
            Status::NeedsRoot,
            None,
            None,
            "cannot read pg_wal",
            "run as the postgres user",
        )]);
        let text = report.render_text();
        assert!(text.contains("[NeedsRoot] wal_dir"));
        assert!(text.contains("run as the postgres user"));
        assert!(text.contains("More access needed for: wal_dir."));
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.needs_root, vec!["wal_dir".to_string()]);
        let value: serde_json::Value = serde_json::from_str(&report.render_json()).expect("json");
        assert_eq!(value["findings"][0]["status"], "NeedsRoot");
        assert!(value["findings"][0]["hint"]
            .as_str()
            .expect("hint")
            .contains("run as the postgres"));
        assert_eq!(value["needs_root"][0], "wal_dir");
    }

    #[test]
    fn identical_findings_are_deduplicated() {
        let one = Finding::new("online", Status::Error, None, None, "refused", "");
        let report = Report::from_findings(vec![one.clone(), one]);
        assert_eq!(report.findings.len(), 1);
    }

    #[test]
    fn needs_root_summary_lists_each_check_once() {
        let report = Report::from_findings(vec![
            Finding::new("wal_dir", Status::NeedsRoot, None, None, "a", "hint a"),
            Finding::new("wal_dir", Status::NeedsRoot, None, None, "b", "hint b"),
            Finding::new(
                "archive_backlog",
                Status::NeedsRoot,
                None,
                None,
                "c",
                "hint c",
            ),
            Finding::new("slots", Status::Ok, Some(1), None, "d", "hint d"),
        ]);
        assert_eq!(
            report.needs_root,
            vec!["archive_backlog".to_string(), "wal_dir".to_string()]
        );
        assert!(report
            .render_text()
            .contains("More access needed for: archive_backlog, wal_dir."));
    }

    #[test]
    fn text_sorts_by_bytes_descending() {
        let report = Report::from_findings(vec![
            Finding::new("small", Status::Warn, Some(10), None, "a", ""),
            Finding::new("none", Status::Ok, None, None, "c", ""),
            Finding::new("large", Status::Warn, Some(50), None, "b", ""),
        ]);
        let checks: Vec<_> = report
            .findings
            .iter()
            .map(|finding| finding.check.as_str())
            .collect();
        assert_eq!(checks, ["large", "small", "none"]);
    }

    #[test]
    fn text_escapes_control_characters() {
        let report = Report::from_findings(vec![Finding::new(
            "wal_dir",
            Status::Warn,
            Some(1),
            None,
            "segment a\nb\tname",
            "hint",
        )]);
        let text = report.render_text();
        assert!(text.contains("evidence: segment a\\nb\\tname\n"));
        assert!(!text.contains("evidence: segment a\nb"));
    }
}
