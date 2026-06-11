// ABOUTME: P3 手动集成测试 - 调用真实 tmutil（需要 macOS 且 Time Machine 已配置）
// 运行方式: cargo test --test manual_test -- --ignored
//
// 注意：系统临时目录（/var/folders）默认被 Time Machine 排除，
// 因此测试目录必须创建在用户主目录下。

use std::fs;
use std::path::PathBuf;
use tm_watcher::{RealTmBackend, TmBackend};

/// 在用户主目录下创建测试目录（系统临时目录默认已被 TM 排除，不可用）
fn create_test_dir() -> PathBuf {
    let home = dirs::home_dir().expect("无法获取主目录");
    let test_dir = home.join(format!(".tm-watcher-test-{}", std::process::id()));
    fs::create_dir_all(&test_dir).unwrap();
    test_dir
}

/// 验证真实 tmutil 排除与查询的端到端行为
/// sticky 排除（xattr）无需 sudo
#[test]
#[ignore = "需要 macOS 真实 tmutil 环境，手动运行"]
fn test_real_tmutil_add_and_check_exclusion() {
    let backend = RealTmBackend::new();

    let test_dir = create_test_dir();
    let node_modules = test_dir.join("node_modules");
    fs::create_dir(&node_modules).unwrap();

    // 初始状态：未排除
    assert!(!backend.is_excluded(&node_modules).unwrap());

    // 添加排除
    backend.add_exclusion(&node_modules).unwrap();

    // 验证已排除
    assert!(backend.is_excluded(&node_modules).unwrap());

    // 清理：删除测试目录（sticky 排除随目录删除自动失效）
    fs::remove_dir_all(&test_dir).unwrap();
}

/// 验证 Time Machine 配置检测不报错
#[test]
#[ignore = "需要 macOS 真实 tmutil 环境，手动运行"]
fn test_real_tmutil_check_configured() {
    let backend = RealTmBackend::new();
    // 只验证调用成功，不断言具体值（取决于机器是否配置了 Time Machine）
    let result = backend.check_configured();
    assert!(result.is_ok());
}
