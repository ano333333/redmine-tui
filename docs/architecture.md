# Agent 実装ガイド

この文書は、agent が本リポジトリを読む・変更する際のアーキテクチャ前提をまとめる。
詳細な意思決定の背景は `docs/adrs/` を参照する。

## ディレクトリ構成

- `src/main.rs`
  - Cargo feature で native/Web の entry point を選ぶ。feature と target が不一致（`native` と `web-demo` の同時指定、非 wasm32 target での `web-demo` 等）なら `compile_error!` で compile 不能にする。
- `src/entry/`
  - native/Web の起動処理を置く。
- `src/stores/`
  - `store.rs` は、子 Store ・子 Action の統合を行う。外部からはこのファイルからエクスポートされる Store と Action を公開インターフェースとして用いる。
- `src/usecases/`
  - アプリ固有の操作を置く。Store・Client の情報統合、および同期的な Dispatch や非同期タスクによる Action の形成を担う。
  - 非同期 usecase について、同期的な Action dispatch はここで即座に行い、非同期で形成する Action は `Future` として返却する形が基本形である。`Store` を直接書き換えず、`Dispatcher::dispatch` を介して Action を積む。`runner` が Future を platform の runtime port（native は Tokio、Web は `spawn_local`）で起動し、完了 Action を Dispatcher へ戻す。
- `src/clients/`
  - 外部プロセスとの通信を行う。
  - `redmine/base.rs` は `RedmineClient` trait を定義し、 Redmine との通信のインターフェースを定義する。`redmine/default.rs` は `DefaultRedmineClient`（実 HTTP 実装）を定義する。
  - Issue 属性の保存は `IssueUpdate`（編集した属性だけを持つ更新要求）で渡す。送信しない属性は `None`、値の解除は `Some(None)` で表し、API が要求する項目の省略や空文字への変換は HTTP 実装が担う。
  - `redmine/demo/` は `DemoRedmineClient`（実 HTTP を行わず fixture を埋め込む memory mock）を定義する。
- `src/components/`
  - TUI の画面部品を置く。
  - `app.rs` は全体 component と popup stack を統括する。
  - `issue/` はIssue取得状態を解決する外側componentを置き、`issue/detail/` は読み込み済みIssueの詳細画面とその子componentを置く。
  - `*_popup/` は popup component を置く。
- `src/platform/`
  - native/Web の platform adapter を置く。
  - input（`InputEvent`）、runtime（`BackgroundSpawner`）、host（`PlatformHost`）、editor（`TextEditor`）の各 port。
- `src/runner/`
  - native/Web で共有する application lifecycle を置く。
- `src/entities/`
  - Redmine 由来の永続的な domain entity を置く。
  - `IssueAggregate::with_property_diffs` は、Issue 属性の差分を適用した値か、競合した差分を返す。差分の `before`/`after` を現在値へ置き換える処理とあわせて、Store と usecase から共用する。
- `src/vos/`
  - ID、差分、journal detail などの value object を置く。
  - `issue_property_diff.rs` は同じ属性への複数の差分の集約を持つ。
- `src/widgets/`
  - 複数 component から使う汎用 widget を置く。
- `src/libs/`
  - YAML読み込みなど、外部表現から domain data へ変換する補助処理を置く。
- `src/logging.rs`
  - `initialize_logging`（native は file、Web は browser console）と `trace_dbg!` などの補助マクロを置く。
- `src/test_support.rs`
  - snapshot rendering や、テスト用のentityを組み立てる`sample_*`を置く。
- `src/snapshots/`
  - insta snapshot を置く。
- `docs/adrs/`
  - アーキテクチャ判断の記録を置く。

## ディレクトリ間の依存

矢印は依存する側から依存される側への向き。
上の層は下の層に依存してよく（層を飛ばしてもよい）、下の層から上の層への依存はない。
層内の依存は個別の矢印で表す。
`src/clients/` のうち `usecases` が使うのは `redmine/base.rs` の `RedmineClient` trait などのインターフェース定義だけで、これを application に置く。実装（`default.rs` の `DefaultRedmineClient`、`demo/` の `DemoRedmineClient`）と外部表現の変換（`src/libs/`）は `adapter` に置き、実装は起動処理が組み立てて渡す。
例として、`runner` は `components`・`usecases`・`stores`・`clients`・`vos` に、`platform` は completion を `Action` として dispatch するため `stores` に依存する。

