use super::{Dispatcher, Notice, NoticeAction, NoticeId, Store};
use uuid::Uuid;

fn notice_id(value: u128) -> NoticeId {
    Uuid::from_u128(value).into()
}

fn notice_with_elapsed(elapsed: chrono::Duration) -> Notice {
    Notice {
        id: notice_id(1),
        message: "notice".to_string(),
        elapsed,
    }
}

fn push(store: &mut Store, id: NoticeId, message: &str) {
    store.consume_action(
        NoticeAction::Push {
            id,
            message: message.to_string(),
        }
        .into(),
    );
}

fn dismiss(store: &mut Store, id: NoticeId) {
    store.consume_action(NoticeAction::Dismiss { id }.into());
}

fn notice_ids(store: &Store) -> Vec<NoticeId> {
    store.get_notices().iter().map(|notice| notice.id).collect()
}

#[test]
fn push_adds_a_notice_visible_through_get_notices() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "editor failed");

    let notices = store.get_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].id, notice_id(1));
    assert_eq!(notices[0].message, "editor failed");
    assert_eq!(notices[0].elapsed, chrono::Duration::zero());
}

#[test]
fn pushes_keep_the_insertion_order() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "first");
    push(&mut store, notice_id(2), "second");
    push(&mut store, notice_id(3), "third");

    assert_eq!(
        store
            .get_notices()
            .iter()
            .map(|notice| notice.message.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
}

#[test]
fn pushing_the_same_id_twice_adds_only_one_notice() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "first");
    push(&mut store, notice_id(1), "duplicate");

    let notices = store.get_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].message, "first");
}

#[test]
fn dismiss_removes_only_the_matching_notice() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "first");
    push(&mut store, notice_id(2), "second");
    push(&mut store, notice_id(3), "third");

    dismiss(&mut store, notice_id(2));

    assert_eq!(notice_ids(&store), vec![notice_id(1), notice_id(3)]);
}

#[test]
fn dismissing_a_missing_id_changes_nothing() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "first");

    dismiss(&mut store, notice_id(99));

    assert_eq!(notice_ids(&store), vec![notice_id(1)]);
}

#[test]
fn update_removes_notices_past_the_visible_duration() {
    let mut store = Store::new();
    push(&mut store, notice_id(1), "expired");
    store.update(chrono::Duration::seconds(6));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_keeps_notices_before_the_visible_duration() {
    let mut store = Store::new();
    push(&mut store, notice_id(1), "fresh");
    store.update(chrono::Duration::milliseconds(4900));

    assert_eq!(notice_ids(&store), vec![notice_id(1)]);
}

#[test]
fn update_treats_the_exact_boundary_as_expired() {
    let mut store = Store::new();
    push(&mut store, notice_id(1), "boundary");
    store.update(chrono::Duration::seconds(5));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_removes_all_notices_that_expire_together() {
    let mut store = Store::new();
    push(&mut store, notice_id(1), "first");
    push(&mut store, notice_id(2), "second");
    push(&mut store, notice_id(3), "third");

    store.update(chrono::Duration::seconds(5));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_removes_only_expired_notices_and_keeps_fresh_ones() {
    let mut store = Store::new();
    push(&mut store, notice_id(1), "old");
    store.update(chrono::Duration::seconds(3));
    push(&mut store, notice_id(2), "fresh");

    store.update(chrono::Duration::seconds(2));

    assert_eq!(notice_ids(&store), vec![notice_id(2)]);
}

#[test]
fn update_does_nothing_when_there_are_no_notices() {
    let mut store = Store::new();

    store.update(chrono::Duration::zero());

    assert!(store.get_notices().is_empty());
}

#[test]
fn is_visible_is_true_when_elapsed_is_zero() {
    let notice = notice_with_elapsed(chrono::Duration::zero());

    assert!(notice.is_visible());
}

#[test]
fn is_visible_is_true_just_before_the_visible_duration() {
    let notice = notice_with_elapsed(chrono::Duration::milliseconds(4999));

    assert!(notice.is_visible());
}

#[test]
fn is_visible_is_false_at_the_visible_duration_boundary() {
    let notice = notice_with_elapsed(chrono::Duration::seconds(5));

    assert!(!notice.is_visible());
}

#[test]
fn dispatcher_accepts_notice_action_directly() {
    let mut dispatcher = Dispatcher::new();

    dispatcher.dispatch(NoticeAction::Push {
        id: notice_id(1),
        message: "upload failed".to_string(),
    });
    dispatcher.consume_action();

    assert_eq!(dispatcher.store().get_notices().len(), 1);
    assert_eq!(dispatcher.store().get_notices()[0].message, "upload failed");
}
