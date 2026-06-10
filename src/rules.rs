// ABOUTME: 规则匹配器，基于 basename 精确匹配目录

use std::path::Path;

#[derive(Clone)]
pub struct RuleMatcher {
    exclude_rules: Vec<String>,
}

impl RuleMatcher {
    pub fn new(rules: Vec<String>) -> Self {
        Self {
            exclude_rules: rules,
        }
    }

    pub fn should_exclude(&self, path: &Path) -> Option<&str> {
        if let Some(basename) = path.file_name() {
            if let Some(basename_str) = basename.to_str() {
                return self.exclude_rules.iter()
                    .find(|rule| rule.as_str() == basename_str)
                    .map(|s| s.as_str());
            }
        }
        None
    }
}
