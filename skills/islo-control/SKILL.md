---
name: islo-control
description: "Control islo tasks, reminders, calendar, focus, water, shelf, media and settings through its CLI. Configure mail accounts with local private-input forms and query mail, notification and quota status. Use for islo requests, not unrelated apps."
---

# islo control

Use the CLI for the user's requested operations. It shares validation and persistence with the GUI. Never edit the running application's data files behind its back.

## Find the CLI

On Windows, prefer `$env:LOCALAPPDATA\Programs\islo\islo-cli.exe`. In a checkout, use `target/debug/islo-cli.exe`, or build it with `cargo build --locked --bin islo-cli`. The command reference is [cli/README.md](../../cli/README.md) in the repository.

## Operate

1. Run `capabilities` to discover actual commands, required fields, flag types and online/offline support. Run `app status` for local time and timezone offset before resolving relative dates. The interface must be enabled in Settings → General → Command line interface. Enabling registers `islo-cli` in the user's PATH; reopen terminals to pick it up.
2. Use the matching domain command. Convert natural-language dates into explicit local `YYYY-MM-DD HH:MM`; the CLI does not parse phrases like “tomorrow afternoon”. For tasks, list before changes and use the returned `reference`. References are snapshots, not permanent IDs; refresh after changes. Prefer setting explicit values over toggles when supported.
3. Check the exit code and JSON `ok` field, then query to confirm. `backend: running-app` means the GUI handled it; `offline` means the next startup will load it. Async actions return `accepted: true, completed: false`; poll status to assess the actual outcome. If an editor is busy, preserve its unsaved work and report the busy result.
4. For preferences, use `settings list` for canonical keys and string values, `settings set KEY VALUE`, or atomic `settings apply --file PATCH.json`. The CLI cannot change its own enable switch.

Examples:

```powershell
$isloCli = Join-Path $env:LOCALAPPDATA 'Programs\islo\islo-cli.exe'
& $isloCli settings list
& $isloCli capabilities
& $isloCli app status
& $isloCli tasks add --text "Submit report" --due "2026-11-07 15:00"
& $isloCli tasks list
& $isloCli focus start
& $isloCli water drink --ml 250
& $isloCli settings set pet_expression laughing
& $isloCli settings get pet_expression
& $isloCli settings set pet_expression auto
```

Values use the application's existing format: booleans `0`/`1`, decimal integers, and compact JSON strings for lists. Never guess an unknown setting key or treat an invalid value as saved. When a request times out, query first because the write outcome may be unknown; do not automatically repeat a mutation.

For a visible companion, inspect `pet`, `minimal`, `dock` and auto-hide preferences. A saved expression can remain hidden by those settings; change visibility only when the user requests it. Use `--data-dir` only for isolated test directories, since that bypasses live routing.

For every Agent/CLI configuration, ordinary fields belong in commands. Resolve missing ordinary fields with available context or a non-sensitive question; do not open a configuration form for them. A required password, authorization code, API key or token must use the supported domain's compact local secret-only prompt. Reuse existing credentials when possible. Never request secrets in chat, pass them through arguments/JSON/stdin, read the input window or clipboard, or capture screenshots to obtain values. If the CLI does not support a secret configuration, report that limitation instead of editing its credential store. Read `capabilities.interaction_policy` for this contract; it does not imply additional commands exist.

For mail, use `mail list` and `mail configure` with known ordinary metadata. `needs_metadata` returns required ordinary fields and opens no window; complete these through a new command. `awaiting_input` opens only a masked secret prompt. Tell the user to confirm there, then query `mail input-status --id ID`. While pending, preserve its request ID and do not reissue the mutation. Use `mail input-cancel --id ID` if the user cancels. `saved` confirms persistence; check `mail status` separately for connection status.

Outlook uses `provider outlook` and Microsoft browser OAuth. Its public application Client ID is the ordinary `outlook_client_id` setting; a missing ID is `needs_metadata`, not a request for a password. `awaiting_authorization` means the user must sign in and consent on the Microsoft website; poll the same input-status command. Do not inspect login pages, callback URLs or tokens. No password fallback or client secret is used. Product registration and live-test requirements are in [docs/OUTLOOK.md](../../docs/OUTLOOK.md).

Reminders fire only while islo runs. Calendar list/add/remove manage local entries; remote calendars expose connection status/sync. Shelf removal unlinks a reference, never deletes the original. Mail/notifications expose status and counts, not message content. Secret readout, calendar credentials, AI login files and Windows autostart remain outside this API. Do not work around this boundary by editing their stores. Detailed domain rules and examples are in [cli/README.md](../../cli/README.md).
