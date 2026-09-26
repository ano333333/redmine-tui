use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};

use crate::stores::{Dispatcher, IssueAction, Store};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId,
    TrackerId, UserId,
};

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

/// TrackerPopup用のitems/focused_indexを組み立てる。
// FIXME: projectで有効なtrackerに絞っていない。Redmineは使えないtracker_idをエラーなしで
// 無視するため、ローカルでは変更済みに見えてしまう。
pub fn build_tracker_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_tracker_id = store.get_issue(issue_id).0.tracker_id;
    let trackers = store
        .get_trackers()
        .iter()
        .map(|(id, tracker)| (id.get(), tracker.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(trackers, Some(current_tracker_id.get()), true)
}

/// ProjectPopup用のitems/focused_indexを組み立てる。
// FIXME: add_issues権限のあるprojectに絞っていない。Redmineは移動できないproject_idを
// エラーなしで無視するため、ローカルでは変更済みに見えてしまう。
pub fn build_project_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_project_id = store.get_issue(issue_id).0.issue.project_id;
    let projects = store
        .get_projects()
        .iter()
        .map(|(id, project)| (id.get(), project.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(projects, Some(current_project_id.get()), true)
}

/// PriorityPopup用のitems/focused_indexを組み立てる。
pub fn build_priority_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_priority_id = store.get_issue(issue_id).0.priority_id;
    let priorities = store
        .get_priorities()
        .iter()
        .map(|(id, priority)| (id.get(), priority.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(priorities, Some(current_priority_id.get()), true)
}

/// AssignedToPopup用のitems/focused_indexを組み立てる。
pub fn build_assigned_to_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let current_assigned_to_id = store.get_issue(issue_id).0.assigned_to_id;
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
    let (issue, _) = store.get_issue(issue_id);
    let current_target_version_id = issue.target_version_id;
    let target_versions = store
        .get_target_versions(issue.issue.project_id)
        .into_iter()
        .map(|target_version| (target_version.id.get(), target_version.name.clone()))
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
    let current_done_ratio = store.get_issue(issue_id).0.done_ratio;
    let done_ratios = (0..=100)
        .step_by(10)
        .map(|ratio| (ratio, ratio.to_string()))
        .collect::<Vec<_>>();
    build_select_options(done_ratios, Some(current_done_ratio), false)
}

/// CategoryPopup用のitems/focused_indexを組み立てる。
pub fn build_category_options(store: &Store, issue_id: IssueId) -> (Vec<(u16, String)>, usize) {
    let (issue, _) = store.get_issue(issue_id);
    let current_category_id = issue.category_id;
    let categories = store
        .get_categories(issue.issue.project_id)
        .into_iter()
        .map(|category| (category.id.get(), category.name.clone()))
        .collect::<Vec<_>>();
    build_select_options(categories, current_category_id.map(|id| id.get()), true)
}

/// IssueStatusPopupの選択結果からUpdateStatusをdispatchするobserverを組み立てる。
///
/// statusは必須項目のため、選択なし(None)は無視する。
pub fn issue_status_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |status_id| {
        if let Some(status_id) = status_id {
            dispatcher.borrow_mut().dispatch(IssueAction::UpdateStatus {
                id: issue_id,
                status_id: IssueStatusId::new(status_id),
            });
        }
    })
}

/// TrackerPopupの選択結果からUpdateTrackerをdispatchするobserverを組み立てる。
///
/// trackerは必須項目のため、選択なし(None)は無視する。
// FIXME: tracker変更時、Redmineは新trackerで使えないステータスを既定ステータスへ戻すことがある。
// upload後に再取得しないため、ローカルのステータスとずれる。
pub fn tracker_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |tracker_id| {
        if let Some(tracker_id) = tracker_id {
            dispatcher
                .borrow_mut()
                .dispatch(IssueAction::UpdateTracker {
                    id: issue_id,
                    tracker_id: TrackerId::new(tracker_id),
                });
        }
    })
}

/// ProjectPopupの選択結果からUpdateProjectをdispatchするobserverを組み立てる。
///
/// projectは必須項目のため、選択なし(None)は無視する。現在と同じprojectを選んだ場合も、
/// 対象バージョンとカテゴリーを消さないようdispatchしない。
// FIXME: project移動時、Redmineは移動先で無効なtrackerを先頭のtrackerへ変え、別rootのprojectへ
// 移すと親チケットを外す。upload後に再取得しないため、ローカルの値とずれる。
// また、移動したIssueはProjectIssuesStoreに移動前のprojectの一覧として残る。
pub fn project_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |project_id| {
        let Some(project_id) = project_id.map(ProjectId::new) else {
            return;
        };
        let current_project_id = dispatcher
            .borrow()
            .store()
            .get_issue(issue_id)
            .0
            .issue
            .project_id;
        if project_id == current_project_id {
            return;
        }
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateProject {
                id: issue_id,
                project_id,
            });
    })
}

