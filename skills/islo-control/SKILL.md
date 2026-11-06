---
name: islo-control
description: "Query and change islo desktop preferences through its local CLI, including appearance, companion expressions, world-clock cities, music, focus, water and notification preferences. Use for islo settings requests; not for editing account credentials or unrelated desktop apps."
---

# islo settings

Use the CLI to read and change requested preferences. It shares validation and persistence with the GUI and sends changes to a running application, so editing `settings.txt` by hand is unnecessary.

## Find the CLI

On Windows, prefer `$env:LOCALAPPDATA\Programs\islo\islo-cli.exe`. In a checkout, use `target/debug/islo-cli.exe`, or build it with `cargo build --locked --bin islo-cli`. The command reference is [cli/README.md](../../cli/README.md) in the repository.

## Operate

1. Run `settings list` to discover supported keys and their current canonical string values.
2. Change only the preferences requested by the user. Use `settings set KEY VALUE`, or `settings apply --file PATCH.json` for an atomic batch. JSON patch values must be strings.
3. Check the exit code and JSON `ok` field, then query the changed keys to confirm the result. `backend: running-app` means the GUI handled it; `offline` means it will apply at next startup.

Examples:

```powershell
$isloCli = Join-Path $env:LOCALAPPDATA 'Programs\islo\islo-cli.exe'
& $isloCli settings list
& $isloCli settings set pet_expression laughing
& $isloCli settings get pet_expression
& $isloCli settings set pet_expression auto
```

Values use the application's existing format: booleans `0`/`1`, decimal integers, and compact JSON strings for lists. Never guess an unknown setting key or treat an invalid value as saved. When a request times out, query first because the write outcome may be unknown; do not automatically repeat a mutation.

For a visible companion, inspect `pet`, `minimal`, `dock` and auto-hide preferences. A saved expression can remain hidden by those settings; change visibility only when the user requests it. Use `--data-dir` only for isolated test directories, since that bypasses live routing.

The CLI does not expose passwords, email/calendar account credentials, AI login files or Windows autostart. Do not work around that boundary by editing those stores as part of an ordinary settings request.
