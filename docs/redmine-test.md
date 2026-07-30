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
scripts/seed-redmine-test-data.sh
```

The seeder is idempotent. It creates or updates:

- project: `Redmine TUI Sandbox` / `redmine-tui-sandbox`
- users: `alice.tui`, `bob.tui`
- version: `TUI Test v1.0`
- category: `TUI`
- three issues, including a parent issue and child issue
- journal comments and time entries for TUI rendering checks

The seeded user password is:

```text
password123
```

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
