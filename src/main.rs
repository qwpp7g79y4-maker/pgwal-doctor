//! Command-line entry point for pgwal-doctor.

// The library forbids `unsafe`. The binary allows it in one module only: the
// stdout probe in `stdout_probe.rs`.
#![deny(unsafe_code)]

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use pgwal_doctor::{scan, Finding, PsqlSource, Report, Root, Status};

#[allow(unsafe_code)] // SAFETY: the one `fcntl(1, F_GETFD)` call only reads a flag; see the module.
mod stdout_probe;

const CANNOT_WRITE: &str = "error: cannot write the report: standard output is closed";

/// Read-only report of why PostgreSQL's pg_wal keeps growing.
#[derive(Parser, Debug)]
#[command(
    name = "pgwal-doctor",
    bin_name = "pgwal-doctor",
    version,
    about,
    term_width = 0
)]
struct Cli {
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,

    /// Read this PostgreSQL data directory (offline checks).
    #[arg(long, value_name = "PATH")]
    datadir: Option<PathBuf>,

    /// Also run the online checks through `psql -d <DSN>`. Only SELECT queries run,
    /// in a read-only session. A DSN with a password is refused: use ~/.pgpass or
    /// PGPASSWORD.
    #[arg(long, value_name = "DSN")]
    dsn: Option<String>,

    /// Run the online checks with the libpq environment (PGHOST, PGUSER, ...) only.
    #[arg(long)]
    online: bool,
}

fn main() -> ExitCode {
    // Rust replaces a closed descriptor 1 with /dev/null before `main`; the probe
    // recorded before that whether it was closed.
    let stdout_closed = stdout_probe::stdout_was_closed_at_start();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let code = if err.use_stderr() { 2 } else { 0 };
            // Help and version have nowhere to go when standard output is closed.
            if stdout_closed && code == 0 {
                return cannot_write();
            }
            err.exit();
        }
    };
    if cli.datadir.is_none() && cli.dsn.is_none() && !cli.online {
        eprintln!("error: give --datadir, --dsn or --online");
        return ExitCode::from(2);
    }
    if stdout_closed {
        return cannot_write();
    }
    let root = cli.datadir.map(Root::new);
    let report = if cli.dsn.is_some() || cli.online {
        match PsqlSource::new(cli.dsn) {
            Ok(source) => scan(root.as_ref(), Some(&source)),
            Err(err) => {
                // Offline checks still run; the DSN problem is one more finding.
                let mut findings = scan(root.as_ref(), None).findings;
                findings.push(Finding::new(
                    "online",
                    Status::Error,
                    None,
                    None,
                    err.to_string(),
                    "pass the password through ~/.pgpass or PGPASSWORD, not on the command line",
                ));
                Report::from_findings(findings)
            }
        }
    } else {
        scan(root.as_ref(), None)
    };
    if let Err(err) = print_report(&report, cli.json) {
        // A full disk or a closed pipe is not a panic. Stderr may be gone too,
        // so its result is ignored.
        let _ = writeln!(io::stderr(), "error: cannot write the report: {err}");
        return ExitCode::from(2);
    }
    ExitCode::from(report.exit_code() as u8)
}

fn cannot_write() -> ExitCode {
    let _ = writeln!(io::stderr(), "{CANNOT_WRITE}");
    ExitCode::from(2)
}

fn print_report(report: &Report, json: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    let text = if json {
        report.render_json()
    } else {
        report.render_text()
    };
    write!(out, "{text}")?;
    out.flush()
}
