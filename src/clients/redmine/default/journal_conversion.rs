//! Issueレスポンスに埋め込まれたJournalのDTOをドメイン型へ変換する。

use serde::Deserialize;

use crate::clients::redmine::RedmineClientError;
use crate::entities::Journal;
use crate::vos::{IssueId, JournalDetail, JournalId};

use super::journal_detail_conversion::{RedmineJournalDetail, try_into_domain};
use super::value_conversion;

/// RedmineのJournalレスポンスを受け取る中間DTO。
#[derive(Deserialize)]
pub(super) struct RedmineJournal {
    pub(super) id: u16,
    pub(super) user: RedmineJournalUser,
    pub(super) updated_on: Option<String>,
    pub(super) notes: String,
    /// Journal JSONにネストされたIssue属性の変更履歴。
    #[serde(default)]
    pub(super) details: Vec<RedmineJournalDetail>,
}

/// Journalの`user`に含まれる`{ id, name }`形式をデシリアライズする。
///
/// Journal変換ではユーザー名だけを取り出すため、複数のドメイン型への変換を担う
/// `NamedRedmineEntity`とは独立したDTOとして扱う。
#[derive(Deserialize)]
pub(super) struct RedmineJournalUser {
    pub(super) id: u16,
    pub(super) name: String,
}

/// Journal DTOを、所属するIssueを明示したドメイン型へ変換する。
///
/// RedmineのJournal JSONにはIssue IDが含まれないため、呼び出し元はJournalを
/// 内包していたIssueのIDを渡す必要がある。一度も編集されていないjournalは
/// `updated_on`が`null`で返るため、その場合は`None`として扱う。`updated_on`が
/// 値を持つのにRFC 3339形式でなければ[`RedmineClientError::Client`]を返す。
/// 属性変更の変換エラーも同じエラー型で返す。
pub(super) fn convert_journal(
    issue_id: IssueId,
    journal: RedmineJournal,
) -> Result<Journal, RedmineClientError> {
    let updated_on = journal
        .updated_on
        .as_deref()
        .map(value_conversion::parse_datetime)
        .transpose()?;
    // `attr`以外のpropertyは変換対象外を表すNoneとなるため、Journalには含めない。
    let details = journal
        .details
        .into_iter()
        .map(try_into_domain)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|attr| attr)
        .map(JournalDetail::Attr)
        .collect();

    Ok(Journal {
        id: JournalId::new(journal.id),
        issue_id,
        user: journal.user.name,
        updated_on,
        details,
        notes: journal.notes,
    })
}
