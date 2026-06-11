// ABOUTME: Time Machine 后端抽象 - 隔离 tmutil 调用以便测试

use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Time Machine 操作的抽象接口
pub trait TmBackend: Send + Sync {
    /// 检查 Time Machine 是否已配置
    fn check_configured(&self) -> Result<bool>;

    /// 添加排除目录
    fn add_exclusion(&self, path: &Path) -> Result<()>;

    /// 检查目录是否已被排除
    fn is_excluded(&self, path: &Path) -> Result<bool>;
}

/// 测试用的假后端 - 使用内存 HashSet 模拟
#[derive(Clone)]
pub struct FakeTmBackend {
    excluded_paths: Arc<Mutex<HashSet<PathBuf>>>,
    is_configured: bool,
}

impl Default for FakeTmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeTmBackend {
    pub fn new() -> Self {
        Self {
            excluded_paths: Arc::new(Mutex::new(HashSet::new())),
            is_configured: true,
        }
    }

    /// 创建未配置的后端（测试用）
    pub fn new_unconfigured() -> Self {
        Self {
            excluded_paths: Arc::new(Mutex::new(HashSet::new())),
            is_configured: false,
        }
    }

    /// 获取所有已排除的路径（测试用）
    pub fn get_excluded_paths(&self) -> Vec<PathBuf> {
        self.excluded_paths
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect()
    }
}

impl TmBackend for FakeTmBackend {
    fn check_configured(&self) -> Result<bool> {
        Ok(self.is_configured)
    }
    fn add_exclusion(&self, path: &Path) -> Result<()> {
        self.excluded_paths
            .lock()
            .unwrap()
            .insert(path.to_path_buf());
        Ok(())
    }

    fn is_excluded(&self, path: &Path) -> Result<bool> {
        Ok(self.excluded_paths.lock().unwrap().contains(path))
    }
}

/// 真实后端 - 调用 macOS tmutil 命令
pub struct RealTmBackend;

impl RealTmBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RealTmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TmBackend for RealTmBackend {
    fn check_configured(&self) -> Result<bool> {
        let output = std::process::Command::new("tmutil")
            .arg("destinationinfo")
            .output()
            .map_err(|e| anyhow::anyhow!("无法执行 tmutil: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // tmutil 未配置时输出 "No destinations configured"
        if stdout.contains("No destinations configured")
            || stderr.contains("No destinations configured")
        {
            return Ok(false);
        }

        Ok(output.status.success())
    }

    fn add_exclusion(&self, path: &Path) -> Result<()> {
        let output = std::process::Command::new("tmutil")
            .arg("addexclusion")
            .arg(path)
            .output()
            .map_err(|e| anyhow::anyhow!("无法执行 tmutil: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "tmutil addexclusion 失败 ({}): {}",
                path.display(),
                stderr.trim()
            );
        }

        Ok(())
    }

    fn is_excluded(&self, path: &Path) -> Result<bool> {
        let output = std::process::Command::new("tmutil")
            .arg("isexcluded")
            .arg(path)
            .output()
            .map_err(|e| anyhow::anyhow!("无法执行 tmutil: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "tmutil isexcluded 失败 ({}): {}",
                path.display(),
                stderr.trim()
            );
        }

        // 输出格式: "[Excluded]    /path/to/dir" 或 "[Included]    /path/to/dir"
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("[Excluded]"))
    }
}
