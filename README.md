# redmine-tui

[日本語の README はこちら](README.ja.md)。

redmine-tui is a draft terminal UI for browsing and editing Redmine issues from your terminal. It connects to a Redmine server through the REST API.

## Features

- Browse issues and their properties, descriptions, child issues, and journals.
- Update issue properties such as status, priority, assignee, dates, category, and estimated hours.
- Edit issue descriptions and journal notes in your configured terminal editor.
- Add journal notes and record spent time.
- Save changes to Redmine and review conflicts when the server data changed during editing.

This project is still a prototype, and its interface and feature set may change.

## Requirements

- Rust toolchain compatible with edition 2024
- A Redmine account with REST API access enabled and an API key for the native TUI
- Nix (optional), to use the provided development shell

## Run the TUI

Set your API key and start the app:

```sh
REDMINE_API_KEY=<your-api-key> cargo run
```

The app loads Redmine data at startup. `REDMINE_API_KEY` is required; if it is missing or the initial load fails, the app prints an error and exits.

Connection settings:

- `REDMINE_API_KEY`: Redmine REST API access key (required)
- `REDMINE_URL`: Redmine base URL (optional; defaults to `http://127.0.0.1:${REDMINE_PORT:-8080}`)
- `REDMINE_PORT`: fallback port used only when `REDMINE_URL` is not set (optional; defaults to `8080`)

For example, connect to a Redmine server on a non-default URL:

```sh
REDMINE_URL=https://redmine.example.com REDMINE_API_KEY=<your-api-key> cargo run
```

## Basic controls

The app is keyboard-driven. The focused panel handles input, and available actions are shown in the interface.

| Key       | Action                                               |
| --------- | ---------------------------------------------------- |
| `j` / `k` | Move down / up through items or fields               |
| `h` / `l` | Move between panels or columns where available       |
| `e`       | Edit the focused field or text                       |
| `y`       | Open the issue picker                                |
| `Ctrl-S`  | Save the current issue or journal entry to Redmine   |
| `Enter`   | Confirm a selection                                  |
| `q`       | Quit from the main screen or close the focused popup |

When editing text, the configured external editor is opened. Popup controls can vary; check the hints shown in each screen.

## Typical workflow

1. Start the app with your Redmine API key.
2. Press `y` to choose an issue, then use `j` and `k` to move through the list and `Enter` to open it.
3. Navigate issue fields and journal entries with `j` and `k`. Press `e` to edit a field or note.
4. Press `Ctrl-S` to send the changes to Redmine. If a conflict is detected, review the server and local values in the conflict screen.

## Development

Contributor setup, local Redmine instructions, test commands, and repository guidelines are in [CONTRIBUTING.md](CONTRIBUTING.md).
