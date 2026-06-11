// ABOUTME: 遍历基准测试 - 对比 walkdir 与 jwalk（含剪枝），隔离 tmutil/数据库开销
// 用法: cargo run --release --example bench_walk -- <walkdir|jwalk> <path>

use std::time::Instant;

const RULES: &[&str] = &[
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
];

fn matches(name: &str) -> bool {
    RULES.contains(&name)
}

fn bench_walkdir(path: &str) -> (usize, usize) {
    let mut visited = 0;
    let mut matched = 0;
    let mut it = walkdir::WalkDir::new(path).follow_links(false).into_iter();
    while let Some(entry) = it.next() {
        let Ok(entry) = entry else { continue };
        visited += 1;
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if matches(&name) {
            matched += 1;
            it.skip_current_dir(); // 剪枝
        }
    }
    (visited, matched)
}

fn bench_jwalk(path: &str) -> (usize, usize) {
    let mut visited = 0;
    let mut matched = 0;
    let walker = jwalk::WalkDir::new(path)
        .follow_links(false)
        .skip_hidden(false) // 规则含 .venv/.cache 等隐藏目录，必须关闭
        .process_read_dir(|_depth, _path, _state, children| {
            // 并行读目录时剪枝：匹配的子目录不再下钻
            for child in children.iter_mut().flatten() {
                if child.file_type.is_dir() {
                    let name = child.file_name.to_string_lossy();
                    if matches(&name) {
                        child.read_children_path = None; // 剪枝
                    }
                }
            }
        });
    for entry in walker {
        let Ok(entry) = entry else { continue };
        visited += 1;
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if matches(&name) {
            matched += 1;
        }
    }
    (visited, matched)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let method = args
        .get(1)
        .expect("用法: bench_walk <walkdir|jwalk> <path>");
    let path = args
        .get(2)
        .expect("用法: bench_walk <walkdir|jwalk> <path>");

    let start = Instant::now();
    let (visited, matched) = match method.as_str() {
        "walkdir" => bench_walkdir(path),
        "jwalk" => bench_jwalk(path),
        _ => panic!("未知方法: {}", method),
    };
    let elapsed = start.elapsed();

    println!(
        "{}: 访问 {} 个条目, 匹配 {} 个目录, 耗时 {:.3}s",
        method,
        visited,
        matched,
        elapsed.as_secs_f64()
    );
}
