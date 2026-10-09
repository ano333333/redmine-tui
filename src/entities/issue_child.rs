use crate::vos::{IssueId, TrackerId};

/// Issue詳細の取得結果に含まれる子Issueの一覧項目。
///
/// 子Issue自身の詳細ではなく、取得時点のID・トラッカー・題名と、その下の子一覧だけを持つ。
/// 子Issueの詳細を取得済みなら、表示にはStoreが持つ編集反映後の値を優先する。
#[derive(Clone, Debug, PartialEq)]
pub struct IssueChild {
    pub id: IssueId,
    pub tracker_id: TrackerId,
    pub subject: String,
    pub children: Vec<IssueChild>,
}
