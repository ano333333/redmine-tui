## Issue properties

property-subject = Subject
property-description = Description
property-project = Project
property-tracker = Tracker
property-status = Status
property-priority = Priority
property-author = Author
property-assignee = Assignee
property-target-version = Version
property-start-date = Start date
property-due-date = Due date
property-done-ratio = % Done
property-estimated-hours = Estimate
property-spent-hours = Spent time
property-category = Category
property-created-on = Created
property-updated-on = Updated
property-parent-issue = Parent issue
property-child-issues = Subtasks
property-private = Private

## Shared values

value-none = (none)
value-unknown = (unknown)
value-default = (default)
value-changed = (changed)
value-yes = Yes
value-no = No
button-cancel = Cancel
button-continue = Continue
add-file = + Add file

## Issue

issue-fetching = Loading…
issue-retry-hint = Press r to retry

## Issue detail

detail-section-history = History

## Issue detail header

header-parent-label = Parent
header-edited = *Unsaved
header-uploading = ↑Saving
header-unconfirmed = ?Unconfirmed
header-parent-fetching = Loading…
header-parent-fetch-failed = Failed to load

## Markdown images

markdown-image-loading = {" "}(loading)
markdown-image-failed = {" "}(could not be loaded)

## Attachments list

attachments-title = Attachments
attachments-stage-uploading = sending
attachments-stage-attaching = attaching
attachments-stage-confirming = confirming

## Child issues list

children-title = Subtasks
children-summary = Open { $incomplete } / Closed { $complete }
children-add = + Add subtask
children-unfetched = No data
children-status-unknown = Unknown

## Journals list

journals-add = + Add comment
journal-state-edited = (edited)
journal-state-uploading = (uploading)
journal-state-local = (local)
journal-state-deleted = (deleted #{ $id })
journal-header-updated = { $user } updated on { $date }
journal-header-local = Local comment
journal-detail-attr-changed = { $property } changed from { $old } to { $new }
journal-detail-file = File
journal-detail-attachment-added = { $file } { $filename } added
journal-detail-attachment-deleted = { $file } { $filename } deleted

## New issue

new-issue-title = New issue
new-issue-hint = Enter: edit  Ctrl+S: create  y: select issue
new-issue-submitting-hint = Creating…  y: select issue
new-issue-attachments = Attachments
new-issue-description-lines = { $first_line } ({ $count } lines)

## Issue select popup

issue-select-projects = PROJECTS
issue-select-issues = ISSUES
issue-select-preview = PREVIEW
issue-select-loading = Loading…
issue-select-retry-hint = Press r to retry
issue-select-empty = No issues
issue-select-hint = h/l switch   j/k move   Enter open   n new   q close

## Select box popup

select-box-hint = j/k move · Enter select · q back
select-box-none-choice = No selection (None)

## Number input popup

number-input-hint = Enter: OK / Esc: cancel / empty: unset
number-input-too-large = The number is too large

## Date picker popup

date-picker-title = Select date
date-picker-year = YYYY
date-picker-month = MM
date-picker-day = DD
date-picker-calendar = Calendar

## Spent time input popup

spent-time-title = Spent time
spent-time-activity = ACTIVITY
spent-time-hours = HOURS
spent-time-memo = MEMO
spent-time-hint = Enter: edit / h,j,k,l: move
spent-time-save = SAVE
spent-time-hours-placeholder = Enter hours
spent-time-memo-placeholder = Enter a memo

## Attachment input popup

attachment-input-title = Attach file
attachment-input-filename = File name
attachment-input-path = File path
attachment-input-hint = Enter: confirm / Tab: switch field / Esc: cancel
attachment-input-required = Enter a file name and a file path

## Issue property conflict popup

property-conflict-message = Your edits conflict with the server. Choose which to keep.
property-conflict-name = Property
property-conflict-before = Before edit
property-conflict-after = After edit
property-conflict-server = Server

## Remote journal conflict popup

journal-conflict-message = The journal notes conflict with the server. Choose which to keep.
journal-conflict-local = LOCAL · Your changes
journal-conflict-remote = REMOTE · Server changes

## Toasts

toast-new-issue-attachment-read-failed = Could not read { $filename } to attach to the new issue: { $reason }
toast-new-issue-attachment-upload-failed = Failed to send { $filename } to attach to the new issue: { $reason }
toast-issue-create-failed = Failed to create the issue: { $reason }
toast-issue-create-unconfirmed = Could not confirm whether the issue was created. Creating it again may create a duplicate, so check it in the issue select popup: { $reason }
toast-issue-upload-failed = Failed to save issue #{ $id }: { $reason }
toast-issue-upload-unconfirmed = Saved issue #{ $id }, but failed to fetch it for confirmation: { $reason }
toast-remote-journal-upload-failed = Failed to save the remote journal: { $reason }
toast-remote-journal-upload-unconfirmed = Saved journal #{ $journal_id }, but failed to fetch it for confirmation: { $reason }
toast-local-journal-upload-failed = Failed to save the local journal: { $reason }
toast-local-journal-upload-unconfirmed = Saved the local journal, but failed to fetch it for confirmation: { $reason }
toast-deleted-journal-upload-failed = Failed to post the deleted journal: { $reason }
toast-deleted-journal-upload-unconfirmed = Posted the deleted journal, but failed to fetch it for confirmation: { $reason }
toast-attachment-read-failed = Could not read the file to attach to issue #{ $issue_id }: { $reason }
toast-attachment-upload-failed = Failed to send the file to attach to issue #{ $issue_id }: { $reason }
toast-attachment-attach-failed = Failed to attach the file to issue #{ $issue_id }: { $reason }
toast-attachment-attach-unconfirmed = Attached the file to issue #{ $issue_id }, but failed to fetch it for confirmation. Adding it again will attach it twice: { $reason }
toast-editor-failed = Failed to edit with the editor: { $error }
toast-image-display-unavailable = Images are not shown because the terminal size in pixels is unavailable: { $reason }
toast-new-issue-subject-required = Enter a subject before creating the issue
toast-computed-property-not-editable = Issue #{ $issue_id }: { $property } is calculated from its subtasks and cannot be edited
