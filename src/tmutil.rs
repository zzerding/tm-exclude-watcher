// ABOUTME: Time Machine 工具抽象，提供真实和测试实现

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub trait TmUtilTrait: Send + Sync {
    fn add_exclusion(&self, path: &Path) -> Result<(), String>;
    fn is_excluded(&self, path: &Path) -> Result<bool, String>;
    fn remove_exclusion(&self, path: &Path) -> Result<(), String>;
}

pub struct RealTmUtil;

impl TmUtilTrait for RealTmUtil {
    fn add_exclusion(&self, path: &Path) -> Result<(), String> {
        let path_str = path.to_str().ok_or("路径包含无效 UTF-8 字符")?;

        let output = Command::new("tmutil")
            .args(["addexclusion", path_str])
            .output()
            .map_err(|e| format!("执行 tmutil 失败: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "tmutil addexclusion 失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    fn is_excluded(&self, path: &Path) -> Result<bool, String> {
        let path_str = path.to_str().ok_or("路径包含无效 UTF-8 字符")?;

        let output = Command::new("tmutil")
            .args(["isexcluded", path_str])
            .output()
            .map_err(|e| format!("执行 tmutil 失败: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "tmutil isexcluded 失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("[Excluded]"))
    }

    fn remove_exclusion(&self, path: &Path) -> Result<(), String> {
        let path_str = path.to_str().ok_or("路径包含无效 UTF-8 字符")?;

        let output = Command::new("tmutil")
            .args(["removeexclusion", path_str])
            .output()
            .map_err(|e| format!("执行 tmutil 失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // PathNotFound (Error -43) 不算错误
            if stderr.contains("does not exist")
                || stderr.contains("not found")
                || stderr.contains("Error (-43)") {
                return Ok(());
            }
            return Err(format!("tmutil removeexclusion 失败: {}", stderr));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct MockTmUtil {
    calls: Arc<Mutex<Vec<PathBuf>>>,
    remove_calls: Arc<Mutex<Vec<PathBuf>>>,
}

impl MockTmUtil {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            remove_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn was_called_with(&self, path: &Path) -> bool {
        let calls = self.calls.lock().unwrap();
        calls.iter().any(|p| p == path)
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    pub fn remove_was_called_with(&self, path: &Path) -> bool {
        let calls = self.remove_calls.lock().unwrap();
        calls.iter().any(|p| p == path)
    }
}

impl TmUtilTrait for MockTmUtil {
    fn add_exclusion(&self, path: &Path) -> Result<(), String> {
        self.calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn is_excluded(&self, _path: &Path) -> Result<bool, String> {
        Ok(false)
    }

    fn remove_exclusion(&self, path: &Path) -> Result<(), String> {
        self.remove_calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}
