use chrono::{DateTime, Local, TimeZone};

use super::{Dispatcher, NoticeAction, NoticeId, Store};
use uuid::Uuid;

fn notice_id(value: u128) -> NoticeId {
    Uuid::from_u128(value).into()
}

fn created_at() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("test timestamp is valid")
}

fn push(store: &mut Store, id: NoticeId, message: &str) {
    push_at(store, id, message, created_at());
}

fn push_at(store: &mut Store, id: NoticeId, message: &str, created_at: DateTime<Local>) {
    store.consume_action(
        NoticeAction::Push {
            id,
            message: message.to_string(),
            created_at,
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

fn now_after(created_at: DateTime<Local>, elapsed: chrono::Duration) -> DateTime<Local> {
    created_at + elapsed
}

#[test]
fn push_adds_a_notice_visible_through_get_notices() {
    let mut store = Store::new();

    push(&mut store, notice_id(1), "editor failed");

    let notices = store.get_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].id, notice_id(1));
    assert_eq!(notices[0].message, "editor failed");
    assert_eq!(notices[0].created_at, created_at());
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
    let created = created_at();

    push_at(&mut store, notice_id(1), "expired", created);
    store.update(now_after(created, chrono::Duration::seconds(6)));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_keeps_notices_before_the_visible_duration() {
    let mut store = Store::new();
    let created = created_at();

    push_at(&mut store, notice_id(1), "fresh", created);
    store.update(now_after(created, chrono::Duration::milliseconds(4900)));

    assert_eq!(notice_ids(&store), vec![notice_id(1)]);
}

#[test]
fn update_treats_the_exact_boundary_as_expired() {
    let mut store = Store::new();
    let created = created_at();

    push_at(&mut store, notice_id(1), "boundary", created);
    store.update(now_after(created, chrono::Duration::seconds(5)));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_removes_all_notices_that_expire_together() {
    let mut store = Store::new();
    let created = created_at();

    push_at(&mut store, notice_id(1), "first", created);
    push_at(&mut store, notice_id(2), "second", created);
    push_at(&mut store, notice_id(3), "third", created);

    store.update(now_after(created, chrono::Duration::seconds(5)));

    assert!(store.get_notices().is_empty());
}

#[test]
fn update_removes_only_expired_notices_and_keeps_fresh_ones() {
    let mut store = Store::new();
    let old = created_at();
    let fresh = created_at() + chrono::Duration::seconds(3);

    push_at(&mut store, notice_id(1), "old", old);
    push_at(&mut store, notice_id(2), "fresh", fresh);

    store.update(now_after(old, chrono::Duration::seconds(5)));

    assert_eq!(notice_ids(&store), vec![notice_id(2)]);
}

#[test]
fn update_does_nothing_when_there_are_no_notices() {
    let mut store = Store::new();

    store.update(created_at());

    assert!(store.get_notices().is_empty());
}

#[test]
fn dispatcher_accepts_notice_action_directly() {
    let mut dispatcher = Dispatcher::new();

    dispatcher.dispatch(NoticeAction::Push {
        id: notice_id(1),
        message: "upload failed".to_string(),
        created_at: created_at(),
    });
    dispatcher.consume_action();

    assert_eq!(dispatcher.store().get_notices().len(), 1);
    assert_eq!(dispatcher.store().get_notices()[0].message, "upload failed");
}
