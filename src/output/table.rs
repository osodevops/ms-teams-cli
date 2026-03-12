use comfy_table::{presets::UTF8_FULL, Table};

pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(headers);
    for row in rows {
        table.add_row(row);
    }
    println!("{table}");
}
