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
scripts/seed-redmine-test-data.sh
```

Seeder は Redmine コンテナ内で Rails runner として実行されます。冪等に作られており、再実行しても同じデータを作成または更新します。

作成、更新されるデータ:

- project: `Redmine TUI Sandbox` / `redmine-tui-sandbox`
- users: `alice.tui`, `bob.tui`
- version: `TUI Test v1.0`
- category: `TUI`
- 親子 Issue を含む 3 件の Issue
- TUI の描画確認用の journal コメントと作業時間

投入されるユーザーのパスワード:

```text
password123
```

## Redmine 参考リンク

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine

## リポジトリ構成

- `src/`: Rust TUI のソースコード
- `datas/`: ローカル YAML fixture データ
- `compose.redmine.yml`: ローカル Redmine 用 Docker Compose 構成
- `docker/redmine/seed_test_data.rb`: Redmine Rails runner seeder
- `scripts/seed-redmine-test-data.sh`: seeder 実行ラッパー
- `docs/redmine-test.md`: ローカル Redmine の詳細メモ