```mermaid
flowchart TD
    subgraph layer_boot["起動・lifecycle"]
        main["main.rs"] --> entry["src/entry/"]
        entry --> runner["src/runner/"]
        entry --> logging["src/logging.rs"]
        runner --> logging
    end

    subgraph layer_adapter["adapter"]
        client_impls["src/clients/<br/>（impl）"] --> libs["src/libs/"]
    end

    subgraph layer_ui["UI・platform"]
        components["src/components/"] --> widgets["src/widgets/"]
        components --> platform["src/platform/"]
    end

    subgraph layer_app["application"]
        usecases["src/usecases/"] --> stores["src/stores/"]
        usecases --> client_trait["src/clients/<br/>（trait）"]
    end

    subgraph layer_domain["domain"]
        entities["src/entities/"] --> vos["src/vos/"]
    end

    layer_boot --> layer_ui
    layer_ui --> layer_app
    layer_adapter --> layer_app
    layer_adapter ~~~ layer_ui
    layer_app --> layer_domain
```

`usecases` は `components` に依存しない。
Component から usecase の関数を呼ぶことはあるが（例: `app.rs` が `issue_popup_options` や `redmine::{cancel_issue_upload, continue_issue_upload}` を呼ぶ）、逆方向の依存は発生させない。
同様に `clients` は `stores` にも `usecases` にも依存せず、`RedmineClient` trait と HTTP 実装（`DefaultRedmineClient`）、`DemoRedmineClient` を提供する。`DemoRedmineClient` の fixture は `src/libs/yaml.rs` の parser を使うため `clients` から `libs` への依存がある。

## Flux を参考にした構成

本リポジトリは厳密なFlux実装ではない。
Action を Dispatcher へ送り、Dispatcher が Store を更新し、Store 更新後に Component を update する、という一方向データフローを基本にした構成である。

Store の更新は原則として Dispatcher を介して行う。
初期セットアップを除き、action の処理と component の `update` は交互に行う。
これは意図した lifecycle であり、全 action を drain してから一度だけ update する設計ではない。

厳密なFluxとの差分として、以下を許容する。

- Store 更新通知は pub/sub ではなく、上位層が `consume_action -> update` を明示的に呼ぶ。
- `Dispatcher` は action queue と `Store` を内部に持つ。
- 親 `Store` は Issue と Journal の状態と更新処理を非公開の `IssueStore` に委譲する。Journal 本体は `IssueAggregate::journals` が所有し、`IssueStore` は取得済み Issue ごとに全 Journal の `RemoteJournalState`、0 件または 1 件の Local Journal、取得結果から消えた編集中 Journal の退避データ（DeletedJournal）を持つ。Journal の操作は Issue が取得済みの場合だけ受理する。
- Issue 詳細の取得結果は `Action::IssueFetchSucceeded` 1件で Issue、Journal、子一覧を反映する。`IssueStore` は Journal の所有関係と重複を検査してから登録し、一部だけを反映した状態を作らない。
- `IssueAggregate` は親 Issue の ID だけを持ち、子 Issue の ID 一覧は持たない。子一覧は詳細取得で得た `IssueChild`（ID・トラッカー・題名・再帰的な子一覧）として `IssueStore` が Issue ごとに保持する。子の詳細を取得済みなら、表示には `IssueView` の値を使う。
- Issue 属性の状態（Synced / Edited / Uploading）は Journal の編集と下書きを含まない。Issue 属性の状態が変わっても Journal の作業は引き継ぐ。同じ Issue の upload は Issue 属性と Journal を合わせて1件に限り、`IssueStore` が検査する。
- Component と usecase は `IssueStore` を直接参照せず、親 `Store` の Issue getter を通して entity、同期状態、diff、競合情報を取得する。
- focus、cursor、scroll、render cache などの同期的な UI state は Store ではなく Component / FocusState に保持する。
- 親子 Component 間の focus 遷移は Store / Action を経由せず、`process_event` の戻り値と `focus_event` で直接処理する。
- editor 起動、Redmine への非同期取得・保存などの外部副作用は `AppEffect` として Component から取り出し、`runner` 側で実行する。Redmine 関連の `AppEffect` は `usecases::redmine` の関数を platform の runtime port（native は Tokio、Web は `spawn_local`）で spawn し、完了 Action を Dispatcher に戻す。
- `create_widget(&Store)` で Store を参照して表示用 entity を取得してよい。

Store は、失敗または Action の不受理に見える分岐を以下に区別して扱う。

