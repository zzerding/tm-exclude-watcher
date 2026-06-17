// ABOUTME: Time Machine 后端抽象 - 隔离 tmutil 调用以便测试

use anyhow::Result;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum TmBackendError {
    PathNotFound(PathBuf),
}

impl fmt::Display for TmBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotFound(path) => write!(formatter, "路径不存在: {}", path.display()),
        }
    }
}

impl std::error::Error for TmBackendError {}

/// Time Machine 操作的抽象接口
pub trait TmBackend: Send + Sync {
    /// 检查 Time Machine 是否已配置
    fn check_configured(&self) -> Result<bool>;

    /// 添加排除目录
    fn add_exclusion(&self, path: &Path) -> Result<()>;

    /// 移除排除目录
    fn remove_exclusion(&self, path: &Path) -> Result<()>;

    /// 检查目录是否已被排除
    fn is_excluded(&self, path: &Path) -> Result<bool>;

    /// 批量检查目录是否已被排除
    fn are_excluded(&self, paths: &[PathBuf]) -> Result<Vec<bool>>;
}

/// 测试用的假后端 - 使用内存 HashSet 模拟
#[derive(Clone)]
pub struct FakeTmBackend {
    excluded_paths: Arc<Mutex<HashSet<PathBuf>>>,
    add_exclusion_calls: Arc<AtomicUsize>,
    remove_exclusion_calls: Arc<AtomicUsize>,
    is_excluded_calls: Arc<AtomicUsize>,
    are_excluded_calls: Arc<AtomicUsize>,
    next_add_error: Arc<Mutex<Option<String>>>,
    next_remove_error: Arc<Mutex<Option<FakeRemoveError>>>,
    next_batch_error: Arc<Mutex<Option<String>>>,
    is_configured: bool,
}

