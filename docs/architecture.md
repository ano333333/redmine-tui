# Agent 実装ガイド

この文書は、agent が本リポジトリを読む・変更する際のアーキテクチャ前提をまとめる。
詳細な意思決定の背景は `docs/adrs/` を参照する。

## ディレクトリ構成

- `src/main.rs`
  - terminal 初期化、main loop、最上位 layout を持つ。
  - loop は `AppContainer::update`、draw、crossterm event read、`AppContainer::handle_key_event` の順に進む。
- `src/app_container.rs`
  - terminal と application component の接続点。
  - `Dispatcher` と `AppComponent` を保持する。
  - editor 起動など terminal 外部副作用を扱う。
- `src/app.rs`
  - `Action`、`Dispatcher`、`Store` を定義する。
  - Store は entity と差分状態を保持する。
- `src/components/`
  - TUI の画面部品を置く。
  - `app.rs` は全体 component と popup stack を統括する。
  - `issue/` は issue detail 画面とその子 component を置く。
  - `*_popup/` は popup component を置く。
- `src/entities/`
  - Redmine 由来の永続的な domain entity を置く。
- `src/vos/`
  - ID、差分、journal detail などの value object を置く。
- `src/widgets/`
  - 複数 component から使う汎用 widget を置く。
- `src/libs/`
  - YAML読み込みなど、外部表現から domain data へ変換する補助処理を置く。
- `src/test_support.rs`
  - snapshot rendering や test fixture を置く。
- `src/snapshots/`
  - insta snapshot を置く。
- `docs/adrs/`
  - アーキテクチャ判断の記録を置く。

## Flux を参考にした構成

本リポジトリは厳密なFlux実装ではない。
Action を Dispatcher へ送り、Dispatcher が Store を更新し、Store 更新後に Component を update する、という一方向データフローを基本にした構成である。

Store の更新は原則として Dispatcher を介して行う。
初期セットアップを除き、action の処理と component の `update` は交互に行う。
これは意図した lifecycle であり、全 action を drain してから一度だけ update する設計ではない。

厳密なFluxとの差分として、以下を許容する。

- Store 更新通知は pub/sub ではなく、上位層が `consume_action -> update` を明示的に呼ぶ。
- `Dispatcher` は action queue と `Store` を内部に持つ。
- focus、cursor、scroll、render cache などの同期的な UI state は Store ではなく Component / FocusState に保持する。
- 親子 Component 間の focus 遷移は Store / Action を経由せず、`process_event` の戻り値と `focus_event` で直接処理する。
- editor 起動などの外部副作用は `AppEffect` として Component から取り出し、`AppContainer` 側で実行する。
- `create_widget(&Store)` で Store を参照して表示用 entity を取得してよい。

## Component lifecycle

Component は以下の lifecycle を前提に実装する。

1. Store action 処理時
   - `Dispatcher::consume_action`
   - `Store` 更新
   - `Component::update`
   - `Component::create_widget`
   - `Widget::render`
2. 同期的なキーイベント処理時
   - `Component::process_event`
   - 必要に応じて action dispatch、popup open/close、effect request
   - `Component::update`
   - `Component::create_widget`
   - `Widget::render`
3. 他 Component のキーイベント処理による focus 遷移時
   - focused child の `process_event`
   - parent が別 child の `focus_event` を呼ぶ
   - `Component::update`
   - `Component::create_widget`
   - `Widget::render`

`create_widget` は `Widget` を実装した concrete struct を返す。
`Option<Widget>` にはしない。

`create_widget(&Store)` は許容する。
主な用途は Store から描画に必要な entity を取得することである。
幅依存の buffer、markdown render cache、scroll/focus 補正などの重い派生状態は `update` 側で扱う。

`src/components/issue/` 配下の component では、`create_widget` 時点で対象 issue が Store に存在することを設計上の不変条件とする。
この不変条件に依存する箇所では、無言の `unwrap()` ではなく、不変条件を説明する `expect(...)` を使う。

```rust
let (issue, _) = store
    .get_issue(self.id)
    .expect("PropertyComponent requires its issue to exist in Store");
```

## Component の実装分割

Component は原則として `widget.rs`、`focus_state.rs`、`component.rs` に分割する。
focus を持たない component では `focus_state.rs` を省略してよい。

### Widget

Widget は表示に直接関わるデータのみを受け取り、各 frame での render を行う。

Widget の責務:

- `ratatui::widgets::Widget` を実装する。
- `render(self, area, buf)` で Buffer へ描画する。
- 表示に必要な文字列、数値、選択状態、focus状態、事前計算済み buffer を受け取る。
- layout と style を決める。
- snapshot test の対象になる表示を作る。

Widget に入れない責務:

- crossterm key event の解釈。
- Store action の dispatch。
- focus 遷移の決定。
- 外部I/O。
- 重い永続データ取得。

Widget は必要なら `line_count` など表示に密接な計算メソッドを持ってよい。
ただし、Component や FocusState と同じ計算を重複させない。

### FocusState

FocusState はキー入力とその処理に関わるデータのみを持つ。
フォーカス位置計算、カーソル表示位置計算、focus 入退場の同期処理を行う。

FocusState の責務:

- `process_event` で key event を解釈する。
- `focus_event` で親 component からの focus 入退場を処理する。
- `update` で focus 可能範囲、幅、高さ、行数などを最新状態へ補正する。
- cursor position を返す。
- 上下左右移動、境界到達、編集開始などを `EventProcessResult` として返す。

FocusState に入れない責務:

- Store 参照。
- Widget 生成。
- entity の保持。
- 表示文字列の組み立て。
- action dispatch。

### Component

Component は Widget と FocusState を統括し、lifecycle に関わるメソッドをある程度統一した形式で公開する。

Component の責務:

- `new` で component 固有 state を初期化する。
- `process_event` で FocusState や子 component へ event を委譲し、親へ返す同期イベントを決める。
- `focus_event` で親からの focus 遷移を受け取る。
- `update` で Store や entity から component state、FocusState、render cache を更新する。
- `create_widget` で Widget を構築する。
- 子 component を持つ場合は、子の `process_event` / `focus_event` / `update` / `create_widget` を統括する。

Component は Store から entity を取得して Widget に参照を渡してよい。
ただし、Component field に frame をまたぐ entity 参照を保持しない。
entity の owned copy を field に持つ場合は、Store との二重管理と clone cost が妥当かを先に検討する。

## テスト方針

Widget、FocusState、Component は責務ごとにテストする。

### Widget test

Widget は単体テストを書く。

確認すること:

- render結果の snapshot。
- layout、clip、wrap、style、focus表示。
- `line_count` など表示計算。

Widget test は Store や Dispatcher に依存させない。
必要な表示データを直接渡す。

### FocusState test

FocusState は単体テストを書く。

確認すること:

- key event による focus/cursor 移動。
- 境界到達時の `EventProcessResult`。
- `focus_event` による入退場。
- `update` による範囲補正。
- unfocused 時に入力を無視すること。

FocusState test は Widget や Store に依存させない。

### Component test

Component は Widget と FocusState の結合を確認する結合テストを書く。

確認すること:

- `process_event` 後に `create_widget` の表示状態へ反映されること。
- `focus_event` 後に Widget の focus 表示と cursor 位置が一致すること。
- `update` 後に FocusState の補正、Widget の表示、line count が整合すること。
- 子 component を持つ component では、子から親への `EventProcessResult` と親から別子への `focus_event` がつながること。

Component test では必要に応じて Store fixture を使ってよい。
表示の最終確認には snapshot test を使う。