- 異常系: 自プロセスの制御破綻を示す状態機械違反。`panic!` で即座に停止する。異常系を `Result` で呼び出し元へ返すのは、Flux を参考にした一方向データフローでは dispatch 時点と consume 時点が分離しておりエラーを返す先がないため採用しない。
- 準異常系: 外部プロセスや外部データ起因の復帰可能な失敗。message を Action に載せ、状態復帰と notice によるユーザー通知を行う。失敗後の再試行に必要な状態がある場合は、失敗 Action によって対象の状態機械を再試行可能な状態へ戻し、message を状態の一部として保持する。
- stale completion: 重複を許した非同期要求の追い越し。request ID の一致判定で破棄し、暗黙の状態判定では破棄しない。現時点でこれに該当するのは `ProjectIssuesStore` のみ。`IssueAction` と `JournalAction` は重複を事前条件で排除するため、想定した状態以外へ着弾した完了は stale completion として捨てず異常系として拒否する。
- マージ戦略: サーバー由来のデータをローカルへ取り込む際、ローカル編集を保護するために更新を適用しない意図的な no-op。取得した Journal を取り込む際に、未送信の編集差分を取得値で置き換えないことがこれにあたる。編集中の Journal は本体だけを取得値に更新して `diff.before` を残し、サーバーの notes が `after` と同じなら Synced にする。notes の競合はその Journal を保存するときの取得で判定する。upload 中の Journal は取得値で上書きしない。取得結果から消えた編集中の Journal は元の ID と編集後の notes で退避し、同じ ID が再び現れたらサーバーの notes からの編集として戻す。退避した Journal は利用者が個別に新規投稿するか破棄する。
- Issue 属性の保存は、保存前の取得・PUT・確認の取得を順に行う。保存前の取得で得た Journal と子一覧は、Issue 属性の競合の有無にかかわらず取り込む。確認の取得だけが失敗した場合は PUT を繰り返さず、送信した差分を適用した値を新しい基準値にする。
- Remote Journal の保存も、保存前の取得・PUT・確認の取得を順に行う。各取得の結果は Issue 本体・他の Journal・子一覧ごと取り込み、Issue 属性と他の Journal の編集差分は残す。保存前の取得から対象が消えていれば、旧 ID へ PUT せずに退避する。競合した場合は対象の本体を取得値で上書きせず、取得した notes を競合情報として保持する。確認の取得だけが失敗した場合は PUT を繰り返さず、保存前の取得値の対象 notes を送信した値にして完了する。
- 冪等 no-op: 同じ `NoticeId` の再追加など、Action 自体が冪等であることを契約として持つ正常な no-op。stale completion とマージ戦略は同じ no-op の見た目になりやすいため独立して扱う。

getter 契約は、API が表す状態と cardinality で決める。不在が示す意味が異なるため、entity の種類だけで一律には決めない。

- strict 単体取得: 存在が呼び出し元の事前条件である getter は `get_xxx` とし、参照を直接返し、不在は異常系として `panic!` する。
- 状態・cardinality を表す `Option`: 読み込み状態、ページの未要求、0 件・1 件など、不在そのものが状態や cardinality を表す取得は `Option` を返す。`IssueStore` の単体 getter では、`Option` を返すものを `try_get_xxx` と命名する。呼び出し側が取得値の存在を特定の経路で前提する場合は、無言の `unwrap()` ではなく `expect(...)` で不変条件を説明する。
- master snapshot の `Option`: 起動時に一度だけ同期するマスターデータ（`IssueStatus` など）は、起動後に取得した Issue や Journal がスナップショットに存在しない ID を参照し得るため陳腐化で欠損し得る。単体のマスターデータ getter は `Option` を返し、呼び出し元は表示上の fallback で処理する。

Issue の getter は、取得済みの本体と読み込み状態を分けて扱う。

```rust
enum IssueState { Synced, Edited, Uploading }
enum IssueFetchState { Fetching, FetchFailed { message: String } }

fn get_issue(&self, id: IssueId) -> (IssueView<'_>, IssueState);
fn try_get_issue_state(&self, id: IssueId) -> Option<IssueState>;
fn try_get_issue_fetch_state(&self, id: IssueId) -> Option<IssueFetchState>;
```

| 内部状態                    | `try_get_issue_state` | `try_get_issue_fetch_state` | `get_issue` |
| --------------------------- | --------------------- | --------------------------- | ----------- |
| 未登録                      | `None`                | `None`                      | panic       |
| Fetching                    | `None`                | `Some(Fetching)`            | panic       |
| FetchFailed                 | `None`                | `Some(FetchFailed)`         | panic       |
| Synced / Edited / Uploading | `Some(..)`            | `None`                      | 表示値と状態 |

- 本体の存在が不変条件である経路は `get_issue` を直接使い、不在を事前検査して処理をスキップしない。
- 子 Issue や親 Issue の表示など不在が正常な経路では、`try_get_issue_state(id).is_some()` を確認してから `get_issue` を使う。
- 未登録は両方の状態 getter が `None` の場合であり、`try_get_issue_fetch_state` の `None` だけで判定しない。

