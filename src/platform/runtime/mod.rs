use std::any::Any;

use crate::stores::Action;

/// background taskからrunnerへ返す、実行環境に依存しない完了通知。
///
/// task側ではStoreを更新せず、runnerがこの通知をUI thread上でAction dispatchへ接続する。
pub enum BackgroundCompletion {
    Succeeded(Vec<Action>),
    Panicked { message: String },
}

/// 通常の文字列panic payloadをmessageへ変換し、それ以外の型には共通文言を返す。
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload.downcast_ref::<&str>().map_or_else(
        || {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "worker task panicked".to_string())
        },
        |message| message.to_string(),
    )
}

impl From<Vec<Action>> for BackgroundCompletion {
    fn from(actions: Vec<Action>) -> Self {
        Self::Succeeded(actions)
    }
}

impl From<Box<dyn Any + Send>> for BackgroundCompletion {
    fn from(payload: Box<dyn Any + Send>) -> Self {
        Self::Panicked {
            message: panic_message(payload.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_success_value_to_succeeded() {
        let completion = BackgroundCompletion::from(vec![
            Action::WorkerPanicked {
                message: "first".to_string(),
            },
            Action::WorkerPanicked {
                message: "second".to_string(),
            },
        ]);
        let BackgroundCompletion::Succeeded(actions) = completion else {
            panic!("expected successful completion")
        };
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::WorkerPanicked { message } if message == "first"));
        assert!(matches!(&actions[1], Action::WorkerPanicked { message } if message == "second"));
    }

    #[test]
    fn maps_panic_payloads_to_panicked() {
        let cases: Vec<(Box<dyn Any + Send>, &str)> = vec![
            (Box::new("worker panic"), "worker panic"),
            (Box::new("worker panic".to_string()), "worker panic"),
            (Box::new(42_u8), "worker task panicked"),
        ];
        for (payload, expected_message) in cases {
            assert!(matches!(
                BackgroundCompletion::from(payload),
                BackgroundCompletion::Panicked { message } if message == expected_message
            ));
        }
    }
}
