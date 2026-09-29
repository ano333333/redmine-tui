# Test Redmine

This repository uses Docker's official Redmine image for local integration testing.

## Start

```sh
docker compose -f compose.redmine.yml up -d
```

Wait until initialization finishes, then open:

```text
http://localhost:8080
```

The default Redmine login is:

```text
admin / admin
```

## Configuration

The compose file defaults are intentionally local-test only. Override them with environment variables when needed:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

Common variables:

- `REDMINE_IMAGE`, default `redmine:6.1`
- `REDMINE_DB_IMAGE`, default `mysql:8.0`
- `REDMINE_PORT`, default `8080`
- `REDMINE_DB_DATABASE`, default `redmine`
- `REDMINE_DB_USERNAME`, default `redmine`
- `REDMINE_DB_PASSWORD`, default `redmine`
- `REDMINE_DB_ROOT_PASSWORD`, default `redmine-root`
- `REDMINE_SECRET_KEY_BASE`, default `redmine-tui-local-test-secret`

## Seed Test Data

Start Redmine first, then run:

```sh
cargo xtask seed-redmine
```

This command reads `datas/`, generates SQL, applies `docker/redmine/fresh_test_data.sql`, and then inserts the fixture data into the MySQL service from `compose.redmine.yml`.

Preview the SQL without changing the database:

```sh
cargo xtask seed-redmine --dry-run
```

The seeder currently inserts the fixture data present under `datas/`:

- projects, users, trackers, issue statuses, priorities, target versions, categories, and time entry activities
- issues from `datas/issues/*.yml`, preserving issue IDs
- journals and journal details from `datas/journals/*.yml`
- a `Developer` role, membership of every fixture user in every project with that role, and workflow transitions between every pair of statuses for every tracker, so that API updates can change assignees and statuses
- REST API access for the Redmine admin user via API key `0123456789abcdef0123456789abcdef01234567`

When seeding a Docker Compose project with a non-default project name, pass it through:

```sh
cargo xtask seed-redmine --project-name redmine-tui-client-test
```

## Redmine Client Integration Tests

Unit tests use `wiremock` and run with normal `cargo test`. Docker-backed Redmine client tests live in `src/clients/redmine/default_tests/container/`, compile only with the `container-tests` feature, and run through `xtask`:

```sh
cargo xtask test-redmine-client
```

The integration command starts one Redmine instance with Docker Compose under a unique project name and a random host port, then runs every test in that module with `--features container-tests` in a single serial `cargo test` run. Each test re-seeds the database before it runs, so tests do not depend on each other's changes. The seed resets `AUTO_INCREMENT`, so records created through the API get the same IDs on every run. The containers and volumes are removed when the command finishes.

These tests read the connection from `REDMINE_TUI_TEST_BASE_URL` and `REDMINE_TUI_TEST_PROJECT_NAME`, which `xtask` sets. Running them directly with `cargo test --features container-tests` fails with a message pointing to the command above.

## E2E

E2E scenarios live in `tests/e2e/`, compile only with the `e2e-tests` feature, and run through `xtask`:

```sh
cargo xtask test-e2e
```

The command starts Redmine the same way as the client integration tests, then runs the `e2e` test target serially. Each scenario re-seeds the database, launches the native binary in a PTY with `testty`, and checks both the screen and the Redmine API. Scenarios that edit text replace the editor with a fake editor set through `VISUAL`.

## Seeder Checks

```sh
bash tests/redmine_seeder_files_test.sh
```

## Stop

```sh
docker compose -f compose.redmine.yml down
```

## Reset Data

This removes the Redmine database and uploaded files.

```sh
docker compose -f compose.redmine.yml down -v
```

## Source

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine
