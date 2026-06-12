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
    /// 确认延迟秒数（目录创建后等待此时长再排除）
    #[serde(default = "default_confirmation_delay")]
    pub confirmation_delay_seconds: u64,
    /// 删除目录时是否自动清理数据库记录
    #[serde(default = "default_cleanup_on_delete")]
    pub cleanup_on_delete: bool,
    /// 定期清理间隔（小时）
    #[serde(default = "default_interval_hours")]
    pub interval_hours: u64,
}

fn default_confirmation_delay() -> u64 {
    5
}

fn default_cleanup_on_delete() -> bool {
    true
}

fn default_interval_hours() -> u64 {
    24
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
            confirmation_delay_seconds: default_confirmation_delay(),
            cleanup_on_delete: default_cleanup_on_delete(),
            interval_hours: default_interval_hours(),
        }
    }

    /// 加载配置文件；不存在时自动生成默认配置
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("无法读取配置文件: {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("配置文件格式错误: {}", path.display()))?;

            if config.interval_hours == 0 {
                anyhow::bail!("配置错误: interval_hours 不能为 0");
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_loads_with_default_interval_hours() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // 写入一个不包含 interval_hours 的旧配置
        let old_config = r#"
watch_paths = ["~/test"]
exclude_rules = ["node_modules"]
confirmation_delay_seconds = 5
cleanup_on_delete = true
"#;
        std::fs::write(&config_path, old_config).unwrap();

        let config = Config::load_or_create(&config_path).unwrap();
        assert_eq!(config.interval_hours, 24);
    }

    #[test]
    fn test_config_default_includes_interval_hours() {
        let config = Config::default_config();
        assert_eq!(config.interval_hours, 24);
    }

    #[test]
    fn test_config_rejects_zero_interval_hours() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let bad_config = r#"
watch_paths = ["~/test"]
exclude_rules = ["node_modules"]
interval_hours = 0
"#;
        std::fs::write(&config_path, bad_config).unwrap();

        let result = Config::load_or_create(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("interval_hours"));
    }
}
