// ABOUTME: 列表输出格式化 - 将数据库排除记录渲染为 CLI 可打印文本

use crate::database::ExclusionRecord;
use std::cmp::Ordering;
use std::path::Path;

pub fn format_exclusion_list(records: &[ExclusionRecord]) -> String {
    if records.is_empty() {
        return "没有排除记录。".to_string();
    }

    let home_dir = dirs::home_dir();
    let mut sorted_records: Vec<&ExclusionRecord> = records.iter().collect();
    sorted_records.sort_by(|left, right| compare_records(left, right));

    let rows: Vec<ListRow> = sorted_records
        .iter()
        .enumerate()
        .map(|(index, record)| ListRow {
            index: (index + 1).to_string(),
            size: format_size(record.size_bytes),
            rule: record.rule.clone(),
            checked_at: format_checked_at(record.last_checked_at.as_deref()),
            path: format_path(&record.path, home_dir.as_deref()),
        })
        .collect();

    let widths = ColumnWidths::from_rows(&rows);
    let total_size: i64 = records.iter().filter_map(|record| record.size_bytes).sum();
    let unknown_count = records
        .iter()
        .filter(|record| record.size_bytes.is_none())
        .count();

    let mut output = format!(
        "排除记录: {} 条，已知大小合计 {}，未知大小 {} 条\n\n",
        records.len(),
        format_size(Some(total_size)),
        unknown_count
    );
    output.push_str(&format!(
        "{}  {}  {}  {}  路径\n",
        pad_display("#", widths.index),
        pad_display("大小", widths.size),
        pad_display("规则", widths.rule),
        pad_display("检查时间", widths.checked_at)
    ));

    for row in rows {
        output.push_str(&format!(
            "{}  {}  {}  {}  {}\n",
            pad_display(&row.index, widths.index),
            pad_display(&row.size, widths.size),
            pad_display(&row.rule, widths.rule),
            pad_display(&row.checked_at, widths.checked_at),
            row.path
        ));
    }

    output
}

struct ListRow {
    index: String,
    size: String,
    rule: String,
    checked_at: String,
    path: String,
}

struct ColumnWidths {
    index: usize,
    size: usize,
    rule: usize,
    checked_at: usize,
}

impl ColumnWidths {
    fn from_rows(rows: &[ListRow]) -> Self {
        let mut widths = Self {
            index: display_width("#"),
            size: display_width("大小"),
            rule: display_width("规则"),
            checked_at: display_width("检查时间"),
        };

        for row in rows {
            widths.index = widths.index.max(display_width(&row.index));
            widths.size = widths.size.max(display_width(&row.size));
            widths.rule = widths.rule.max(display_width(&row.rule));
            widths.checked_at = widths.checked_at.max(display_width(&row.checked_at));
        }

        widths
    }
}

fn compare_records(left: &ExclusionRecord, right: &ExclusionRecord) -> Ordering {
    match (left.size_bytes, right.size_bytes) {
        (Some(left_size), Some(right_size)) => right_size
            .cmp(&left_size)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.rule.cmp(&right.rule)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left
            .path
            .cmp(&right.path)
            .then_with(|| left.rule.cmp(&right.rule)),
    }
}

fn format_checked_at(checked_at: Option<&str>) -> String {
    match checked_at {
        Some(value) if value.chars().count() > 16 => value.chars().take(16).collect(),
        Some(value) => value.to_string(),
        None => "未检查".to_string(),
    }
}

fn format_path(path: &Path, home_dir: Option<&Path>) -> String {
    let Some(home_dir) = home_dir else {
        return path.display().to_string();
    };

    if path == home_dir {
        return "~".to_string();
    }

    match path.strip_prefix(home_dir) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

fn format_size(size_bytes: Option<i64>) -> String {
    match size_bytes {
        Some(size) if size < 1024 => format!("{size} B"),
        Some(size) => format_known_size(size),
        None => "未知".to_string(),
    }
}

fn format_known_size(size_bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut value = size_bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    let value_text = format!("{value:.1}");
    let trimmed = value_text.strip_suffix(".0").unwrap_or(&value_text);
    format!("{} {}", trimmed, UNITS[unit_index])
}

fn pad_display(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{}{}", value, " ".repeat(padding))
}

fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum()
}