`IssueStore` が保持する `IssueAggregate` はサーバーから取得した基準値であり、Issue 属性の編集 Action では変更しない。編集は、その時点の表示値を `before` にした `IssuePropertyDiff` を編集した順に追加して表す。差し引きで変更がなくなった場合は Synced へ戻す。

`get_issue` が返す `IssueView` は、属性ごとに最後の diff の `after`、diff がなければ基準値を Store の寿命で参照する。保存時に送る値は `IssueAggregate::with_property_diffs` で同じ規則により求める。

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

`src/components/issue/detail/` 配下の component では、`create_widget` 時点で対象 issue が Store に存在することを設計上の不変条件とする。
外側の `src/components/issue/IssueComponent` は未取得、取得中、取得失敗も扱い、この不変条件を満たす状態でだけ detail component を生成する。
この不変条件に依存する箇所では、strict getter の `get_issue` を直接使い、不在時の panic で契約違反を検出する。

```rust
let (issue, _) = store.get_issue(self.id);
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
- 戻り値`Option<EventProcessResult>`は、`None`をイベントを解釈しなかった（未処理）、`Some(Handled)`をイベントを使ったが親への要求はない、それ以外を親への要求とする。端で動かなかった場合も、解釈したキーなら`Some(Handled)`を返す。Componentも同じ意味で返す。
- 親は子の`Handled`をそのまま`Handled`で返す。子の`CursorLeavedFrom*`を受けて隣の子へフォーカスを移したら`Handled`を返す。移せない場合、中間の親は`CursorLeavedFrom*`を外へ返し、最上位のComponent（`IssueDetailComponent`）は`Handled`を返す。`AppComponent`はどのComponentも使わなかった`q`だけでアプリを終了する。

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

テストは単体テスト、外部プロセスへのアダプターのテスト、E2Eの3層で構成する。内部で閉じるstruct同士の結合テストは、画面から観測できない層の間の契約に限って残す。

| 層                                 | 対象                                                                  | 実行方法                          |
| ---------------------------------- | --------------------------------------------------------------------- | --------------------------------- |
| 単体テスト                         | Store、FocusState、Widget、Component、usecase、value object、変換処理 | `cargo test`                      |
| 外部プロセスへのアダプターのテスト | `DefaultRedmineClient`とRedmine containerの契約                       | `cargo xtask test-redmine-client` |
| E2E                                | PTY上のnativeバイナリによるユーザー操作の流れ                         | `cargo xtask test-e2e`            |

### どの層でテストするか

単体テストかどうかは、組み合わせるstructの数ではなく、テスト対象がひとつかどうかで判断する。Storeを入力として組み立て、Componentの描画、`process_event`の戻り値、発行したActionを検証するテストは、Componentを対象とする単体テストである。

変更の内容ごとに、次の層でテストする。

- Store、FocusState、Widget、Component、usecase、value objectの振る舞い: 単体テスト。組み合わせや境界値はここで網羅する。
- Redmine APIとの送受信（requestの形式、responseの変換、Redmineがどう反映するか）: Redmine clientテスト。実際のRedmineで起こしにくい応答（5xx、壊れたJSONなど）の変換だけは、wiremockを使う単体テストにする。
- ユーザー操作の流れ（popupでの選択、編集、upload、競合の解決、画面の切り替えなど）: E2E。
- 画面から観測できない層の間の契約: 結合テストとして`src/components/app.rs`と`src/runner/tests.rs`に置く。

1件のテストが複数の性質を含む場合は、性質ごとに分けて上の層へ置く。たとえばpopupの選択肢の組み合わせは単体テストに、選んだ値がIssueに反映される流れはE2Eに置く。

### 期待値の書き方

assertの期待値はリテラルまたは`const`で書く。入力と実装が同時に誤っても検出できるよう、期待値を入力や実装の出力から取らない。

- 入力と期待値で同じ`const`を使ってよい。どちらも定数のため、片方だけがずれることはない。
- 入力のコレクションの長さ、パースしたentityのフィールド、Storeから取り出した値など、定数でない値から期待値を取らない。
- 期待値が`const`同士の計算で決まる場合（seedのIssueが3件なので次に作成されるIDは4、など）も、コード上はリテラルで書き、計算方法をコメントに残す。
- 型で保証されている性質（`Send`であることなど）はテストしない。

### テストデータ

- 単体テストでは、Storeへ`test_support`の`sample_*`で組み立てたentityをSyncで渡す。テストが依存する値はテストの中か`sample_*`に書き、`datas/`のfixtureは読まない。`datas/`はDemo clientとRedmine seederの入力である。
- Redmine clientテストとE2Eは、テストごとに`datas/`からseedを入れ直した状態で始まる。seedには全テストで共通の土台（マスターデータ、project、role、workflowなど）だけを入れる。
- テスト固有のデータは、E2EではGivenの段階でRedmine APIを使って追加する。他のユーザーによる更新や競合も、APIによる更新として書く。seedの再投入はAUTO_INCREMENTもリセットするため、APIで作成したデータのIDは毎回同じになる。

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
- 解釈したキーで`Some(_)`、解釈しないキーで`None`を返すこと。
- `focus_event` による入退場。
- `update` による範囲補正。
- unfocused 時に入力を無視すること。

FocusState test は Widget や Store に依存させない。

### Component test

Component は Widget と FocusState の組み合わせを、Component を対象とする単体テストで確認する。

確認すること:

- `process_event` 後に `create_widget` の表示状態へ反映されること。
- `focus_event` 後に Widget の focus 表示と cursor 位置が一致すること。
- `update` 後に FocusState の補正、Widget の表示、line count が整合すること。
- 子 component を持つ component では、子から親への `EventProcessResult` と親から別子への `focus_event` がつながること。

Component test では必要に応じて Store を入力として使ってよい。
表示の最終確認には snapshot test を使う。

### Redmine clientテスト

`src/clients/redmine/default_tests/container/`に置き、`container-tests` featureを有効にしたときだけcompileする。`cargo xtask test-redmine-client`がDocker ComposeでRedmineを1つ起動し、各テストは開始時にseedを入れ直す。正常系と実際に起こせるエラー（401、404、422）はcontainerで検証し、wiremockは5xxや壊れたJSONなど実際のRedmineで起こしにくい応答の変換に限る。

### E2E

`tests/e2e/`に置き、`e2e-tests` featureを有効にしたときだけcompileする。`cargo xtask test-e2e`がRedmine containerを起動し、testtyでnativeバイナリをPTY上で操作する。

- シナリオはテスト関数ごとに固定し、Gherkinの`// Scenario:`、`// Given`、`// When`、`// Then`をコメントで書く。
- 各シナリオは開始時にseedを入れ直す。シナリオ固有のデータはGivenでRedmine APIを使って追加する。
- 結果は画面表示と、Redmine APIで取得し直した状態で検証する。リクエスト回数はE2Eで検証しない。
- editorは`VISUAL`に指定した`FakeEditor`で置き換える。testtyの仮想端末はcursor位置の問い合わせに応答しないため、`FakeEditor::finish_editing`が応答を書き込む。

