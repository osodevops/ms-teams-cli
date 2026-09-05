use serde::Serialize;

/// Print a single object as key: value lines.
pub fn print_object<T: Serialize>(data: &T) {
    let value = match serde_json::to_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    if let serde_json::Value::Object(map) = value {
        for (key, val) in &map {
            let display = match val {
                serde_json::Value::Null => String::new(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            super::write_stdout_line(&format!("{key}: {display}"));
        }
    } else {
        // Fallback for non-objects
        super::write_stdout_line(&serde_json::to_string_pretty(&value).unwrap_or_default());
    }
}

/// Print a list of objects as TSV with a header row.
pub fn print_list<T: Serialize>(items: &[T]) {
    write_list(items, super::write_stdout_line);
}

fn write_list<T: Serialize>(items: &[T], mut write_line: impl FnMut(&str)) {
    if items.is_empty() {
        return;
    }

    // Collect all items as JSON values
    let values: Vec<serde_json::Value> = items
        .iter()
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect();

    if !matches!(values.first(), Some(serde_json::Value::Object(_))) {
        return;
    }
    // Optional fields can first appear in any row. Keep the same sorted key
    // order as serde's object maps, but include columns from every object.
    let headers: std::collections::BTreeSet<&String> = values
        .iter()
        .filter_map(serde_json::Value::as_object)
        .flat_map(|map| map.keys())
        .collect();
    let headers: Vec<&str> = headers.into_iter().map(String::as_str).collect();

    // Print header row
    write_line(&headers.join("\t"));

    // Print data rows
    for val in &values {
        if let serde_json::Value::Object(map) = val {
            let row: Vec<String> = headers
                .iter()
                .map(|h| match map.get(*h) {
                    Some(serde_json::Value::Null) | None => String::new(),
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                })
                .collect();
            write_line(&row.join("\t"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::ChatMessage;

    #[test]
    fn message_subject_column_does_not_depend_on_first_message() {
        let untitled: ChatMessage = serde_json::from_value(serde_json::json!({
            "id": "untitled", "subject": null
        }))
        .unwrap();
        let titled: ChatMessage = serde_json::from_value(serde_json::json!({
            "id": "titled", "subject": "Release α & <plan>"
        }))
        .unwrap();
        for messages in [
            vec![untitled.clone(), titled.clone()],
            vec![titled, untitled],
        ] {
            let mut lines = Vec::new();
            write_list(&messages, |line| lines.push(line.to_owned()));
            assert_eq!(lines[0], "id\tsubject");
            assert!(lines.iter().any(|line| line == "untitled\t"));
            assert!(lines
                .iter()
                .any(|line| line == "titled\tRelease α & <plan>"));
        }
    }

    #[test]
    fn list_keeps_nested_values_and_blanks_for_missing_columns() {
        let values = [
            serde_json::json!({"a": 1}),
            serde_json::json!({"b": {"nested": true}}),
        ];
        let mut lines = Vec::new();
        write_list(&values, |line| lines.push(line.to_owned()));
        assert_eq!(lines, ["a\tb", "1\t", "\t{\"nested\":true}"]);
    }

    #[test]
    fn empty_and_non_object_lists_produce_no_output() {
        for values in [vec![], vec![serde_json::json!(1)]] {
            write_list(&values, |_| panic!("unexpected output"));
        }
    }
}
