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
