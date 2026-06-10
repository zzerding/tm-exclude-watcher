// ABOUTME: tm-watcher CLI 入口，处理命令行参数并执行扫描

use std::env;
use std::path::Path;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use tm_watcher::{Cleaner, Database, RealTmUtil, RuleMatcher, Scanner, Watcher};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("用法: tm-watcher <命令> [参数]");
        eprintln!("命令:");
        eprintln!("  scan <目录>  - 扫描目录并排除匹配规则的子目录");
        eprintln!("  list         - 列出所有排除记录");
        eprintln!("  clean        - 清理无效记录并更新元数据");
        eprintln!("  watch <目录> - 实时监控目录并自动排除");
        process::exit(1);
    }

    let command = &args[1];

    // 初始化数据库
    let db_path = dirs::data_local_dir()
        .map(|p| p.join("tm-watcher/exclusions.db"))
        .unwrap_or_else(|| {
            eprintln!("错误: 无法获取本地数据目录");
            process::exit(1);
        });

    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("错误: 无法创建数据库目录: {}", e);
                process::exit(1);
            });
        }
    }

    let db = Database::new(&db_path).unwrap_or_else(|e| {
        eprintln!("错误: 无法初始化数据库: {}", e);
        process::exit(1);
    });

    match command.as_str() {
        "scan" => handle_scan(&args, db),
        "list" => handle_list(db),
        "clean" => handle_clean(db),
        "watch" => handle_watch(&args, db).await,
        _ => {
            eprintln!("错误: 未知命令 '{}'", command);
            eprintln!("支持的命令: scan, list, clean, watch");
            process::exit(1);
        }
    }
}

fn handle_scan(args: &[String], db: Database) {
    if args.len() < 3 {
        eprintln!("用法: tm-watcher scan <目录路径>");
        process::exit(1);
    }

    let path = &args[2];
    let scan_path = Path::new(path);

    if !scan_path.exists() {
        eprintln!("错误: 路径不存在: {}", path);
        process::exit(1);
    }

    let rules = vec![
        "node_modules".to_string(),
        "target".to_string(),
        ".venv".to_string(),
        "__pycache__".to_string(),
        "dist".to_string(),
        "build".to_string(),
    ];

    let scanner = Scanner::new(db, rules, Box::new(RealTmUtil));

    println!("开始扫描: {}", path);

    match scanner.scan(scan_path) {
        Ok(result) => {
            println!("\n扫描完成！");
            println!("  新排除目录: {}", result.excluded_count);
            println!("  已记录跳过: {}", result.skipped_count);
        }
        Err(e) => {
            eprintln!("错误: 扫描失败: {}", e);
            process::exit(1);
        }
    }
}

fn handle_list(db: Database) {
    match db.list_all() {
        Ok(records) => {
            if records.is_empty() {
                println!("没有排除记录");
                return;
            }

            println!("排除记录列表 ({} 条):\n", records.len());
            println!("{:<50} {:<15} {:<10} {}", "路径", "规则", "大小", "创建时间");
            println!("{}", "-".repeat(90));

            for record in records {
                println!(
                    "{:<50} {:<15} {:>8.2} MB {}",
                    &record.path,
                    &record.rule,
                    record.size_mb(),
                    record.created_at_display()
                );
            }
        }
        Err(e) => {
            eprintln!("错误: 无法读取记录: {}", e);
            process::exit(1);
        }
    }
}

fn handle_clean(db: Database) {
    let cleaner = Cleaner::new(db, Box::new(RealTmUtil));

    println!("开始清理...");

    match cleaner.clean() {
        Ok(stats) => {
            println!("\n清理完成！");
            println!("  已移除记录: {}", stats.removed_count);
            println!("  已更新记录: {}", stats.updated_count);
            if stats.error_count > 0 {
                println!("  错误数量: {}", stats.error_count);
                for err in stats.errors {
                    eprintln!("    - {}", err);
                }
            }
        }
        Err(e) => {
            eprintln!("错误: 清理失败: {}", e);
            process::exit(1);
        }
    }
}

async fn handle_watch(args: &[String], db: Database) {
    if args.len() < 3 {
        eprintln!("用法: tm-watcher watch <目录路径>");
        process::exit(1);
    }

    let path = &args[2];
    let watch_path = Path::new(path);

    if !watch_path.exists() {
        eprintln!("错误: 路径不存在: {}", path);
        process::exit(1);
    }

    let rules = vec![
        "node_modules".to_string(),
        "target".to_string(),
        ".venv".to_string(),
        "__pycache__".to_string(),
        "dist".to_string(),
        "build".to_string(),
    ];

    let rule_matcher = RuleMatcher::new(rules);
    let watcher = Watcher::new(
        db,
        rule_matcher,
        Arc::new(RealTmUtil),
        Duration::from_secs(5),
    );

    // 设置 Ctrl+C 处理
    let watcher_ref = &watcher;
    tokio::select! {
        result = watcher_ref.watch(watch_path) => {
            if let Err(e) = result {
                eprintln!("监控错误: {:?}", e);
                process::exit(1);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n收到停止信号，正在退出...");
        }
    }
}
