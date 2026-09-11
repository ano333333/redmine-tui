/// Redmine上のJournalのnotesを編集するときの変更前後の値。
#[derive(Clone, Debug, PartialEq)]
pub struct JournalNotesDiff {
    pub before: String,
    pub after: String,
}
