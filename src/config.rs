// ABOUTME: 配置管理 - 定义排除规则，支持默认配置生成与加载

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// 要监控的根目录列表
    #[serde(default)]
    pub watch_paths: Vec<String>,
    /// 目录名匹配规则列表
    #[serde(default)]
    pub exclude_rules: Vec<String>,
}

impl Config {
    /// 默认配置：常见开发目录 + 常见构建产物规则
    pub fn default_config() -> Self {
        Self {
            watch_paths: vec![
                "~/Documents".to_string(),
                "~/Projects".to_string(),
                "~/Code".to_string(),
                "~/Developer".to_string(),
            ],
            exclude_rules: vec![
                "node_modules",
                "target",
                "vendor",
                ".venv",
                "venv",
                "virtualenv",
                "__pycache__",
                "build",
                "dist",
                ".next",
                ".nuxt",
                ".cache",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }

    /// 加载配置文件；不存在时自动生成默认配置
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("无法读取配置文件: {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("配置文件格式错误: {}", path.display()))?;
            return Ok(config);
        }

        // 生成默认配置
        let config = Self::default_config();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建配置目录: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(path, content)
            .with_context(|| format!("无法写入配置文件: {}", path.display()))?;

        Ok(config)
    }
}