/// PriorityPopupの選択結果からUpdatePriorityをdispatchするobserverを組み立てる。
///
/// priorityは必須項目のため、選択なし(None)は無視する。
// FIXME: 子チケットを持つIssueでは、Redmine既定の`parent_issue_priority: derived`により
// サーバーがpriority_idをエラーなしで無視する。ローカルでは変更済みに見えてしまう。
pub fn priority_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |priority_id| {
        if let Some(priority_id) = priority_id {
            dispatcher
                .borrow_mut()
                .dispatch(IssueAction::UpdatePriority {
                    id: issue_id,
                    priority_id: PriorityId::new(priority_id),
                });
        }
    })
}

/// AssignedToPopupの選択結果からUpdateAssignedToをdispatchするobserverを組み立てる。
pub fn assigned_to_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |assigned_to_id| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateAssignedTo {
                id: issue_id,
                assigned_to_id: assigned_to_id.map(UserId::new),
            });
    })
}

/// TargetVersionPopupの選択結果からUpdateTargetVersionをdispatchするobserverを組み立てる。
pub fn target_version_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |target_version_id| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateTargetVersion {
                id: issue_id,
                target_version_id: target_version_id.map(TargetVersionId::new),
            });
    })
}

/// DoneRatioPopupの選択結果からUpdateDoneRatioをdispatchするobserverを組み立てる。
///
/// 選択なし(None)はキャンセルとして扱い、dispatchしない。
pub fn done_ratio_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |done_ratio| {
        if let Some(done_ratio) = done_ratio {
            dispatcher
                .borrow_mut()
                .dispatch(IssueAction::UpdateDoneRatio {
                    id: issue_id,
                    done_ratio,
                });
        }
    })
}

/// CategoryPopupの選択結果からUpdateCategoryをdispatchするobserverを組み立てる。
pub fn category_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<u16>)> {
    Box::new(move |category_id| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateCategory {
                id: issue_id,
                category_id: category_id.map(CategoryId::new),
            });
    })
}

/// EstimatedHoursPopup用の現在値を取得する。
pub fn current_estimated_hours(store: &Store, issue_id: IssueId) -> Option<f64> {
    store.get_issue(issue_id).0.estimated_hours
}

/// EstimatedHoursPopupの入力結果からUpdateEstimatedHoursをdispatchするobserverを組み立てる。
pub fn estimated_hours_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(Option<f64>)> {
    Box::new(move |estimated_hours| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateEstimatedHours {
                id: issue_id,
                estimated_hours,
            });
    })
}

/// StartDatePopup用の現在値(選択済み開始日)を取得する。
pub fn current_start_date(store: &Store, issue_id: IssueId) -> Option<DateTime<Local>> {
    store.get_issue(issue_id).0.start_date
}

/// DueDatePopup用の現在値(選択済み期日)を取得する。
pub fn current_due_date(store: &Store, issue_id: IssueId) -> Option<DateTime<Local>> {
    store.get_issue(issue_id).0.due_date
}

/// StartDatePopupの選択結果からUpdateStartDateをdispatchするobserverを組み立てる。
pub fn start_date_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(DateTime<Local>)> {
    Box::new(move |date| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateStartDate {
                id: issue_id,
                start_date: Some(date),
            });
    })
}

