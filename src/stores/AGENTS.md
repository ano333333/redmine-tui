# stores

このファイルのスコープは `src/stores/` です。

- 状態機械に反するActionは `panic!` で拒否してください。`Result` で呼び出し元へ返さないでください。
- 外部起因の失敗は、messageを載せた失敗Actionで状態を戻してください。再試行に必要な状態があれば、再試行できる状態へ戻し、messageを状態の一部として持たせてください。
- 重複した非同期要求の追い越しは、request IDの一致で判定して捨ててください。暗黙の状態判定で捨てないでください。重複を事前条件で排除できるActionは、想定外の状態に届いた完了を `panic!` で拒否してください。
- 存在が呼び出し元の事前条件であるgetterは `get_xxx` とし、不在ならpanicさせてください。不在が状態や件数を表すgetterは `Option` を返し、`IssueStore` の単体getterでは `try_get_xxx` と命名してください。マスターデータの単体getterは `Option` を返してください。
- 呼び出し側で取得値の存在を前提にするときは、`unwrap()` ではなく `expect(...)` で不変条件を説明してください。
- テストでは、`test_support` の `sample_*` で組み立てたentityを、詳細取得と同じActionでStoreへ渡してください。テスト専用の登録Actionは作らないでください。
