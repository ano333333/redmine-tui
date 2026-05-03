# ratatui における描画結果キャッシュ調査

## 結論

`ratatui` に「widget の描画結果をフレーム間で保持して、そのまま再利用する」ための専用 API は、確認した範囲では存在しません。`ratatui` 自体は immediate mode で、毎フレーム UI 全体を描く前提です。公式 docs でも `Terminal::draw` ごとに現在バッファと前回バッファの差分だけを端末へ流す設計とされています。  
https://docs.rs/ratatui/latest/ratatui/struct.Frame.html  
https://docs.rs/ratatui/latest/ratatui/

ただし、「描画結果の永続化」は実装可能です。鍵になるのは `Buffer` です。`ratatui` の widget は端末へ直接描かず、中間 `Buffer` に描きます。`Frame::buffer_mut()` で現在フレームの `Buffer` に触れられ、`Buffer` 自体は `Clone` 可能で、`merge` / `diff` もあります。つまり、独自に offscreen `Buffer` を持ってキャッシュし、必要時だけ再生成し、通常フレームではそのキャッシュ内容を転写する方式が取れます。  
https://docs.rs/ratatui/latest/ratatui/buffer/struct.Buffer.html  
https://docs.rs/ratatui/latest/ratatui/struct.Frame.html

## 調査結果

### 1. widget の永続化

`WidgetRef` / `StatefulWidgetRef` で widget を参照で再利用する道はあります。これは「widget オブジェクトを毎回作らない」ための仕組みで、描画結果キャッシュそのものではありません。  
https://ratatui.rs/concepts/widgets/  
https://docs.rs/ratatui/latest/ratatui/widgets/trait.FrameExt.html

### 2. markdown のパース結果キャッシュ

`tui-markdown` は `from_str` / `from_str_with_options` で markdown を `Text` に変換します。ソースを見ると `pulldown-cmark::Parser` を回して `Text` を組み立てています。なので少なくとも markdown の再パースは、内容が変わらない限り自前で `Text` や AST を保持して避ける価値があります。  
https://docs.rs/tui-markdown/latest/tui_markdown/  
https://docs.rs/tui-markdown/latest/src/tui_markdown/lib.rs.html

### 3. 描画結果そのもののキャッシュ

ここが本題ですが、公式の専用 API はない一方、`Buffer` を使えば十分実現可能です。  
実装方針はだいたい以下です。

- `content_revision`
- `width`
- `theme/style`
- `scroll`
- 将来の画像なら `height`, `cell aspect`, `backend capability`

をキーにして `Buffer` をキャッシュする。

描画時は、キーが一致すれば再パース・再レイアウト・再画像変換をせず、キャッシュ済み `Buffer` の可視部分だけを現在フレームへコピーする。

これは公式 discussion でも近い発想が出ています。特に scrolling / wrapping の文脈で「intermediate buffer」「virtual canvas」「faux buffer / Viewport」を使ってから表示領域だけコピーする、という話があります。  
https://github.com/ratatui/ratatui/discussions/552

この点はかなり重要です。markdown は「内容は同じでも幅が変わると折り返し結果が変わる」ので、`Text` キャッシュだけでは不十分なことがあります。幅依存のレイアウトコストまで落としたいなら、`Buffer` キャッシュが本命です。

### 4. すでにある最適化との関係

`ratatui` はフレーム全体を毎回描いても、最後は前回との差分だけ端末へ送ります。discussion でも maintainers がその前提を説明しています。  
https://github.com/ratatui/ratatui/discussions/579

ただしこれは「端末出力の差分最適化」であって、markdown のパース、wrap、画像のセル化みたいな重い前処理は節約しません。なので、markdown 描画で前処理コストを気にするのは妥当です。

## 実務上のおすすめ

- まず `markdown -> Text/AST` をキャッシュする
- 次に `width` 等をキーに `rendered Buffer` をキャッシュする

特に将来的に画像を入れるなら、最初から「内容キャッシュ」と「表示サイズ依存の描画キャッシュ」を分けた方が設計が崩れにくいです。

私の見立てでは、`ratatui` で markdown viewer を作るなら `StatefulWidget` か `WidgetRef` ベースの独自 widget にして、内部 state に `parsed` と `rendered_buffer` を持つのが一番自然です。これは docs と discussion を踏まえた推奨で、公式に専用パターンとして明文化されているわけではありません。

## 参考リンク

- Ratatui `Frame`: https://docs.rs/ratatui/latest/ratatui/struct.Frame.html
- Ratatui crate docs: https://docs.rs/ratatui/latest/ratatui/
- Ratatui `Buffer`: https://docs.rs/ratatui/latest/ratatui/buffer/struct.Buffer.html
- Ratatui widgets concept: https://ratatui.rs/concepts/widgets/
- Ratatui `FrameExt`: https://docs.rs/ratatui/latest/ratatui/widgets/trait.FrameExt.html
- `tui-markdown`: https://docs.rs/tui-markdown/latest/tui_markdown/
- `tui-markdown` source: https://docs.rs/tui-markdown/latest/src/tui_markdown/lib.rs.html
- Ratatui discussion #552: https://github.com/ratatui/ratatui/discussions/552
- Ratatui discussion #579: https://github.com/ratatui/ratatui/discussions/579
