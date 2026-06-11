// ABOUTME: 列表输出格式化 - 将数据库排除记录渲染为 CLI 可打印文本

use crate::database::ExclusionRecord;

pub fn format_exclusion_list(records: &[ExclusionRecord]) -> String {
    if records.is_empty() {
        return "没有排除记录。".to_string();
    }

    let mut output = String::from("排除记录:\n");
    for record in records {
        let size = format_size_mb(record.size_bytes);
        output.push_str(&format!(
            "- 路径: {}\n  规则: {}\n  大小: {}\n  创建时间: {}\n",
            record.path.display(),
            record.rule,
            size,
            record.created_at
        ));
    }

    output
}

fn format_size_mb(size_bytes: Option<i64>) -> String {
    match size_bytes {
        Some(size) => format!("{:.2} MB", size as f64 / 1024.0 / 1024.0),
        None => "未知".to_string(),
    }
}
