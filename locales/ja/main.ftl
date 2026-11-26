## Issue properties

property-subject = 題名
property-description = 説明
property-project = プロジェクト
property-tracker = トラッカー
property-status = ステータス
property-priority = 優先度
property-author = 作成者
property-assignee = 担当者
property-target-version = 対象バージョン
property-start-date = 開始日
property-due-date = 期日
property-done-ratio = 進捗率
property-estimated-hours = 予定工数
property-spent-hours = 実績工数
property-category = カテゴリー
property-created-on = 作成日
property-updated-on = 更新日
property-parent-issue = 親チケット
property-child-issues = 子チケット
property-private = 非公開

## Shared values

value-none = (なし)
value-unknown = (不明)
value-default = (既定)
value-changed = (変更あり)
value-yes = はい
value-no = いいえ
button-cancel = キャンセル
button-continue = 続行
add-file = ＋ ファイルを追加

## Issue

issue-fetching = 表示中です
issue-retry-hint = r で再試行

## Issue detail

detail-section-history = 履歴

## Issue detail header

header-parent-label = 親チケット
header-edited = ＊未保存
header-uploading = ↑送信中
header-unconfirmed = ？未確認
header-parent-fetching = 取得中…
header-parent-fetch-failed = 取得失敗

## Markdown images

markdown-image-loading = （読み込み中）
markdown-image-failed = （読み込めませんでした）

## Attachments list

attachments-title = 添付ファイル
attachments-stage-uploading = 送信中
attachments-stage-attaching = 添付中
attachments-stage-confirming = 確認中

## Child issues list

children-title = 子チケット
children-summary = 未完了 { $incomplete } / 完了 { $complete }
children-add = ＋ 子チケットを追加
children-unfetched = 未取得
children-status-unknown = (不明)

## Journals list

journals-add = ＋ コメントを追加
journal-state-edited = (edited)
journal-state-uploading = (uploading)
journal-state-local = (local)
journal-state-deleted = (deleted #{ $id })
journal-header-updated = { $user }が{ $date }に更新
journal-header-local = ローカルコメント
journal-detail-attr-changed = { $property } を { $old } から { $new } に変更
journal-detail-file = ファイル
journal-detail-attachment-added = { $file } { $filename } を追加
journal-detail-attachment-deleted = { $file } { $filename } を削除

## New issue

new-issue-title = 新しいIssue
new-issue-hint = Enter: 編集  Ctrl+S: 作成  y: Issue選択
new-issue-submitting-hint = 作成中…  y: Issue選択
new-issue-attachments = 添付ファイル
new-issue-description-lines = { $first_line } (全{ $count }行)

## Issue select popup

issue-select-projects = PROJECTS
issue-select-issues = ISSUES
issue-select-preview = PREVIEW
issue-select-loading = 読込中です
issue-select-retry-hint = r で再試行
issue-select-empty = Issueはありません
issue-select-hint = h/l 切替   j/k 移動   Enter 開く   n 新規作成   q 閉じる

## Select box popup

select-box-hint = j/k 移動 · Enter 決定 · q 戻る
select-box-none-choice = 選択なし(None)

## Number input popup

number-input-hint = Enter: 確定 / Esc: 取消 / 空欄: 未設定
number-input-too-large = 数値が大きすぎます

## Date picker popup

date-picker-title = 日付選択
date-picker-year = 年
date-picker-month = 月
date-picker-day = 日
date-picker-calendar = カレンダー

## Spent time input popup

spent-time-title = 実績工数
spent-time-activity = ACTIVITY
spent-time-hours = HOURS
spent-time-memo = MEMO
spent-time-hint = Enter: edit / h,j,k,l: move
spent-time-save = SAVE
spent-time-hours-placeholder = 工数を入力
spent-time-memo-placeholder = メモを入力

## Attachment input popup

attachment-input-title = ファイルを添付
attachment-input-filename = ファイル名
attachment-input-path = ファイルパス
attachment-input-hint = Enter: 確定 / Tab: 入力欄の切替 / Esc: 取消
attachment-input-required = ファイル名とファイルパスを入力してください

## Issue property conflict popup

property-conflict-message = 編集内容とサーバー内容が競合しています。採用する方を選択してください。
property-conflict-name = プロパティ名
property-conflict-before = 編集前の値
property-conflict-after = 編集後の値
property-conflict-server = サーバーの値

## Remote journal conflict popup

journal-conflict-message = Journal本文が競合しています。採用する方を選択してください。
journal-conflict-local = LOCAL · あなたの変更
journal-conflict-remote = REMOTE · サーバーの変更

## Toasts

toast-new-issue-attachment-read-failed = 新しいIssueへ添付する{ $filename }を読み込めませんでした: { $reason }
toast-new-issue-attachment-upload-failed = 新しいIssueへ添付する{ $filename }の送信に失敗しました: { $reason }
toast-issue-create-failed = Issueの作成に失敗しました: { $reason }
toast-issue-create-unconfirmed = Issueを作成できたか確認できませんでした。もう一度作成すると二重に作成されることがあるため、Issue選択で確認してください: { $reason }
toast-issue-upload-failed = Issue #{ $id }の保存に失敗しました: { $reason }
toast-issue-upload-unconfirmed = Issue #{ $id }を保存しましたが、確認の取得に失敗しました: { $reason }
toast-remote-journal-upload-failed = Remote Journalの保存に失敗しました: { $reason }
toast-remote-journal-upload-unconfirmed = Journal #{ $journal_id }を保存しましたが、確認の取得に失敗しました: { $reason }
toast-local-journal-upload-failed = Local Journalの保存に失敗しました: { $reason }
toast-local-journal-upload-unconfirmed = Local Journalを保存しましたが、確認の取得に失敗しました: { $reason }
toast-deleted-journal-upload-failed = 削除されたJournalの投稿に失敗しました: { $reason }
toast-deleted-journal-upload-unconfirmed = 削除されたJournalを投稿しましたが、確認の取得に失敗しました: { $reason }
toast-attachment-read-failed = Issue #{ $issue_id }へ添付するファイルを読み込めませんでした: { $reason }
toast-attachment-upload-failed = Issue #{ $issue_id }へ添付するファイルの送信に失敗しました: { $reason }
toast-attachment-attach-failed = Issue #{ $issue_id }へのファイルの添付に失敗しました: { $reason }
toast-attachment-attach-unconfirmed = Issue #{ $issue_id }にファイルを添付しましたが、確認の取得に失敗しました。もう一度追加すると二重に添付されます: { $reason }
toast-editor-failed = エディタによる編集に失敗しました: { $error }
toast-image-display-unavailable = terminalのピクセル単位の大きさを取得できないため、画像を表示しません: { $reason }
toast-new-issue-subject-required = 題名を入力してから作成してください
toast-computed-property-not-editable = Issue #{ $issue_id }の{ $property }は子Issueから計算されるため編集できません
