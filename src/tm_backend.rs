// ABOUTME: Time Machine tmutil 封装 - 执行真实排除、移除和状态查询命令。

use anyhow::Result;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// 真实后端 - 调用 macOS tmutil 命令
#[derive(Clone)]
pub struct RealTmBackend {
    tmutil_path: PathBuf,
}

impl RealTmBackend {
    pub fn new() -> Self {
        Self {
            tmutil_path: PathBuf::from("tmutil"),
        }
    }

    pub fn with_tmutil_path(tmutil_path: impl Into<PathBuf>) -> Self {
        Self {
            tmutil_path: tmutil_path.into(),
        }
    }

    fn command(&self) -> Command {
        Command::new(&self.tmutil_path)
    }

    /// 检查 Time Machine 是否已配置
    pub fn check_configured(&self) -> Result<bool> {
        let output = self
            .command()
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

    /// 添加排除目录
    pub fn add_exclusion(&self, path: &Path) -> Result<()> {
        let output = self
            .command()
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

    /// 移除排除目录
    pub fn remove_exclusion(&self, path: &Path) -> Result<()> {
        let output = self
            .command()
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

    /// 检查目录是否已被排除
    pub fn is_excluded(&self, path: &Path) -> Result<bool> {
        let output = self
            .command()
            .arg("isexcluded")
            .arg(path)
            .env("TM_WATCHER_TMUTIL_METHOD", "is_excluded")
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
        parse_isexcluded_output(&stdout, 1).map(|statuses| statuses[0])
    }

    /// 批量检查目录是否已被排除
    pub fn are_excluded(&self, paths: &[PathBuf]) -> Result<Vec<bool>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let output = self
            .command()
            .arg("isexcluded")
            .args(paths)
            .env("TM_WATCHER_TMUTIL_METHOD", "are_excluded")
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

impl Default for RealTmBackend {
    fn default() -> Self {
        Self::new()
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
            let status_line = line.trim_start();
            if status_line.starts_with("[Excluded]") {
                Some(true)
            } else if status_line.starts_with("[Included]") {
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
    fn parse_isexcluded_output_ignores_status_text_in_path() {
        let stdout = "\
[Included]    /tmp/[Excluded]/target
 [Excluded]    /tmp/[Included]/node_modules
";

        assert_eq!(
            parse_isexcluded_output(stdout, 2).unwrap(),
            vec![false, true]
        );
    }

    #[test]
    fn parse_isexcluded_output_rejects_missing_status_lines() {
        let err = parse_isexcluded_output("[Excluded]    /tmp/a\n", 2).unwrap_err();

        assert!(err.to_string().contains("预期 2 条"));
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::RealTmBackend;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    pub(crate) struct FakeTmutil {
        _temp_dir: TempDir,
        script_path: PathBuf,
        calls_path: PathBuf,
        _excluded_path: PathBuf,
        fail_add_path: PathBuf,
        _unconfigured_path: PathBuf,
    }

    impl FakeTmutil {
        pub(crate) fn new() -> Self {
            Self::create(false)
        }

        pub(crate) fn new_unconfigured() -> Self {
            Self::create(true)
        }

        pub(crate) fn backend(&self) -> RealTmBackend {
            RealTmBackend::with_tmutil_path(self.script_path.clone())
        }

        pub(crate) fn add_exclusion_call_count(&self) -> usize {
            read_lines(&self.calls_path)
                .into_iter()
                .filter(|line| line.starts_with("addexclusion\t"))
                .count()
        }

        pub(crate) fn fail_next_add_other(&self, message: &str) {
            fs::write(&self.fail_add_path, message).unwrap();
        }

        fn create(is_unconfigured: bool) -> Self {
            let temp_dir = TempDir::new().unwrap();
            let script_path = temp_dir.path().join("tmutil");
            let calls_path = temp_dir.path().join("calls.log");
            let excluded_path = temp_dir.path().join("excluded.log");
            let fail_add_path = temp_dir.path().join("fail-add");
            let unconfigured_path = temp_dir.path().join("unconfigured");

            fs::File::create(&calls_path).unwrap();
            fs::File::create(&excluded_path).unwrap();
            if is_unconfigured {
                fs::File::create(&unconfigured_path).unwrap();
            }

            write_script(
                &script_path,
                &calls_path,
                &excluded_path,
                &fail_add_path,
                &unconfigured_path,
            );

            Self {
                _temp_dir: temp_dir,
                script_path,
                calls_path,
                _excluded_path: excluded_path,
                fail_add_path,
                _unconfigured_path: unconfigured_path,
            }
        }
    }

    fn write_script(
        script_path: &Path,
        calls_path: &Path,
        excluded_path: &Path,
        fail_add_path: &Path,
        unconfigured_path: &Path,
    ) {
        let script = format!(
            r#"#!/bin/sh
cmd="$1"
shift
calls='{}'
excluded='{}'
fail_add='{}'
unconfigured='{}'

case "$cmd" in
  destinationinfo)
    if [ -f "$unconfigured" ]; then
      echo "No destinations configured" >&2
      exit 1
    fi
    exit 0
    ;;
  addexclusion)
    path="$1"
    printf 'addexclusion\t%s\n' "$path" >> "$calls"
    if [ -f "$fail_add" ]; then
      cat "$fail_add" >&2
      rm "$fail_add"
      exit 1
    fi
    printf '%s\n' "$path" >> "$excluded"
    exit 0
    ;;
  isexcluded)
    method="${{TM_WATCHER_TMUTIL_METHOD:-unknown}}"
    printf 'isexcluded\t%s\t%s' "$method" "$#" >> "$calls"
    for path in "$@"; do
      printf '\t%s' "$path" >> "$calls"
    done
    printf '\n' >> "$calls"
    for path in "$@"; do
      if grep -Fxq "$path" "$excluded"; then
        printf '[Excluded]    %s\n' "$path"
      else
        printf '[Included]    %s\n' "$path"
      fi
    done
    exit 0
    ;;
  removeexclusion)
    path="$1"
    printf 'removeexclusion\t%s\n' "$path" >> "$calls"
    grep -Fxv "$path" "$excluded" > "$excluded.tmp" || true
    mv "$excluded.tmp" "$excluded"
    exit 0
    ;;
esac

echo "unexpected tmutil command: $cmd" >&2
exit 2
"#,
            calls_path.display(),
            excluded_path.display(),
            fail_add_path.display(),
            unconfigured_path.display()
        );

        let mut file = fs::File::create(script_path).unwrap();
        file.write_all(script.as_bytes()).unwrap();

        #[cfg(unix)]
        {
            let mut permissions = file.metadata().unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(script_path, permissions).unwrap();
        }
    }

    fn read_lines(path: &PathBuf) -> Vec<String> {
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }
}