## Platform境界（native/Web）

GitHub Pages向けWebデモをRatzilla `DomBackend`で配信するため、Store、Component、Widget、entity、value object、Redmine usecaseの状態遷移をnative/Webで共有し、platform差はアプリケーションの入口と外部副作用のadapterへ閉じ込める。設計根拠は[ADR 10](adrs/010.md)を参照する。

```mermaid
flowchart TD
    input["InputEvent"] --> component["Component"]
    component -->|effect| runner["App runner"]
    runner --> ports["platform ports<br/>Redmine・Editor・Runtime・Logging"]
    ports -->|completion| store["Dispatcher/Store"]
    component --> action["Action"]
    action --> store
```

| port    | native          | Web                       |
| ------- | --------------- | ------------------------- |
| 入力    | crossterm       | Ratzilla `DomBackend`     |
| Runtime | Tokio           | `spawn_local`             |
| Redmine | HTTP            | memory mock               |
| Editor  | external editor | textarea                  |
| Logging | file            | browser console           |
| Cursor  | terminal cursor | cursor位置のセルをreverse |

### 実装上の規則

- Componentはcrosstermではなく`src/platform/input/`の`InputEvent`を受け取る。
- 共通層（components・widgets・stores・usecases・entities・vos）はcrossterm、Ratzilla、DOM、Tokio、filesystem、process、HTTP実装へ直接依存しない。
- 外部副作用は`AppEffect`としてrunnerに渡し、runnerがplatformのportで実行する（runtime handle や executor 固有型を Component、Store、Client に渡さない）。
- `InteractionMode::Editing`中はComponentへの入力配送を止める。

Webは`web-demo` featureとwasm32 targetでbuildする。ローカルでの確認手順は[README.ja.md](../README.ja.md)を参照する。GitHub Pagesへの配信は`.github/workflows/pages.yml`が`cargo xtask build-pages`でbuildして行う。