#[derive(Clone)]
enum FakeRemoveError {
    PathNotFound,
    Other(String),
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
            add_exclusion_calls: Arc::new(AtomicUsize::new(0)),
            remove_exclusion_calls: Arc::new(AtomicUsize::new(0)),
            is_excluded_calls: Arc::new(AtomicUsize::new(0)),
            are_excluded_calls: Arc::new(AtomicUsize::new(0)),
            next_add_error: Arc::new(Mutex::new(None)),
            next_remove_error: Arc::new(Mutex::new(None)),
            next_batch_error: Arc::new(Mutex::new(None)),
            is_configured: true,
        }
    }

    /// 创建未配置的后端（测试用）
    pub fn new_unconfigured() -> Self {
        Self {
            excluded_paths: Arc::new(Mutex::new(HashSet::new())),
            add_exclusion_calls: Arc::new(AtomicUsize::new(0)),
            remove_exclusion_calls: Arc::new(AtomicUsize::new(0)),
            is_excluded_calls: Arc::new(AtomicUsize::new(0)),
            are_excluded_calls: Arc::new(AtomicUsize::new(0)),
            next_add_error: Arc::new(Mutex::new(None)),
            next_remove_error: Arc::new(Mutex::new(None)),
            next_batch_error: Arc::new(Mutex::new(None)),
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

    /// 获取 add_exclusion 调用次数（测试用）
    pub fn add_exclusion_call_count(&self) -> usize {
        self.add_exclusion_calls.load(Ordering::SeqCst)
    }

    /// 获取 remove_exclusion 调用次数（测试用）
    pub fn remove_exclusion_call_count(&self) -> usize {
        self.remove_exclusion_calls.load(Ordering::SeqCst)
    }

    /// 获取 is_excluded 调用次数（测试用）
    pub fn is_excluded_call_count(&self) -> usize {
        self.is_excluded_calls.load(Ordering::SeqCst)
    }

    pub fn are_excluded_call_count(&self) -> usize {
        self.are_excluded_calls.load(Ordering::SeqCst)
    }

    pub fn fail_next_add_other(&self, message: &str) {
        *self.next_add_error.lock().unwrap() = Some(message.to_string());
    }

    pub fn fail_next_remove_path_not_found(&self) {
        *self.next_remove_error.lock().unwrap() = Some(FakeRemoveError::PathNotFound);
    }

    pub fn fail_next_remove_other(&self, message: &str) {
        *self.next_remove_error.lock().unwrap() = Some(FakeRemoveError::Other(message.to_string()));
    }

    pub fn fail_next_batch_other(&self, message: &str) {
        *self.next_batch_error.lock().unwrap() = Some(message.to_string());
    }
}

impl TmBackend for FakeTmBackend {
    fn check_configured(&self) -> Result<bool> {
        Ok(self.is_configured)
    }
    fn add_exclusion(&self, path: &Path) -> Result<()> {
        self.add_exclusion_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(message) = self.next_add_error.lock().unwrap().take() {
            return Err(anyhow::anyhow!(message));
        }

        self.excluded_paths
            .lock()
            .unwrap()
            .insert(path.to_path_buf());
        Ok(())
    }

    fn remove_exclusion(&self, path: &Path) -> Result<()> {
        self.remove_exclusion_calls.fetch_add(1, Ordering::SeqCst);

        if let Some(error) = self.next_remove_error.lock().unwrap().take() {
            match error {
                FakeRemoveError::PathNotFound => {
                    return Err(TmBackendError::PathNotFound(path.to_path_buf()).into());
                }
                FakeRemoveError::Other(message) => return Err(anyhow::anyhow!(message)),
            }
        }

        self.excluded_paths.lock().unwrap().remove(path);
        Ok(())
    }

    fn is_excluded(&self, path: &Path) -> Result<bool> {
        self.is_excluded_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.excluded_paths.lock().unwrap().contains(path))
    }

    fn are_excluded(&self, paths: &[PathBuf]) -> Result<Vec<bool>> {
        self.are_excluded_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(message) = self.next_batch_error.lock().unwrap().take() {
            return Err(anyhow::anyhow!(message));
        }

        let excluded_paths = self.excluded_paths.lock().unwrap();
        Ok(paths
            .iter()
            .map(|path| excluded_paths.contains(path))
            .collect())
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

    fn remove_exclusion(&self, path: &Path) -> Result<()> {
        let output = std::process::Command::new("tmutil")
            .arg("removeexclusion")
            .arg(path)
            .output()
            .map_err(|e| anyhow::anyhow!("无法执行 tmutil: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_path_not_found_output(&stderr) {
                return Err(TmBackendError::PathNotFound(path.to_path_buf()).into());
            }
            anyhow::bail!(
                "tmutil removeexclusion 失败 ({}): {}",
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

    fn are_excluded(&self, paths: &[PathBuf]) -> Result<Vec<bool>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let output = std::process::Command::new("tmutil")
            .arg("isexcluded")
            .args(paths)
            .output()
            .map_err(|e| anyhow::anyhow!("无法执行 tmutil: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tmutil isexcluded 批量检查失败: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_isexcluded_output(&stdout, paths.len())
    }
}

fn is_path_not_found_output(stderr: &str) -> bool {
    stderr.contains("No such file or directory")
        || stderr.contains("does not exist")
        || stderr.contains("Error (-43)")
}

fn parse_isexcluded_output(stdout: &str, expected_count: usize) -> Result<Vec<bool>> {
    let statuses = stdout
        .lines()
        .filter_map(|line| {
            if line.contains("[Excluded]") {
                Some(true)
            } else if line.contains("[Included]") {
                Some(false)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if statuses.len() != expected_count {
        anyhow::bail!(
            "tmutil isexcluded 返回 {} 条状态，预期 {} 条",
            statuses.len(),
            expected_count
        );
    }

    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use super::{is_path_not_found_output, parse_isexcluded_output};

    #[test]
    fn path_not_found_output_includes_tmutil_error_43() {
        let stderr =
            "/tmp/project/target: Error (-43) while attempting to change exclusion setting.";

        assert!(is_path_not_found_output(stderr));
    }

    #[test]
    fn path_not_found_output_keeps_existing_english_messages() {
        assert!(is_path_not_found_output("No such file or directory"));
        assert!(is_path_not_found_output("path does not exist"));
    }

    #[test]
    fn path_not_found_output_rejects_ordinary_tmutil_errors() {
        assert!(!is_path_not_found_output("Operation not permitted"));
        assert!(!is_path_not_found_output(
            "Error (-50) while attempting to change exclusion setting."
        ));
    }

    #[test]
    fn parse_isexcluded_output_reads_one_status_per_path() {
        let stdout = "\
[Excluded]    /tmp/a
[Included]    /tmp/b
";

        assert_eq!(
            parse_isexcluded_output(stdout, 2).unwrap(),
            vec![true, false]
        );
    }

    #[test]
    fn parse_isexcluded_output_rejects_missing_status_lines() {
        let err = parse_isexcluded_output("[Excluded]    /tmp/a\n", 2).unwrap_err();

        assert!(err.to_string().contains("预期 2 条"));
    }
}
