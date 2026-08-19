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
- REST API access for the Redmine admin user via API key `0123456789abcdef0123456789abcdef01234567`

When seeding a Docker Compose project with a non-default project name, pass it through:

```sh
cargo xtask seed-redmine --project-name redmine-tui-client-test
```

## Redmine Client Integration Tests

Unit tests use `wiremock` and run with normal `cargo test`. Docker-backed Redmine client tests are ignored by default and run through `xtask`:

```sh
cargo xtask test-redmine-client
```

The integration command starts Redmine with `testcontainers`, seeds fixture data, and runs small method-level contract tests for `DefaultRedmineClient`.

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
