// ABOUTME: 进程级 fake tmutil - 通过脚本模拟 tmutil 命令并记录调用。

use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tm_watcher::RealTmBackend;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub struct FakeTmutil {
    _temp_dir: TempDir,
    script_path: PathBuf,
    calls_path: PathBuf,
    excluded_path: PathBuf,
    fail_add_path: PathBuf,
    fail_remove_path: PathBuf,
    fail_remove_not_found_path: PathBuf,
    fail_batch_path: PathBuf,
    _unconfigured_path: PathBuf,
}

impl FakeTmutil {
    pub fn new() -> Self {
        Self::create(false)
    }

    pub fn new_unconfigured() -> Self {
        Self::create(true)
    }

    pub fn backend(&self) -> RealTmBackend {
        RealTmBackend::with_tmutil_path(self.script_path.clone())
    }

    pub fn add_exclusion(&self, path: &Path) -> Result<()> {
        self.backend().add_exclusion(path)
    }

    pub fn get_excluded_paths(&self) -> Vec<PathBuf> {
        read_lines(&self.excluded_path)
            .into_iter()
            .map(PathBuf::from)
            .collect()
    }

    pub fn add_exclusion_call_count(&self) -> usize {
        self.count_calls("addexclusion\t")
    }

    pub fn remove_exclusion_call_count(&self) -> usize {
        self.count_calls("removeexclusion\t")
    }

    pub fn is_excluded_call_count(&self) -> usize {
        read_lines(&self.calls_path)
            .into_iter()
            .filter(|line| line.starts_with("isexcluded\tis_excluded\t"))
            .count()
    }

    pub fn are_excluded_call_count(&self) -> usize {
        read_lines(&self.calls_path)
            .into_iter()
            .filter(|line| line.starts_with("isexcluded\tare_excluded\t"))
            .count()
    }

    pub fn fail_next_add_other(&self, message: &str) {
        fs::write(&self.fail_add_path, message).unwrap();
    }

    pub fn fail_next_remove_path_not_found(&self) {
        fs::write(&self.fail_remove_not_found_path, "").unwrap();
    }

    pub fn fail_next_remove_other(&self, message: &str) {
        fs::write(&self.fail_remove_path, message).unwrap();
    }

    pub fn fail_next_batch_other(&self, message: &str) {
        fs::write(&self.fail_batch_path, message).unwrap();
    }

    fn create(is_unconfigured: bool) -> Self {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("tmutil");
        let calls_path = temp_dir.path().join("calls.log");
        let excluded_path = temp_dir.path().join("excluded.log");
        let fail_add_path = temp_dir.path().join("fail-add");
        let fail_remove_path = temp_dir.path().join("fail-remove");
        let fail_remove_not_found_path = temp_dir.path().join("fail-remove-not-found");
        let fail_batch_path = temp_dir.path().join("fail-batch");
        let unconfigured_path = temp_dir.path().join("unconfigured");

        fs::File::create(&calls_path).unwrap();
        fs::File::create(&excluded_path).unwrap();
        if is_unconfigured {
            fs::File::create(&unconfigured_path).unwrap();
        }

        write_script(ScriptPaths {
            script_path: &script_path,
            calls_path: &calls_path,
            excluded_path: &excluded_path,
            fail_add_path: &fail_add_path,
            fail_remove_path: &fail_remove_path,
            fail_remove_not_found_path: &fail_remove_not_found_path,
            fail_batch_path: &fail_batch_path,
            unconfigured_path: &unconfigured_path,
        });

        Self {
            _temp_dir: temp_dir,
            script_path,
            calls_path,
            excluded_path,
            fail_add_path,
            fail_remove_path,
            fail_remove_not_found_path,
            fail_batch_path,
            _unconfigured_path: unconfigured_path,
        }
    }

    fn count_calls(&self, prefix: &str) -> usize {
        read_lines(&self.calls_path)
            .into_iter()
            .filter(|line| line.starts_with(prefix))
            .count()
    }
}

struct ScriptPaths<'a> {
    script_path: &'a Path,
    calls_path: &'a Path,
    excluded_path: &'a Path,
    fail_add_path: &'a Path,
    fail_remove_path: &'a Path,
    fail_remove_not_found_path: &'a Path,
    fail_batch_path: &'a Path,
    unconfigured_path: &'a Path,
}

fn write_script(paths: ScriptPaths<'_>) {
    let script = format!(
        r#"#!/bin/sh
cmd="$1"
shift
calls='{}'
excluded='{}'
fail_add='{}'
fail_remove='{}'
fail_remove_not_found='{}'
fail_batch='{}'
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
    if ! grep -Fxq "$path" "$excluded"; then
      printf '%s\n' "$path" >> "$excluded"
    fi
    exit 0
    ;;
  removeexclusion)
    path="$1"
    printf 'removeexclusion\t%s\n' "$path" >> "$calls"
    if [ -f "$fail_remove_not_found" ]; then
      rm "$fail_remove_not_found"
      echo "$path: Error (-43) while attempting to change exclusion setting." >&2
      exit 1
    fi
    if [ -f "$fail_remove" ]; then
      cat "$fail_remove" >&2
      rm "$fail_remove"
      exit 1
    fi
    grep -Fxv "$path" "$excluded" > "$excluded.tmp" || true
    mv "$excluded.tmp" "$excluded"
    exit 0
    ;;
  isexcluded)
    method="${{TM_WATCHER_TMUTIL_METHOD:-unknown}}"
    count="$#"
    printf 'isexcluded\t%s\t%s' "$method" "$count" >> "$calls"
    for path in "$@"; do
      printf '\t%s' "$path" >> "$calls"
    done
    printf '\n' >> "$calls"
    if [ "$count" -gt 1 ] && [ -f "$fail_batch" ]; then
      cat "$fail_batch" >&2
      rm "$fail_batch"
      exit 1
    fi
    for path in "$@"; do
      if grep -Fxq "$path" "$excluded"; then
        printf '[Excluded]    %s\n' "$path"
      else
        printf '[Included]    %s\n' "$path"
      fi
    done
    exit 0
    ;;
esac

echo "unexpected tmutil command: $cmd" >&2
exit 2
"#,
        paths.calls_path.display(),
        paths.excluded_path.display(),
        paths.fail_add_path.display(),
        paths.fail_remove_path.display(),
        paths.fail_remove_not_found_path.display(),
        paths.fail_batch_path.display(),
        paths.unconfigured_path.display()
    );

    let mut file = fs::File::create(paths.script_path).unwrap();
    file.write_all(script.as_bytes()).unwrap();

    #[cfg(unix)]
    {
        let mut permissions = file.metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(paths.script_path, permissions).unwrap();
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(ToOwned::to_owned)
        .collect()
}
