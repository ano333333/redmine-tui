use crate::usecases::{IssueCreateFailure, UsecaseFailure};
use crate::vos::IssueId;
use crate::widgets::ToastWidget;

const TOAST_VISIBLE_FOR: chrono::Duration = chrono::Duration::seconds(5);

struct Toast {
    message: String,
    elapsed: chrono::Duration,
}

/// 一時通知を追加順に持ち、表示経過時間で破棄する。
#[derive(Default)]
pub struct ToastListComponent {
    toasts: Vec<Toast>,
}

impl ToastListComponent {
    pub fn push_failure(&mut self, failure: &UsecaseFailure) {
        let message = match failure {
            UsecaseFailure::IssueCreate { cause, .. } => match cause {
                IssueCreateFailure::AttachmentFileRead { filename, reason } => {
                    format!("新しいIssueへ添付する{filename}を読み込めませんでした: {reason}")
                }
                IssueCreateFailure::AttachmentFileUpload { filename, reason } => {
                    format!("新しいIssueへ添付する{filename}の送信に失敗しました: {reason}")
                }
                IssueCreateFailure::Create { reason } => {
                    format!("Issueの作成に失敗しました: {reason}")
                }
            },
            UsecaseFailure::IssueUpload { id, reason } => {
                format!("Issue #{id}の保存に失敗しました: {reason}")
            }
            UsecaseFailure::IssueUploadConfirm { id, reason } => {
                format!("Issue #{id}を保存しましたが、確認の取得に失敗しました: {reason}")
            }
            UsecaseFailure::RemoteJournalUpload { reason, .. } => {
                format!("Remote Journalの保存に失敗しました: {reason}")
            }
            UsecaseFailure::RemoteJournalUploadConfirm { journal_id, reason } => {
                format!("Journal #{journal_id}を保存しましたが、確認の取得に失敗しました: {reason}")
            }
            UsecaseFailure::LocalJournalUpload { reason, .. } => {
                format!("Local Journalの保存に失敗しました: {reason}")
            }
            UsecaseFailure::LocalJournalUploadConfirm { reason, .. } => {
                format!("Local Journalを保存しましたが、確認の取得に失敗しました: {reason}")
            }
            UsecaseFailure::DeletedJournalUpload { reason, .. } => {
                format!("削除されたJournalの投稿に失敗しました: {reason}")
            }
            UsecaseFailure::DeletedJournalUploadConfirm { reason, .. } => {
                format!("削除されたJournalを投稿しましたが、確認の取得に失敗しました: {reason}")
            }
            UsecaseFailure::AttachmentFileRead { issue_id, reason } => {
                format!("Issue #{issue_id}へ添付するファイルを読み込めませんでした: {reason}")
            }
            UsecaseFailure::AttachmentFileUpload { issue_id, reason } => {
                format!("Issue #{issue_id}へ添付するファイルの送信に失敗しました: {reason}")
            }
            UsecaseFailure::AttachmentAttach { issue_id, reason } => {
                format!("Issue #{issue_id}へのファイルの添付に失敗しました: {reason}")
            }
            UsecaseFailure::AttachmentAttachConfirm { issue_id, reason } => format!(
                "Issue #{issue_id}にファイルを添付しましたが、確認の取得に失敗しました。もう一度追加すると二重に添付されます: {reason}"
            ),
        };
        self.push(message);
    }

    pub fn push_editor_failure(&mut self, error: &dyn std::fmt::Display) {
        self.push(format!("エディタによる編集に失敗しました: {error}"));
    }

    pub fn push_image_display_unavailable(&mut self, reason: &str) {
        self.push(format!(
            "terminalのピクセル単位の大きさを取得できないため、画像を表示しません: {reason}"
        ));
    }

    pub fn push_new_issue_subject_required(&mut self) {
        self.push("題名を入力してから作成してください".to_string());
    }

    pub fn push_computed_property_not_editable(&mut self, issue_id: IssueId, property: &str) {
        self.push(format!(
            "Issue #{issue_id}の{property}は子Issueから計算されるため編集できません"
        ));
    }

    fn push(&mut self, message: String) {
        self.toasts.push(Toast {
            message,
            elapsed: chrono::Duration::zero(),
        });
    }

    /// 表示経過時間を進め、累積が5秒に達したtoastを破棄する。
    pub fn tick(&mut self, elapsed: chrono::Duration) {
        for toast in &mut self.toasts {
            toast.elapsed += elapsed;
        }
        self.toasts
            .retain(|toast| toast.elapsed < TOAST_VISIBLE_FOR);
    }

    pub fn create_widget(&self) -> ToastWidget {
        ToastWidget::new(
            self.toasts
                .iter()
                .map(|toast| toast.message.clone())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;
    use crate::vos::JournalId;

    fn messages(toast_list: &ToastListComponent) -> Vec<String> {
        toast_list.create_widget().messages
    }

    #[test]
    fn tick_keeps_toasts_shorter_than_five_seconds_in_total() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push("failed".to_string());

        toast_list.tick(chrono::Duration::seconds(2));
        toast_list.tick(chrono::Duration::milliseconds(2999));

        assert_eq!(messages(&toast_list), ["failed"]);
    }

    #[test]
    fn tick_removes_toasts_reaching_five_seconds_in_total() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push("failed".to_string());

        toast_list.tick(chrono::Duration::seconds(2));
        toast_list.tick(chrono::Duration::seconds(3));

        assert!(messages(&toast_list).is_empty());
    }

