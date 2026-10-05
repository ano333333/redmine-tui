# redmine-tui

[English README](README.md) はこちら。

redmine-tui は、ターミナルから Redmine の Issue を閲覧・編集するための TUI のドラフトです。Redmine REST API に接続します。

## 主な機能

- Issue のプロパティ、説明、子 Issue、Journal の閲覧
- status、priority、担当者、日付、category、予定工数などの更新
- 設定したターミナル editor を使った Issue の説明と Journal の編集
- Journal の追加と作業時間の記録
- 変更の Redmine への保存と、編集中にサーバーデータが変わった場合の競合確認

このプロジェクトはまだプロトタイプであり、画面や機能は今後変更される可能性があります。

## 必要なもの

- Rust edition 2024 に対応した Rust ツールチェーン
- ネイティブ版を使う場合、REST API が有効な Redmine と API key
- Nix（任意。提供されている開発シェルを使う場合）

## TUI の起動

API key を設定して起動します。

```sh
REDMINE_API_KEY=<your-api-key> cargo run
```

起動時に Redmine からデータを読み込みます。`REDMINE_API_KEY` が未設定の場合、または初期読み込みに失敗した場合は、エラーを表示して終了します。

接続設定:

- `REDMINE_API_KEY`: Redmine REST API のアクセスキー（必須）
- `REDMINE_URL`: Redmine の base URL（任意。既定値は `http://127.0.0.1:${REDMINE_PORT:-8080}`）
- `REDMINE_PORT`: `REDMINE_URL` 未設定時の fallback port（任意。既定値は `8080`）

既定と異なる URL に接続する例:

```sh
REDMINE_URL=https://redmine.example.com REDMINE_API_KEY=<your-api-key> cargo run
```

## 基本操作

キーボードで操作します。フォーカス中の領域に応じて入力の動作が変わり、実行できる操作は画面内に表示されます。

| キー      | 操作                                                  |
| --------- | ----------------------------------------------------- |
| `j` / `k` | 項目やフィールドを下／上に移動                        |
| `h` / `l` | 対応する画面でパネルや列を左右に移動                  |
| `e`       | フォーカス中のフィールドやテキストを編集              |
| `y`       | Issue 選択画面を開く                                  |
| `Ctrl-S`  | Issue または Journal の変更を Redmine に保存          |
| `Enter`   | 選択を確定                                            |
| `q`       | メイン画面で終了、またはフォーカス中の popup を閉じる |

テキスト編集時は、設定された外部 editor が起動します。popup ごとに操作が異なる場合があるため、画面内の案内を確認してください。

## 基本的な使い方

1. Redmine API key を指定して起動します。
2. `y` で Issue 選択画面を開き、`j` / `k` で移動して `Enter` で Issue を開きます。
3. `j` / `k` で Issue のフィールドや Journal を移動し、`e` で編集します。
4. `Ctrl-S` で Redmine に保存します。競合が検出された場合は、競合画面でサーバー側とローカルの値を確認します。

## 開発について

開発環境の準備、ローカル Redmine の利用方法、テストコマンド、リポジトリの開発方針は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
