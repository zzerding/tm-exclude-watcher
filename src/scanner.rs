// ABOUTME: 目录扫描器，递归查找匹配规则的目录并排除

use crate::{Database, RuleMatcher, TmUtilTrait};
use std::path::Path;
use walkdir::WalkDir;

pub struct Scanner {
    rule_matcher: RuleMatcher,
    database: Database,
    tmutil: Box<dyn TmUtilTrait>,
}

pub struct ScanResult {
    pub excluded_count: usize,
    pub skipped_count: usize,
}

impl Scanner {
    pub fn new(database: Database, rules: Vec<String>, tmutil: Box<dyn TmUtilTrait>) -> Self {
        Self {
            rule_matcher: RuleMatcher::new(rules),
            database,
            tmutil,
        }
    }

    pub fn scan(&self, root: &Path) -> Result<ScanResult, String> {
        let mut excluded_count = 0;
        let mut skipped_count = 0;

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            if let Some(matched_rule) = self.rule_matcher.should_exclude(path) {
                let path_str = path.to_str().ok_or("无效路径")?;

                if self.database.is_recorded(path_str).map_err(|e| e.to_string())? {
                    skipped_count += 1;
                    continue;
                }

                self.tmutil.add_exclusion(path)?;

                let size = Self::calculate_size(path);
                self.database
                    .record_exclusion(path_str, matched_rule, size)
                    .map_err(|e| e.to_string())?;

                excluded_count += 1;
            }
        }

        Ok(ScanResult {
            excluded_count,
            skipped_count,
        })
    }

    fn calculate_size(_path: &Path) -> i64 {
        0
    }
}
