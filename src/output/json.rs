use serde::Serialize;

pub fn print<T: Serialize>(value: &T) {
    let json = serde_json::to_string_pretty(value).unwrap_or_else(|e| {
        format!(
            r#"{{"success":false,"error":{{"code":"SERIALIZE_ERROR","message":"{}"}}}}"#,
            e
        )
    });
    println!("{json}");
}
