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

ローカル Redmine の起動・停止・seed 手順は [docs/redmine-test.md](docs/redmine-test.md) を参照してください。

## Working Guidelines

- 変更前に既存の実装パターンとテストの置き方を確認してください。
- アーキテクチャ上の判断や component lifecycle に関わる変更は、先に [docs/architecture.md](docs/architecture.md) と関連 ADR を確認してください。
- アーキテクチャ方針を変える場合は、コードだけでなく `docs/architecture.md` の更新、または `docs/adrs/` への新規追加も検討してください。特に `docs/adrs/` の既存ファイルはユーザーからの指示があるまで更新せず、新しい内容は原則新規ファイルに記載してください。
- UI 表示の変更では、影響する snapshot を確認してください。snapshot の更新は意図した差分だと判断できる場合に限って行ってください。
- `datas/`、`docker/redmine/seed_test_data.rb`、`tests/redmine_seeder_files_test.sh` はローカル Redmine テストデータの整合性に関わります。fixture 追加・変更時はあわせて確認してください。
