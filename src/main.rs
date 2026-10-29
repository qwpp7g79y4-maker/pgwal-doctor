//! Command-line entry point for pgwal-doctor.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use pgwal_doctor::{scan, Report, Root};

/// Read-only report of why PostgreSQL's pg_wal keeps growing.
#[derive(Parser, Debug)]
#[command(name = "pgwal-doctor", version, about)]
struct Cli {
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,

    /// Read this PostgreSQL data directory (offline checks).
    #[arg(long, value_name = "PATH", required = true)]
    datadir: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = cli.datadir.map(Root::new);
    let report = scan(root.as_ref(), None);
    print_report(&report, cli.json);
    ExitCode::from(report.exit_code() as u8)
}

fn print_report(report: &Report, json: bool) {
    if json {
        print!("{}", report.render_json());
    } else {
        print!("{}", report.render_text());
    }
}
