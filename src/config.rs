// ABOUTME: 配置管理 - 定义排除规则

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exclude_rules: Vec<String>,
}
