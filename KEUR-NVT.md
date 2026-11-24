build.rs: pgwal-doctor has no build script; nothing is generated at build time.
about.hbs: no third-party license page is generated for this tool, so no template exists.
about.toml: cargo-about is not used in this project, so there is no configuration for it.
docs: documentation is the README, the manual page and the examples directory; there is no docs directory.
samples: the fictional fixtures live in examples/quiet-datadir, so there is no samples directory.
scripts: there are no helper scripts; everything runs through cargo and the binary.
.github/workflows/release.yml: there is no release workflow because the tool is not released (publish = false).
