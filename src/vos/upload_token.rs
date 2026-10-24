/// Redmineへ送ったファイル本体を、Issueへ紐付けるときに指定する値。
///
/// アプリは中身を解釈せず、受け取った値をそのまま紐付けに使う。Redmineでは1回しか使えない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadToken(String);

impl UploadToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
