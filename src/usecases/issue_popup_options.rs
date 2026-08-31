use crate::stores::Store;
use crate::vos::{EntityIdValue, IssueId};

/// idと名前のentry一覧と現在値から、SelectBoxPopupComponent::new用の
/// items(id昇順ソート、`sorted`指定時)とフォーカス箇所のindexを組み立てる。
///
/// 現在値が見つからない場合、及び`current_id`がNoneの場合はfocused_indexを0にする。
fn build_select_options(
    mut entries: Vec<(u16, String)>,
    current_id: Option<u16>,
    sorted: bool,
) -> (Vec<(u16, String)>, usize) {
    if sorted {
        entries.sort_by_key(|(id, _)| *id);
    }
    let focused_index = current_id
        .and_then(|current_id| entries.iter().position(|(id, _)| *id == current_id))
        .unwrap_or(0);
    (entries, focused_index)
}

/// IssueStatusPopup用のitems/focused_indexを組み立てる。
pub fn build_issue_status_options(store: &Store) -> (Vec<(u16, String)>, usize) {
    let issue_statuses = store
        .get_issue_statuses()
        .iter()
        .map(|(id, status)| (id.get(), status.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(issue_statuses, None, false)
}

/// AssignedToPopup用のitems/focused_indexを組み立てる。
pub fn build_assigned_to_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_assigned_to_id = store
        .get_issue(issue_id)
        .and_then(|(issue, _)| issue.assigned_to_id);
    let users = store
        .get_users()
        .iter()
        .map(|(id, user)| (id.get(), user.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(users, current_assigned_to_id.map(|id| id.get()), true)
}

/// TargetVersionPopup用のitems/focused_indexを組み立てる。
pub fn build_target_version_options(
    store: &Store,
    issue_id: IssueId,
) -> (Vec<(u16, String)>, usize) {
    let current_target_version_id = store
        .get_issue(issue_id)
        .and_then(|(issue, _)| issue.target_version_id);
    let target_versions = store
        .get_target_versions()
        .iter()
        .map(|(id, target_version)| (id.get(), target_version.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(
        target_versions,
        current_target_version_id.map(|id| id.get()),
        true,
    )
}

/// DoneRatioPopup用のitems/focused_indexを組み立てる。
///
/// 選択肢は0,10,...,100の固定11件。
pub fn build_done_ratio_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_done_ratio = store
        .get_issue(issue_id)
        .map(|(issue, _)| issue.done_ratio)
        .unwrap_or(0);
    let done_ratios = (0..=100)
        .step_by(10)
        .map(|ratio| (ratio, ratio.to_string()))
        .collect::<Vec<_>>();
    build_select_options(done_ratios, Some(current_done_ratio), false)
}

/// CategoryPopup用のitems/focused_indexを組み立てる。
pub fn build_category_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_category_id = store
        .get_issue(issue_id)
        .and_then(|(issue, _)| issue.category_id);
    let categories = store
        .get_categories()
        .iter()
        .map(|(id, category)| (id.get(), category.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(categories, current_category_id.map(|id| id.get()), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{Dispatcher, IssueAction};

    fn loaded_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher);
        dispatcher.dispatch(IssueAction::Load { id: 3.into() });
        while dispatcher.consume_actinos_len() > 0 {
            dispatcher.consume_action();
        }
        dispatcher
    }

    #[test]
    fn issue_status_options_are_not_sorted_and_focus_the_first_item() {
        let dispatcher = loaded_dispatcher();

        let (items, focused_index) = build_issue_status_options(dispatcher.store());

        assert!(!items.is_empty());
        assert_eq!(focused_index, 0);
    }

    #[test]
    fn assigned_to_options_are_sorted_and_focus_the_current_assignee() {
        let mut dispatcher = loaded_dispatcher();
        let current_assigned_to_id = dispatcher
            .store()
            .get_users()
            .keys()
            .next()
            .copied()
            .expect("fixture has at least one user");
        dispatcher.dispatch(IssueAction::UpdateAssignedTo {
            id: 3.into(),
            assigned_to_id: Some(current_assigned_to_id),
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_assigned_to_options(dispatcher.store(), 3.into());

        assert!(items.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(items[focused_index].0, current_assigned_to_id.get());
    }

    #[test]
    fn assigned_to_options_focus_the_first_item_when_unassigned() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdateAssignedTo {
            id: 3.into(),
            assigned_to_id: None,
        });
        dispatcher.consume_action();

        let (_, focused_index) = build_assigned_to_options(dispatcher.store(), 3.into());

        assert_eq!(focused_index, 0);
    }

    #[test]
    fn target_version_options_are_sorted_and_focus_the_current_value() {
        let mut dispatcher = loaded_dispatcher();
        let current_target_version_id = dispatcher
            .store()
            .get_target_versions()
            .keys()
            .next()
            .copied()
            .expect("fixture has at least one target version");
        dispatcher.dispatch(IssueAction::UpdateTargetVersion {
            id: 3.into(),
            target_version_id: Some(current_target_version_id),
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_target_version_options(dispatcher.store(), 3.into());

        assert!(items.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(items[focused_index].0, current_target_version_id.get());
    }

    #[test]
    fn done_ratio_options_are_fixed_steps_of_ten() {
        let dispatcher = loaded_dispatcher();

        let (items, _) = build_done_ratio_options(dispatcher.store(), 3.into());

        assert_eq!(
            items,
            (0..=100)
                .step_by(10)
                .map(|ratio| (ratio, ratio.to_string()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn done_ratio_options_focus_the_current_value() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdateDoneRatio {
            id: 3.into(),
            done_ratio: 30,
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_done_ratio_options(dispatcher.store(), 3.into());

        assert_eq!(items[focused_index].0, 30);
    }

    #[test]
    fn category_options_are_sorted_and_focus_the_first_item_when_uncategorized() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdateCategory {
            id: 3.into(),
            category_id: None,
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_category_options(dispatcher.store(), 3.into());

        assert!(items.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(focused_index, 0);
    }

    #[test]
    fn category_options_focus_the_current_category() {
        let mut dispatcher = loaded_dispatcher();
        let current_category_id = dispatcher
            .store()
            .get_categories()
            .keys()
            .next()
            .copied()
            .expect("fixture has at least one category");
        dispatcher.dispatch(IssueAction::UpdateCategory {
            id: 3.into(),
            category_id: Some(current_category_id),
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_category_options(dispatcher.store(), 3.into());

        assert_eq!(items[focused_index].0, current_category_id.get());
    }
}
