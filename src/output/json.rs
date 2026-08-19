use serde::Serialize;

pub fn print<T: Serialize>(value: &T) {
    super::write_stdout_line(&serialize(value));
}

pub fn serialize<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| {
        format!(
            r#"{{"success":false,"error":{{"code":"SERIALIZE_ERROR","message":"{}"}}}}"#,
            e
        )
    })
}
