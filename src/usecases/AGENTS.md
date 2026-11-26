# usecases

このファイルのスコープは `src/usecases/` です。

- Storeを直接書き換えず、`Dispatcher::dispatch` でActionを積んでください。起動時の同期のActionはその場でdispatchし、非同期の処理は `UsecaseTask` として返してください。
- 完了後に別のUsecaseを起動するときは、Futureの中で起動せず、`UsecaseOutput::requests` に後続の `UsecaseRequest` を入れて返してください。
- 保存の失敗など一度だけ起きたことは、`UsecaseOutput::events` に `UsecaseEvent` として入れて返してください。イベントの受け取り手を前提にせず、イベントを契機に別のUsecaseを起動しないでください。
- 新しいUsecaseは `UsecaseRequest` にvariantを1つ足し、`start_usecase` から起動してください。ComponentからRedmineのUsecaseを直接呼ばせないでください。