    #[test]
    fn toast_pushed_after_tick_counts_its_own_elapsed_time() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push("old".to_string());
        toast_list.tick(chrono::Duration::seconds(4));
        toast_list.push("new".to_string());

        toast_list.tick(chrono::Duration::seconds(1));

        assert_eq!(messages(&toast_list), ["new"]);
    }

    fn render_toast_snapshot(name: &str, toast_list: &ToastListComponent) {
        render_snapshot(name, 60, 12, toast_list.create_widget());
    }

    #[test]
    fn snapshot_issue_create_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::IssueCreate {
            creation_id: crate::usecases::IssueCreationId::new(),
            cause: IssueCreateFailure::Create {
                reason: "unprocessable entity: Subject cannot be blank".to_string(),
            },
        });

        render_toast_snapshot("toast_list_issue_create_failure", &toast_list);
    }

    #[test]
    fn snapshot_issue_create_attachment_failures() {
        let mut toast_list = ToastListComponent::default();
        for cause in [
            IssueCreateFailure::AttachmentFileRead {
                filename: "screen.png".to_string(),
                reason: "failed to read /tmp/screen.png: not found".to_string(),
            },
            IssueCreateFailure::AttachmentFileUpload {
                filename: "screen.png".to_string(),
                reason: "network error: offline".to_string(),
            },
        ] {
            toast_list.push_failure(&UsecaseFailure::IssueCreate {
                creation_id: crate::usecases::IssueCreationId::new(),
                cause,
            });
        }

        render_snapshot(
            "toast_list_issue_create_attachment_failures",
            60,
            20,
            toast_list.create_widget(),
        );
    }

    #[test]
    fn snapshot_issue_upload_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::IssueUpload {
            id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_issue_upload_failure", &toast_list);
    }

    #[test]
    fn snapshot_issue_upload_confirm_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::IssueUploadConfirm {
            id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_issue_upload_confirm_failure", &toast_list);
    }

    #[test]
    fn snapshot_editor_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_editor_failure(&"editor exited with status 1");

        render_toast_snapshot("toast_list_editor_failure", &toast_list);
    }

    #[test]
    fn snapshot_remote_journal_upload_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::RemoteJournalUpload {
            journal_id: JournalId::new(10),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_remote_journal_upload_failure", &toast_list);
    }

    #[test]
    fn snapshot_remote_journal_upload_confirm_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::RemoteJournalUploadConfirm {
            journal_id: JournalId::new(10),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot(
            "toast_list_remote_journal_upload_confirm_failure",
            &toast_list,
        );
    }

    #[test]
    fn snapshot_local_journal_upload_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::LocalJournalUpload {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_local_journal_upload_failure", &toast_list);
    }

    #[test]
    fn snapshot_local_journal_upload_confirm_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::LocalJournalUploadConfirm {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot(
            "toast_list_local_journal_upload_confirm_failure",
            &toast_list,
        );
    }

    #[test]
    fn snapshot_deleted_journal_upload_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::DeletedJournalUpload {
            original_id: JournalId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_deleted_journal_upload_failure", &toast_list);
    }

    #[test]
    fn snapshot_deleted_journal_upload_confirm_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::DeletedJournalUploadConfirm {
            original_id: JournalId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot(
            "toast_list_deleted_journal_upload_confirm_failure",
            &toast_list,
        );
    }

    #[test]
    fn snapshot_attachment_file_read_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::AttachmentFileRead {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_attachment_file_read_failure", &toast_list);
    }

    #[test]
    fn snapshot_attachment_file_upload_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::AttachmentFileUpload {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_attachment_file_upload_failure", &toast_list);
    }

    #[test]
    fn snapshot_attachment_attach_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::AttachmentAttach {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_attachment_attach_failure", &toast_list);
    }

    #[test]
    fn snapshot_attachment_attach_confirm_failure() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_failure(&UsecaseFailure::AttachmentAttachConfirm {
            issue_id: IssueId::new(3),
            reason: "network error: offline".to_string(),
        });

        render_toast_snapshot("toast_list_attachment_attach_confirm_failure", &toast_list);
    }

    #[test]
    fn snapshot_image_display_unavailable() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_image_display_unavailable("terminal size is zero");

        render_toast_snapshot("toast_list_image_display_unavailable", &toast_list);
    }

    #[test]
    fn snapshot_computed_property_not_editable() {
        let mut toast_list = ToastListComponent::default();
        toast_list.push_computed_property_not_editable(IssueId::new(3), "優先度");

        render_toast_snapshot("toast_list_computed_property_not_editable", &toast_list);
    }
}