/// DueDatePopupの選択結果からUpdateDueDateをdispatchするobserverを組み立てる。
pub fn due_date_popup_observer(
    dispatcher: Rc<RefCell<Dispatcher>>,
    issue_id: IssueId,
) -> Box<dyn FnMut(DateTime<Local>)> {
    Box::new(move |date| {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDueDate {
                id: issue_id,
                due_date: Some(date),
            });
    })
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
    fn tracker_options_are_sorted_and_focus_the_current_tracker() {
        let dispatcher = loaded_dispatcher();

        let (items, focused_index) = build_tracker_options(dispatcher.store(), 3.into());

        assert_eq!(
            items,
            vec![
                (1, "Bug".to_string()),
                (2, "Feature".to_string()),
                (3, "Support".to_string()),
            ]
        );
        assert_eq!(focused_index, 2);
    }

    #[test]
    fn project_options_are_sorted_and_focus_the_current_project() {
        let dispatcher = loaded_dispatcher();

        let (items, focused_index) = build_project_options(dispatcher.store(), 3.into());

        assert_eq!(
            items,
            vec![
                (1, "Sample Project".to_string()),
                (2, "Sample Project 2".to_string()),
            ]
        );
        assert_eq!(focused_index, 0);
    }

    #[test]
    fn priority_options_are_sorted_and_focus_the_current_priority() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdatePriority {
            id: 3.into(),
            priority_id: PriorityId::new(3),
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_priority_options(dispatcher.store(), 3.into());

        assert_eq!(
            items,
            vec![
                (1, "major".to_string()),
                (2, "minor".to_string()),
                (3, "critical".to_string()),
                (4, "blocker".to_string()),
            ]
        );
        assert_eq!(focused_index, 2);
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
        let project_id = dispatcher
            .store()
            .get_issue(IssueId::new(3))
            .0
            .issue
            .project_id;
        let current_target_version_id = dispatcher
            .store()
            .get_target_versions(project_id)
            .into_iter()
            .next()
            .map(|target_version| target_version.id)
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
        let project_id = dispatcher
            .store()
            .get_issue(IssueId::new(3))
            .0
            .issue
            .project_id;
        let current_category_id = dispatcher
            .store()
            .get_categories(project_id)
            .into_iter()
            .next()
            .map(|category| category.id)
            .expect("fixture has at least one category");
        dispatcher.dispatch(IssueAction::UpdateCategory {
            id: 3.into(),
            category_id: Some(current_category_id),
        });
        dispatcher.consume_action();

        let (items, focused_index) = build_category_options(dispatcher.store(), 3.into());

        assert_eq!(items[focused_index].0, current_category_id.get());
    }

    fn shared_loaded_dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(loaded_dispatcher()))
    }

    #[test]
    fn issue_status_observer_dispatches_update_status_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = issue_status_popup_observer(dispatcher.clone(), 3.into());

        observer(Some(2));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.issue.status_id,
            IssueStatusId::new(2)
        );
    }

    #[test]
    fn issue_status_observer_ignores_none() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = issue_status_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn tracker_observer_dispatches_update_tracker_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = tracker_popup_observer(dispatcher.clone(), 3.into());

        observer(Some(1));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.tracker_id,
            TrackerId::new(1)
        );
    }

    #[test]
    fn tracker_observer_ignores_none() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = tracker_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn project_observer_dispatches_update_project_when_another_project_is_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = project_popup_observer(dispatcher.clone(), 3.into());

        observer(Some(2));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.issue.project_id,
            ProjectId::new(2)
        );
    }

    #[test]
    fn project_observer_ignores_none_and_the_current_project() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = project_popup_observer(dispatcher.clone(), 3.into());

        observer(None);
        observer(Some(1));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn priority_observer_dispatches_update_priority_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = priority_popup_observer(dispatcher.clone(), 3.into());

        observer(Some(2));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.priority_id,
            PriorityId::new(2)
        );
    }

    #[test]
    fn priority_observer_ignores_none() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = priority_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn assigned_to_observer_dispatches_update_assigned_to_even_when_cleared() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = assigned_to_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.assigned_to_id,
            None
        );
    }

    #[test]
    fn target_version_observer_dispatches_update_target_version_even_when_cleared() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = target_version_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.target_version_id,
            None
        );
    }

    #[test]
    fn done_ratio_observer_ignores_none() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = done_ratio_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn done_ratio_observer_dispatches_update_done_ratio_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = done_ratio_popup_observer(dispatcher.clone(), 3.into());

        observer(Some(40));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(dispatcher.borrow().store().get_issue(3).0.done_ratio, 40);
    }

    #[test]
    fn category_observer_dispatches_update_category_even_when_cleared() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = category_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(dispatcher.borrow().store().get_issue(3).0.category_id, None);
    }

    #[test]
    fn estimated_hours_observer_dispatches_update_estimated_hours_even_when_cleared() {
        let dispatcher = shared_loaded_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateEstimatedHours {
                id: 3.into(),
                estimated_hours: Some(1.5),
            });
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            current_estimated_hours(dispatcher.borrow().store(), 3.into()),
            Some(1.5)
        );
        let mut observer = estimated_hours_popup_observer(dispatcher.clone(), 3.into());

        observer(None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            current_estimated_hours(dispatcher.borrow().store(), 3.into()),
            None
        );
    }

    fn sample_date() -> DateTime<Local> {
        crate::test_support::local_datetime("2026-02-17T00:00:00+09:00")
    }

    #[test]
    fn current_start_date_reads_the_issue_start_date() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdateStartDate {
            id: 3.into(),
            start_date: Some(sample_date()),
        });
        dispatcher.consume_action();

        assert_eq!(
            current_start_date(dispatcher.store(), 3.into()),
            Some(sample_date())
        );
    }

    #[test]
    fn current_due_date_reads_the_issue_due_date() {
        let mut dispatcher = loaded_dispatcher();
        dispatcher.dispatch(IssueAction::UpdateDueDate {
            id: 3.into(),
            due_date: Some(sample_date()),
        });
        dispatcher.consume_action();

        assert_eq!(
            current_due_date(dispatcher.store(), 3.into()),
            Some(sample_date())
        );
    }

    #[test]
    fn start_date_observer_dispatches_update_start_date_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = start_date_popup_observer(dispatcher.clone(), 3.into());

        observer(sample_date());

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.start_date,
            Some(sample_date())
        );
    }

    #[test]
    fn due_date_observer_dispatches_update_due_date_when_selected() {
        let dispatcher = shared_loaded_dispatcher();
        let mut observer = due_date_popup_observer(dispatcher.clone(), 3.into());

        observer(sample_date());

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher.borrow().store().get_issue(3).0.due_date,
            Some(sample_date())
        );
    }
}
