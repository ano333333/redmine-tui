# redmine-tui

A draft terminal UI for browsing and editing Redmine issue data.

The current application is a local Ratatui prototype. It reads data from a Redmine server running in a container and can update issues and journals.

A Web demo renders the terminal UI in a browser (see the Web Demo section).

## Requirements

- Rust toolchain compatible with edition 2024
- Docker and Docker Compose v2, for the local Redmine test server
- Nix, optional, for the provided development shell

## Development Shell

If you use Nix:

```sh
nix develop
```

This shell provides Rust, Cargo, Clippy, `cargo-insta`, LLVM coverage tools, Trunk, and actionlint.

## Run the TUI

```sh
cargo run
```

The TUI requires `REDMINE_API_KEY` to load initial Redmine entities from a
Redmine server at startup. If the required environment variable is missing, or
if the initial load fails, the app prints the error reason and exits.

```sh
REDMINE_API_KEY=0123456789abcdef0123456789abcdef01234567 cargo run
```

TUI connection environment variables:

- `REDMINE_API_KEY`: required Redmine REST API access key. The TUI loads users,
  statuses, priorities, projects, trackers, versions, categories, and time entry
  activities from Redmine at startup.
- `REDMINE_URL`: Redmine base URL. If omitted, the TUI uses
  `http://127.0.0.1:${REDMINE_PORT:-8080}`.
- `REDMINE_PORT`: fallback port used only when `REDMINE_URL` is omitted.

Key bindings shown in the application:

- `Left` / `Right`: shrink or expand the rendered width
- `Up` / `Down`: shrink or expand the rendered height
- `q`: quit

## Test

```sh
cargo test
```

Snapshot tests use `cargo-insta`:

```sh
cargo insta test
```

Seeder file checks:

```sh
bash tests/redmine_seeder_files_test.sh
```

`.github/workflows/ci.yml` checks the native build (`cargo fmt --check`, `cargo build --workspace`, `cargo test --workspace`) and the Web build (wasm32 `cargo build`, `trunk build`).

## Web Demo

The Web version renders the terminal UI in a browser with Ratzilla and runs against a memory mock (`DemoRedmineClient`) that embeds the fixtures. It does not connect to a real Redmine server and requires no API key. Edits exist only in memory; reloading or leaving the page discards them and restores the fixture state.

```sh
nix develop -c trunk serve --port 8081
```

Open `http://127.0.0.1:8081/` (the default port 8080 is used by the local Redmine server, so use a different port).

## Local Redmine With Docker

This repository includes a Docker Compose setup for a local Redmine instance used during development and testing.

Start Redmine:

```sh
docker compose -f compose.redmine.yml up -d
```

Open:

```text
http://localhost:8080
```

Default Redmine login:

```text
admin / admin
```

Stop Redmine:

```sh
docker compose -f compose.redmine.yml down
```

Reset all Redmine data, including the MySQL database and uploaded files:

```sh
docker compose -f compose.redmine.yml down -v
```

## Redmine Configuration

The Compose file is intended for local testing only. It uses Docker named volumes for MySQL data and Redmine uploaded files.

Common environment variables:

- `REDMINE_IMAGE`, default `redmine:6.1`
- `REDMINE_DB_IMAGE`, default `mysql:8.0`
- `REDMINE_PORT`, default `8080`
- `REDMINE_DB_DATABASE`, default `redmine`
- `REDMINE_DB_USERNAME`, default `redmine`
- `REDMINE_DB_PASSWORD`, default `redmine`
- `REDMINE_DB_ROOT_PASSWORD`, default `redmine-root`
- `REDMINE_SECRET_KEY_BASE`, default `redmine-tui-local-test-secret`

Example using a different host port:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

## Seed Redmine Test Data

Start Redmine first, then run:

```sh
cargo xtask seed-redmine
```

This command generates SQL from `datas/` and pipes it into the MySQL service in `compose.redmine.yml`. It first applies `docker/redmine/fresh_test_data.sql`, so the local test data is reset before seeding.

To inspect the generated SQL without touching the database:

```sh
cargo xtask seed-redmine --dry-run
```

The seeder currently inserts the fixture data present in `datas/`:

- projects from `datas/projects.yml`
- users from `datas/users.yml`
- trackers, statuses, priorities, versions, categories, and time entry activities
- issues from `datas/issues/*.yml`, preserving issue IDs
- journals and journal details from `datas/journals/*.yml`

The compatibility wrapper remains available:

```sh
scripts/seed-redmine-test-data.sh
```

## Redmine References

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine

## Repository Layout

- `src/`: Rust TUI source
- `xtask/`: Cargo development tasks, including Redmine YAML seeding and Pages builds
- `index.html`: Trunk entry HTML for the Web build
- `Trunk.toml`: Trunk configuration for the Web build
- `datas/`: local YAML fixture data
- `compose.redmine.yml`: local Redmine Docker Compose setup
- `docker/redmine/fresh_test_data.sql`: Redmine test-data reset SQL used before YAML seeding
- `scripts/seed-redmine-test-data.sh`: seeder execution wrapper
- `.github/workflows/`: CI and GitHub Pages workflows
- `docs/redmine-test.md`: detailed local Redmine notes
