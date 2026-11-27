# Agent Notes

このファイルのスコープはリポジトリ全体です。

## Project Context

`redmine-tui` は Redmine の Issue データを閲覧・編集する Rust 製 TUI のドラフトです。現在は `datas/` の YAML fixture を読み込むローカルプロトタイプとして開発されています。

アーキテクチャに関する詳細は [docs/architecture.md](docs/architecture.md) を参照してください。このファイルでは同じ内容を重複して説明しません。意思決定の背景は [docs/adrs/](docs/adrs/) も確認してください。

## Useful Commands

- `nix develop`: Rust、Cargo、Clippy、`cargo-insta` などを含む開発シェルに入る
- `cargo run`: TUI を起動する
- `cargo test`: 通常のテストを実行する
- `cargo insta test`: snapshot テストを実行する
- `bash tests/redmine_seeder_files_test.sh`: Redmine seeder fixture の整合性を確認する
- `cargo xtask check-locales`: 日本語のメッセージカタログが英語と同じメッセージと変数を同じ順序で持ち、IDが重複していないかを確認する
- `cargo xtask test-redmine-client`: Docker 上の Redmine に接続する Redmine client テストを実行する
- `cargo xtask test-e2e`: Docker 上の Redmine に接続し、PTY 上で TUI を操作する E2E を実行する
- `cargo xtask coverage`: 単体テストと E2E のカバレッジをまとめたレポートを作成する

ローカル Redmine の起動・停止・seed 手順とテストの実行方法は [CONTRIBUTING.ja.md](CONTRIBUTING.ja.md) を参照してください。

## Working Guidelines

- 変更前に既存の実装パターンとテストの置き方を確認してください。
- アーキテクチャ上の判断に関わる変更は、先に [docs/architecture.md](docs/architecture.md) と関連 ADR を確認してください。
- アーキテクチャ方針を変える場合は、コードだけでなく `docs/architecture.md` の更新、または `docs/adrs/` への新規追加も検討してください。特に `docs/adrs/` の既存ファイルはユーザーからの指示があるまで更新せず、新しい内容は原則新規ファイルに記載してください。
- `docs/` の HTML（`architecture.html`、`adrs/*.html`）は同名の markdown の内容を [docs/assets/template.html](docs/assets/template.html) の骨組みで書き起こし、見た目は `docs/assets/doc.css` に集めてページごとの `<style>` は書かないでください。
- UI 表示の変更では、影響する snapshot を確認してください。snapshot の更新は意図した差分だと判断できる場合に限って行ってください。
- `datas/`、`xtask/src/seed_redmine.rs`、`docker/redmine/fresh_test_data.sql`、`tests/redmine_seeder_files_test.sh` はローカル Redmine テストデータの整合性に関わります。fixture 追加・変更時はあわせて確認してください。
- ドキュメントやコメントを書く・直すときは、置き場所と書き方を [.agents/skills/documentation-layers/SKILL.md](.agents/skills/documentation-layers/SKILL.md) に従って決めてください。
- ディレクトリ固有の規則は、そのディレクトリの `AGENTS.md` にあります。

## Testing

- テストは単体テスト、Redmine client テスト、E2E の3層で書いてください。単体テストかどうかは、組み合わせる struct の数ではなく、テスト対象がひとつかどうかで判断してください。
  - Store、FocusState、Widget、Component、usecase、value object の振る舞いは単体テストで確認し、組み合わせや境界値もここで網羅してください。
  - Redmine API との送受信は Redmine client テストで確認してください。実際の Redmine で起こしにくい応答（5xx、壊れた JSON など）の変換だけは、wiremock を使う単体テストにしてください。
  - ユーザー操作の流れは E2E で確認してください。
  - 画面から観測できない層の間の契約だけを、結合テストとして `src/components/app.rs` と `src/runner/tests.rs` に置いてください。
- 1件のテストが複数の性質を含む場合は、性質ごとに分けて上の層へ置いてください。
- assert の期待値はリテラルまたは `const` で書いてください。入力のコレクションの長さ、パースした entity のフィールド、Store から取り出した値など、定数でない値から期待値を取らないでください。期待値が `const` 同士の計算で決まる場合も、コード上はリテラルで書き、計算方法をコメントに残してください。
- 型で保証されている性質（`Send` であることなど）はテストしないでください。
- 内部の値を確認するためだけのアクセサを足さず、描画結果や spy で確認してください。
- 単体テストは `datas/` の fixture を読まず、依存する値をテストの中か `sample_*` に書いてください。
