# components

このファイルのスコープは `src/components/` です。

## Componentの分割

- Componentは原則として `widget.rs`、`focus_state.rs`、`component.rs` に分けてください。focusを持たないComponentでは `focus_state.rs` を省略して構いません。
- Widgetには表示に直接使うデータだけを渡し、layout、style、`line_count` などの表示の計算をさせてください。キー入力の解釈、Actionのdispatch、focus遷移の決定、外部I/Oは入れないでください。
- FocusStateにはキー入力の解釈、focusの入退場、focus可能な範囲やcursor位置の計算をさせてください。Storeの参照、Widgetの生成、entityの保持、表示文字列の組み立て、dispatchは入れないでください。
- WidgetとFocusStateで同じ計算を重複させないでください。
- `create_widget` は `Option` ではなく、Widgetを実装した具体的なstructを返してください。

## Store・要求との関係

- `create_widget(&Store)` でStoreから描画に使うentityを取得して構いません。幅に依存するbuffer、markdownのrender cache、scrollやfocusの補正など重い派生状態は `update` で作ってください。
- Componentのfieldに、frameをまたいでentityの参照を持たないでください。owned copyを持つ場合は、Storeとの二重管理とcloneのコストが妥当かを先に検討してください。
- 外部副作用は `RequestSink` への要求としてrunnerに渡してください。要求をfieldに溜めて後で書かないでください。
- crosstermのイベントではなく `src/platform/input/` の `InputEvent` を受け取ってください。

## テスト

- Widgetは、Storeを使わずに表示データを直接渡し、render結果のsnapshot、layout、clip、wrap、style、focus表示、`line_count` を確認してください。
- FocusStateは、WidgetとStoreを使わずに、キーによるfocusとcursorの移動、端での戻り値、解釈したキーで `Some(_)`・解釈しないキーで `None` を返すこと、`focus_event` による入退場、`update` による範囲の補正、unfocused時に入力を無視することを確認してください。
- Componentは、必要ならStoreを入力にして、`process_event`・`focus_event`・`update` の後の表示やcursor位置を確認してください。子を持つComponentでは、子の戻り値と別の子への `focus_event` がつながることを確認してください。表示の最終確認にはsnapshotを使ってください。
