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
            println!("{key}: {display}");
        }
    } else {
        // Fallback for non-objects
        println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
    }
}

/// Print a list of objects as TSV with a header row.
pub fn print_list<T: Serialize>(items: &[T]) {
    if items.is_empty() {
        return;
    }

    // Collect all items as JSON values
    let values: Vec<serde_json::Value> = items
        .iter()
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect();

    // Extract headers from the first object
    let headers: Vec<String> = match values.first() {
        Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        _ => return,
    };

    // Print header row
    println!("{}", headers.join("\t"));

    // Print data rows
    for val in &values {
        if let serde_json::Value::Object(map) = val {
            let row: Vec<String> = headers
                .iter()
                .map(|h| {
                    match map.get(h) {
                        Some(serde_json::Value::Null) | None => String::new(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                    }
                })
                .collect();
            println!("{}", row.join("\t"));
        }
    }
}
