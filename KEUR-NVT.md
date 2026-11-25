build.rs: pgwal-doctor has no build script; nothing is generated at build time.
docs: documentation is the README, the manual page and the examples directory; there is no docs directory.
samples: the fictional fixtures live in examples/quiet-datadir, so there is no samples directory.
scripts: there are no helper scripts; everything runs through cargo and the binary.
.github/workflows/release.yml: there is no release workflow because the tool is not released (publish = false).
