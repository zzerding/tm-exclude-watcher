// ABOUTME: 配置管理 - 定义排除规则，支持默认配置生成与加载

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_RESTART_HINT: &str =
    "配置已更新，请运行 'tm-watcher daemon restart' 重启 daemon 使其生效";

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigUpdate {
    Updated(String),
    Skipped(String),
}

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

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建配置目录: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("无法写入配置文件: {}", path.display()))
    }

    pub fn add_rule(&mut self, rule: &str) -> Result<ConfigUpdate> {
        if self.exclude_rules.iter().any(|existing| existing == rule) {
            return Ok(ConfigUpdate::Skipped(format!(
                "排除规则已存在，跳过: {rule}"
            )));
        }

        self.exclude_rules.push(rule.to_string());
        Ok(ConfigUpdate::Updated(format!("已添加排除规则: {rule}")))
    }

    pub fn add_path(&mut self, path: &Path) -> Result<ConfigUpdate> {
        let path = path.to_path_buf();
        let existing_paths: Vec<PathBuf> = self.watch_paths.iter().map(expand_tilde_path).collect();

        if let Some(existing) = existing_paths.iter().find(|existing| *existing == &path) {
            return Ok(ConfigUpdate::Skipped(format!(
                "监控路径已存在，跳过: {}",
                existing.display()
            )));
        }

        if let Some(parent) = existing_paths
            .iter()
            .find(|existing| path.starts_with(existing.as_path()))
        {
            return Ok(ConfigUpdate::Skipped(format!(
                "监控路径已被 {} 覆盖，跳过",
                parent.display()
            )));
        }

        let covered_paths: Vec<&PathBuf> = existing_paths
            .iter()
            .filter(|existing| existing.starts_with(&path))
            .collect();
        if !covered_paths.is_empty() {
            let covered = covered_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("、");
            return Ok(ConfigUpdate::Skipped(format!(
                "监控路径将覆盖 {covered}，跳过"
            )));
        }

        self.watch_paths.push(path.display().to_string());
        Ok(ConfigUpdate::Updated(format!(
            "已添加监控路径: {}",
            path.display()
        )))
    }

    pub fn render(&self, config_path: &Path) -> String {
        let mut output = format!("配置文件: {}\n\n", format_config_path(config_path));
        output.push_str("监控路径:\n");
        append_items(&mut output, &self.watch_paths);
        output.push('\n');
        output.push_str("排除规则:\n");
        append_items(&mut output, &self.exclude_rules);
        output.push('\n');
        output.push_str(&format!(
            "确认延迟: {} 秒\n",
            self.confirmation_delay_seconds
        ));
        output.push_str(&format!(
            "删除时清理: {}\n",
            if self.cleanup_on_delete { "是" } else { "否" }
        ));
        output.push_str(&format!("定期清理间隔: {} 小时\n", self.interval_hours));
        output
    }
}

pub fn expand_tilde_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let path_text = path.to_string_lossy();
    if path_text == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    if let Some(rest) = path_text.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path.to_path_buf()
}

fn format_config_path(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.display().to_string();
    };
    match path.strip_prefix(&home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

fn append_items(output: &mut String, items: &[String]) {
    if items.is_empty() {
        output.push_str("  无\n");
        return;
    }
    for item in items {
        output.push_str(&format!("  - {item}\n"));
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
