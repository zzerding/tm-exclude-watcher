// ABOUTME: 配置管理测试 - 验证默认配置生成和加载

use tempfile::TempDir;
use tm_watcher::{Config, ConfigUpdate};

#[test]
fn test_load_or_create_generates_default_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // 配置文件不存在时，自动生成默认配置
    let config = Config::load_or_create(&config_path).unwrap();

    // 断言：文件已创建
    assert!(config_path.exists());

    // 断言：包含默认规则
    assert!(config.exclude_rules.contains(&"node_modules".to_string()));
    assert!(config.exclude_rules.contains(&"target".to_string()));
    assert!(config.exclude_rules.contains(&".venv".to_string()));

    // 断言：包含默认监控路径
    assert!(!config.watch_paths.is_empty());
}

#[test]
fn test_load_existing_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // 写入自定义配置
    std::fs::write(
        &config_path,
        r#"
watch_paths = ["/tmp/my-code"]
exclude_rules = ["custom_dir"]
"#,
    )
    .unwrap();

    let config = Config::load_or_create(&config_path).unwrap();

    // 断言：加载的是自定义配置，不是默认值
    assert_eq!(config.exclude_rules, vec!["custom_dir".to_string()]);
    assert_eq!(config.watch_paths, vec!["/tmp/my-code".to_string()]);
}

#[test]
fn test_load_or_create_creates_parent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("nested/dir/config.toml");

    // 父目录不存在时也能成功创建
    let config = Config::load_or_create(&config_path).unwrap();
    assert!(config_path.exists());
    assert!(!config.exclude_rules.is_empty());
}

#[test]
fn test_config_update_adds_rule_and_skips_duplicate() {
    let mut config = Config::default_config();

    assert_eq!(
        config.add_rule(".pytest_cache").unwrap(),
        ConfigUpdate::Updated("已添加排除规则: .pytest_cache".to_string())
    );
    assert_eq!(
        config.add_rule(".pytest_cache").unwrap(),
        ConfigUpdate::Skipped("排除规则已存在，跳过: .pytest_cache".to_string())
    );
}

#[test]
fn test_config_update_detects_parent_child_path_conflicts_by_components() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config {
        watch_paths: vec![
            temp_dir
                .path()
                .join("foo/bar")
                .to_string_lossy()
                .into_owned(),
        ],
        ..Default::default()
    };

    assert_eq!(
        config
            .add_path(&temp_dir.path().join("foo/bar/baz"))
            .unwrap(),
        ConfigUpdate::Skipped(format!(
            "监控路径已被 {} 覆盖，跳过",
            temp_dir.path().join("foo/bar").display()
        ))
    );
    assert_eq!(
        config.add_path(&temp_dir.path().join("foo")).unwrap(),
        ConfigUpdate::Skipped(format!(
            "监控路径将覆盖 {}，跳过",
            temp_dir.path().join("foo/bar").display()
        ))
    );
    assert_eq!(
        config.add_path(&temp_dir.path().join("foo/bar2")).unwrap(),
        ConfigUpdate::Updated(format!(
            "已添加监控路径: {}",
            temp_dir.path().join("foo/bar2").display()
        ))
    );
}

#[test]
fn test_expand_tilde_path_expands_bare_home() {
    assert_eq!(
        tm_watcher::expand_tilde_path("~"),
        dirs::home_dir().unwrap()
    );
}
