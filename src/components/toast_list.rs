use crate::stores::Store;
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
    pub fn push(&mut self, message: String) {
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

    /// Storeのnoticeに続けて、このComponentのtoastを追加順に並べる。
    pub fn create_widget(&self, store: &Store) -> ToastWidget {
        ToastWidget::new(
            store
                .get_notices()
                .iter()
                .map(|notice| notice.message.clone())
                .chain(self.toasts.iter().map(|toast| toast.message.clone()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{Action, NoticeAction, NoticeId};

    fn messages(toast_list: &ToastListComponent) -> Vec<String> {
        toast_list.create_widget(&Store::new()).messages
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

    #[test]
    fn create_widget_shows_toasts_after_store_notices() {
        let mut store = Store::new();
        store.consume_action(Action::Notice(NoticeAction::Push {
            id: NoticeId::new(),
            message: "store".to_string(),
        }));
        let mut toast_list = ToastListComponent::default();
        toast_list.push("component".to_string());

        let widget = toast_list.create_widget(&store);

        assert_eq!(widget.messages, ["store", "component"]);
    }
}
