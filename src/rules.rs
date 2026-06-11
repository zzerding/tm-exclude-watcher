// ABOUTME: 规则匹配引擎 - basename 精确匹配

use std::path::Path;

#[derive(Clone)]
pub struct RuleMatcher {
    rules: Vec<String>,
}

impl RuleMatcher {
    pub fn new(rules: Vec<String>) -> Self {
        Self { rules }
    }

    /// 检查路径的 basename 是否匹配任一规则
    /// 返回匹配的规则，如果不匹配返回 None
    pub fn matches(&self, path: &Path) -> Option<String> {
        let basename = path.file_name()?.to_str()?;
        self.matches_name(basename)
    }

    /// 检查目录名是否匹配任一规则（供并行遍历回调使用）
    pub fn matches_name(&self, name: &str) -> Option<String> {
        self.rules
            .iter()
            .find(|rule| rule.as_str() == name)
            .cloned()
    }
}
