# redmine-tui

Redmine の Issue データを閲覧、編集するためのターミナル UI のドラフトです。

現在のアプリケーションは Ratatui を使ったローカルプロトタイプです。コンテナの Redmine サーバーのデータを読み込み、 Issue/Journal の更新ができます。

Web デモは同じ端末 UI をブラウザに描画します（Web デモの節を参照）。

## 必要なもの

- Rust edition 2024 に対応した Rust ツールチェーン
- ローカル Redmine テストサーバー用の Docker と Docker Compose v2
- Nix 任意、提供されている開発シェルを使う場合

## 開発シェル

Nix を使う場合:

```sh
nix develop
```

このシェルには Rust、Cargo、Clippy、`cargo-insta`、LLVM coverage ツール、Trunk、actionlint が含まれます。

## TUI の起動

```sh
cargo run
```

TUI は起動直後に Redmine から初期 Entity を読み込むため、
`REDMINE_API_KEY` を必要とします。必須の環境変数がない場合、または初期
読み込みに失敗した場合は、エラー理由を表示して終了します。

```sh
REDMINE_API_KEY=0123456789abcdef0123456789abcdef01234567 cargo run
```

TUI 接続用の環境変数:

- `REDMINE_API_KEY`: 必須。Redmine REST API のアクセスキー。起動時に
  user、status、priority、project、tracker、version、category、
  time entry activity を Redmine から読み込みます。
- `REDMINE_URL`: Redmine の base URL。未指定の場合、
  `http://127.0.0.1:${REDMINE_PORT:-8080}` を使用します。
- `REDMINE_PORT`: `REDMINE_URL` 未指定時だけ使われる fallback port。

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

### Redmine client テスト

実際の Redmine に接続するテストは `src/clients/redmine/default_tests/container/` に置き、`container-tests` feature を有効にしたときだけ compile します。`xtask` から実行し、Docker が必要です。

```sh
cargo xtask test-redmine-client
```

このコマンドは、一意な project 名とランダムなホスト側ポートで Redmine を Docker Compose で1つ起動し、module 内のテストを1回の `cargo test` で直列に実行します。各テストは開始時に seed を入れ直すため、他のテストの変更に依存しません。seed は `AUTO_INCREMENT` もリセットするため、API で作成したデータの ID は毎回同じになります。終了時に container と volume を削除します。

接続先は `xtask` が設定する `REDMINE_TUI_TEST_BASE_URL` と `REDMINE_TUI_TEST_PROJECT_NAME` から読みます。`cargo test --features container-tests` で直接実行すると、上のコマンドを案内するメッセージで失敗します。`wiremock` を使うテストは、実際の Redmine では起こしにくい応答の検証に限り、通常の `cargo test` で実行します。

### E2E

E2E のシナリオは `tests/e2e/` に置き、`e2e-tests` feature を有効にしたときだけ compile します。`xtask` から実行し、Docker が必要です。

```sh
cargo xtask test-e2e
```

このコマンドは Redmine client テストと同じ方法で Redmine を起動し、`e2e` test target を直列に実行します。各シナリオは seed を入れ直し、`testty` で native バイナリを PTY 上で起動して、画面と Redmine API の両方で結果を確認します。テキストを編集するシナリオは、`VISUAL` に指定した偽の editor で editor を置き換えます。

`.github/workflows/ci.yml` は次の job を並列に実行します。`unit`（`cargo fmt --check`、`cargo build --workspace`、`cargo test --workspace`、seeder ファイルのチェック）、`redmine-client`（`cargo xtask test-redmine-client`）、`e2e`（`cargo xtask test-e2e`）、`web`（wasm32 の `cargo build`、`trunk build`）です。

## Web デモ

Web 版は Ratzilla で端末 UI をブラウザに描画し、fixture を埋め込んだ memory mock（`DemoRedmineClient`）で動きます。実 Redmine には接続せず、API key も不要です。編集内容はメモリ上だけにあり、ページを再読み込み・離脱すると消えて fixture の状態に戻ります。

```sh
nix develop -c trunk serve --port 8081
```

`http://127.0.0.1:8081/` を開きます（既定の 8080 はローカル Redmine が使うため別 port にします）。

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
- `Developer` role、全 fixture user の全 project へのメンバー登録、全 tracker について全 status 間を遷移できる workflow。API で担当者と status を変更するために必要です
- API key `0123456789abcdef0123456789abcdef01234567` による Redmine admin user の REST API アクセス

既定以外の Docker Compose project 名に seed する場合は、project 名を渡します。

```sh
cargo xtask seed-redmine --project-name redmine-tui-client-test
```

互換用ラッパーも残しています。

```sh
scripts/seed-redmine-test-data.sh
```

## Redmine 参考リンク

- Redmine install guide: https://www.redmine.org/projects/redmine/wiki/redmineinstall
- Docker official Redmine image: https://hub.docker.com/_/redmine

## リポジトリ構成

- `src/`: Rust TUI のソースコード
- `tests/e2e/`: PTY 上で操作する E2E シナリオ
- `xtask/`: Redmine YAML seeding、Pages build などの Cargo 開発タスク
- `index.html`: Web build の Trunk 入口 HTML
- `Trunk.toml`: Web build の Trunk 設定
- `datas/`: ローカル YAML fixture データ
- `compose.redmine.yml`: ローカル Redmine 用 Docker Compose 構成
- `docker/redmine/fresh_test_data.sql`: YAML seed 前に使う Redmine テストデータリセット SQL
- `scripts/seed-redmine-test-data.sh`: seeder 実行ラッパー
- `.github/workflows/`: CI と GitHub Pages の workflow
