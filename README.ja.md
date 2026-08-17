# redmine-tui

Redmine の Issue データを閲覧、編集するためのターミナル UI のドラフトです。

現在のアプリケーションは Ratatui を使ったローカルプロトタイプです。`datas/` の YAML データを読み込み、UI とドメインモデルの開発用に Issue 詳細画面を描画します。

## 必要なもの

- Rust edition 2024 に対応した Rust ツールチェーン
- ローカル Redmine テストサーバー用の Docker と Docker Compose v2
- Nix 任意、提供されている開発シェルを使う場合

## 開発シェル

Nix を使う場合:

```sh
nix develop
```

このシェルには Rust、Cargo、Clippy、`cargo-insta`、LLVM coverage ツールが含まれます。

## TUI の起動

```sh
cargo run
```

アプリ内に表示されるキー操作:

- `Left` / `Right`: 描画幅を縮小、拡大
- `Up` / `Down`: 描画高さを縮小、拡大
- `q`: 終了

## テスト

```sh
cargo test
```

Snapshot テストには `cargo-insta` を使います。

```sh
cargo insta test
```

Seeder ファイルのチェック:

```sh
bash tests/redmine_seeder_files_test.sh
```

## Docker でローカル Redmine を起動する

このリポジトリには、開発とテストで使うローカル Redmine 用の Docker Compose 構成が含まれています。

Redmine を起動:

```sh
docker compose -f compose.redmine.yml up -d
```

開く URL:

```text
http://localhost:8080
```

Redmine の初期ログイン:

```text
admin / admin
```

Redmine を停止:

```sh
docker compose -f compose.redmine.yml down
```

MySQL データベースとアップロード済みファイルを含めて、Redmine の全データを削除:

```sh
docker compose -f compose.redmine.yml down -v
```

## Redmine 設定

Compose ファイルはローカルテスト専用です。MySQL データと Redmine のアップロードファイルには Docker named volume を使います。

主な環境変数:

- `REDMINE_IMAGE`, デフォルト `redmine:6.1`
- `REDMINE_DB_IMAGE`, デフォルト `mysql:8.0`
- `REDMINE_PORT`, デフォルト `8080`
- `REDMINE_DB_DATABASE`, デフォルト `redmine`
- `REDMINE_DB_USERNAME`, デフォルト `redmine`
- `REDMINE_DB_PASSWORD`, デフォルト `redmine`
- `REDMINE_DB_ROOT_PASSWORD`, デフォルト `redmine-root`
- `REDMINE_SECRET_KEY_BASE`, デフォルト `redmine-tui-local-test-secret`

ホスト側ポートを変える例:

```sh
REDMINE_PORT=18080 docker compose -f compose.redmine.yml up -d
```

## Redmine テストデータの投入

先に Redmine を起動してから、次を実行します。

```sh
cargo xtask seed-redmine
```

このコマンドは `datas/` から SQL を生成し、`compose.redmine.yml` の MySQL service に投入します。先に `docker/redmine/fresh_test_data.sql` を適用するため、ローカルテストデータはリセットされてから seed されます。

DB を変更せずに生成 SQL だけ確認する場合:

```sh
cargo xtask seed-redmine --dry-run
```

現在 seed されるのは、`datas/` に用意されている次の fixture です。

- `datas/projects.yml` の project
- `datas/users.yml` の user
- tracker、status、priority、version、category、time entry activity
- `datas/issues/*.yml` の issue。issue ID は維持されます
- `datas/journals/*.yml` の journal と journal detail

互換用ラッパーも残しています。

```sh
scripts/seed-redmine-test-data.sh
```

## Redmine 参考リンク

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine

## リポジトリ構成

- `src/`: Rust TUI のソースコード
- `xtask/`: Redmine YAML seeding などの Cargo 開発タスク
- `datas/`: ローカル YAML fixture データ
- `compose.redmine.yml`: ローカル Redmine 用 Docker Compose 構成
- `docker/redmine/fresh_test_data.sql`: YAML seed 前に使う Redmine テストデータリセット SQL
- `scripts/seed-redmine-test-data.sh`: seeder 実行ラッパー
- `docs/redmine-test.md`: ローカル Redmine の詳細メモ
