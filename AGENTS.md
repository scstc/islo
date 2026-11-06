# islo release versions

- Development branches use three components, for example `26.4.3` or `26.4.4`.
- Public release versions use four components. The first release on `26.4.3` is `26.4.3.1`, followed by `26.4.3.2`, and so on. On a new `26.4.4` branch, start again at `26.4.4.1`.
- `VERSION` is the public version source. Increment its fourth component before each release; do not replace an existing release or move its tag.
- Cargo requires three-component SemVer. Keep `Cargo.toml` and the root package in `Cargo.lock` equal to the branch prefix, while application UI, update checks, Windows resources and package filenames use `VERSION` via `ISLO_APP_VERSION`.
- Run update-version tests and the native installer/updater fixtures for both three- and four-component versions when changing release plumbing.
