# Contributing to redmine-tui

This document is for contributors. For app usage, see [README.md](README.md) or [README.ja.md](README.ja.md). 日本語版は [CONTRIBUTING.ja.md](CONTRIBUTING.ja.md) です。

## Development environment

Requirements:

- Rust toolchain compatible with edition 2024
- Docker and Docker Compose v2 for local Redmine, container integration tests, and E2E tests
- Nix (optional)

Enter the provided development shell with:

```sh
nix develop
```

It provides Rust, Cargo, Clippy, `cargo-insta`, LLVM coverage tools, Trunk, and actionlint.

## Local Redmine

The Compose setup is for local development and testing only. Start Redmine with:

```sh
docker compose -f compose.redmine.yml up -d
```

Open <http://localhost:8080/>. The default local login is `admin / admin`.

To stop Redmine:

```sh
docker compose -f compose.redmine.yml down
```

To remove all local Redmine data, including the database and uploaded files:

```sh
docker compose -f compose.redmine.yml down -v
```

The Compose configuration accepts `REDMINE_IMAGE` (default `redmine:6.1`), `REDMINE_DB_IMAGE` (default `mysql:8.0`), `REDMINE_PORT` (default `8080`), and the `REDMINE_DB_*` and `REDMINE_SECRET_KEY_BASE` settings defined in `compose.redmine.yml`. For example:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

Seed the local instance from `datas/`:

```sh
cargo xtask seed-redmine
```

Seeding resets the local test data before inserting fixtures. To inspect generated SQL without changing the database:

```sh
cargo xtask seed-redmine --dry-run
```

For a non-default Compose project name, pass `--project-name <name>`. The compatibility wrapper `scripts/seed-redmine-test-data.sh` is also available.

Fixture changes can affect `datas/`, `xtask/src/seed_redmine.rs`, `docker/redmine/fresh_test_data.sql`, and `tests/redmine_seeder_files_test.sh`; keep these in sync and run the seeder file check.

## Checks and tests

Run the standard suite:

```sh
cargo test
```

Snapshot tests use `cargo-insta`:

```sh
cargo insta test
```

Review snapshot changes before accepting them. Run the fixture consistency check with:

```sh
bash tests/redmine_seeder_files_test.sh
```

### Redmine client integration tests

Real-Redmine client tests are in `src/clients/redmine/default_tests/container/`. They require Docker and run through `xtask`:

```sh
cargo xtask test-redmine-client
```

The task starts a temporary Redmine Compose project on a random host port, re-seeds before each test, runs the module serially, and removes the containers and volumes on completion. It supplies `REDMINE_TUI_TEST_BASE_URL` and `REDMINE_TUI_TEST_PROJECT_NAME`; running the feature tests directly with Cargo is intentionally rejected. `wiremock` tests cover responses that are difficult to produce with a real Redmine and run under normal `cargo test`.

### End-to-end tests

E2E scenarios live in `tests/e2e/` and require Docker:

```sh
cargo xtask test-e2e
```

The task starts Redmine, re-seeds for each scenario, and runs the native app in a PTY. Scenarios check both the terminal screen and Redmine API. Text editing uses a fake editor configured through `VISUAL`.

### Web build and demo

The Web build uses wasm32 and Trunk. To run the demo locally:

```sh
nix develop -c trunk serve --port 8081
```

The demo uses `DemoRedmineClient`, which embeds fixtures and keeps edits in memory; it does not use a real Redmine connection.

## Project structure

- `src/`: Rust application
- `tests/e2e/`: PTY-driven E2E scenarios
- `xtask/`: development commands, fixture seeding, and Pages builds
- `datas/`: local YAML fixtures
- `compose.redmine.yml`: local Redmine setup
- `docker/redmine/fresh_test_data.sql`: test database reset SQL
- `.github/workflows/`: CI and GitHub Pages workflows
- `docs/architecture.md` and `docs/adrs/`: architecture guidance and decision records

Read [docs/architecture.md](docs/architecture.md) and relevant ADRs before making architectural or component lifecycle changes. Existing ADRs record past decisions; add a new ADR when documenting a new architectural decision rather than rewriting historical records.

## CI

`.github/workflows/ci.yml` runs unit/build checks, Redmine client integration tests, E2E tests, and the Web build in separate jobs. The unit job includes formatting, workspace build and tests, and fixture checks.
