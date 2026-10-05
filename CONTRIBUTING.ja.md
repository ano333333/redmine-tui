# redmine-tui へのコントリビュート

この文書はコントリビューター向けです。アプリの利用方法は [README.ja.md](README.ja.md) を参照してください。英語版は [CONTRIBUTING.md](CONTRIBUTING.md) です。

## 開発環境

必要なもの:

- Rust edition 2024 に対応した Rust ツールチェーン
- ローカル Redmine、コンテナ統合テスト、E2E テスト用の Docker と Docker Compose v2
- Nix（任意）

提供されている開発シェルには次のコマンドで入れます。

```sh
nix develop
```

開発シェルには Rust、Cargo、Clippy、`cargo-insta`、LLVM coverage ツール、Trunk、actionlint が含まれます。

## ローカル Redmine

Compose 構成はローカル開発とテスト専用です。次のコマンドで Redmine を起動します。

```sh
docker compose -f compose.redmine.yml up -d
```

<http://localhost:8080/> を開きます。初期ログイン情報は `admin / admin` です。

停止する場合:

```sh
docker compose -f compose.redmine.yml down
```

データベースとアップロード済みファイルを含むすべてのローカルデータを削除する場合:

```sh
docker compose -f compose.redmine.yml down -v
```

Compose 設定では `REDMINE_IMAGE`（既定値 `redmine:6.1`）、`REDMINE_DB_IMAGE`（既定値 `mysql:8.0`）、`REDMINE_PORT`（既定値 `8080`）、および `compose.redmine.yml` に定義されている `REDMINE_DB_*` と `REDMINE_SECRET_KEY_BASE` を指定できます。例:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

`datas/` の fixture をローカル Redmine に投入します。

```sh
cargo xtask seed-redmine
```

seed の前にローカルテストデータがリセットされます。データベースを変更せずに生成 SQL を確認する場合:

```sh
cargo xtask seed-redmine --dry-run
```

既定以外の Compose project 名を使う場合は `--project-name <name>` を渡します。互換用ラッパー `scripts/seed-redmine-test-data.sh` も利用できます。

fixture の変更は `datas/`、`xtask/src/seed_redmine.rs`、`docker/redmine/fresh_test_data.sql`、`tests/redmine_seeder_files_test.sh` に影響することがあります。関連ファイルの整合性を保ち、seeder ファイルのチェックを実行してください。

## チェックとテスト

通常のテストを実行します。

```sh
cargo test
```

Snapshot テストには `cargo-insta` を使います。

```sh
cargo insta test
```

Snapshot の差分を確認してから採用してください。fixture の整合性チェック:

```sh
bash tests/redmine_seeder_files_test.sh
```

### Redmine client 統合テスト

実際の Redmine に接続する client テストは `src/clients/redmine/default_tests/container/` にあります。Docker が必要で、`xtask` から実行します。

```sh
cargo xtask test-redmine-client
```

このタスクはランダムなホスト側 port を使う一時的な Redmine Compose project を起動し、各テストの前に seed を入れ直して、テストを直列に実行します。終了時にコンテナと volume を削除します。接続情報として `REDMINE_TUI_TEST_BASE_URL` と `REDMINE_TUI_TEST_PROJECT_NAME` を設定します。Cargo から feature テストを直接実行することは意図的に拒否されます。実際の Redmine で起こしにくい応答を検証する `wiremock` テストは通常の `cargo test` で実行します。

### E2E テスト

E2E シナリオは `tests/e2e/` にあります。Docker が必要です。

```sh
cargo xtask test-e2e
```

このタスクは Redmine を起動し、各シナリオの前に seed を入れ直して、native アプリを PTY 上で実行します。画面表示と Redmine API の両方を確認します。テキスト編集には `VISUAL` で指定した偽の editor を使います。

### Web build とデモ

Web build は wasm32 と Trunk を使います。デモをローカルで起動する場合:

```sh
nix develop -c trunk serve --port 8081
```

デモは fixture を埋め込んだ `DemoRedmineClient` を使い、編集内容はメモリ上に保持します。実際の Redmine には接続しません。

## プロジェクト構成

- `src/`: Rust アプリケーション
- `tests/e2e/`: PTY 上で操作する E2E シナリオ
- `xtask/`: 開発用コマンド、fixture seeding、Pages build
- `datas/`: ローカル YAML fixture
- `compose.redmine.yml`: ローカル Redmine 構成
- `docker/redmine/fresh_test_data.sql`: テストデータベースのリセット SQL
- `.github/workflows/`: CI と GitHub Pages の workflow
- `docs/architecture.md` と `docs/adrs/`: アーキテクチャガイドと意思決定記録

アーキテクチャや component lifecycle を変更する前に [docs/architecture.md](docs/architecture.md) と関連 ADR を確認してください。既存 ADR は過去の判断を記録したものです。新しいアーキテクチャ判断は、過去の記録を書き換えるのではなく新しい ADR として追加してください。

## CI

`.github/workflows/ci.yml` は unit/build チェック、Redmine client 統合テスト、E2E テスト、Web build を別々の job で実行します。unit job には format、workspace build と test、fixture チェックが含まれます。
