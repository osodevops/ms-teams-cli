use comfy_table::{presets::UTF8_FULL, Table};

pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(headers);
    for row in rows {
        table.add_row(row);
    }
    super::write_stdout_line(&table.to_string());
}
