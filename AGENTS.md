# islo release versions

- Development branches use three components, for example `26.4.3` or `26.4.4`.
- Public release versions use four components. The first release on `26.4.3` is `26.4.3.1`, followed by `26.4.3.2`, and so on. On a new `26.4.4` branch, start again at `26.4.4.1`.
- `VERSION` is the public version source. Start at `.1` on a new branch; when the current version has already been released, increment its fourth component for the next release. Do not replace an existing release or move its tag.
- Cargo requires three-component SemVer. Keep `Cargo.toml` and the root package in `Cargo.lock` equal to the branch prefix, while application UI, update checks, Windows resources and package filenames use `VERSION` via `ISLO_APP_VERSION`.
- Run update-version tests and the native installer/updater fixtures for both three- and four-component versions when changing release plumbing.

## Entry points and settings

- Keep one Cargo package with the desktop entry in `src/main.rs`, CLI entry in `cli/src/main.rs`, shared settings in `src/model.rs`, and local command transport in `src/agent_control.rs`.
- CLI and agent settings changes must reuse model validation and persistence. A running GUI owns mutations; do not overwrite its settings file behind its back.
- For operating the local application's settings, read `skills/islo-control/SKILL.md` and use `islo-cli`. Account secrets and system autostart are outside this settings interface.
