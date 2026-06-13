// ABOUTME: Logs 命令实现 - 读取 daemon 日志尾部并可持续追踪新增内容。

use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

const DEFAULT_LINE_COUNT: usize = 50;
const FOLLOW_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, PartialEq, Eq)]
pub struct LogOptions {
    line_count: usize,
    follow: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            line_count: DEFAULT_LINE_COUNT,
            follow: false,
        }
    }
}

pub fn cmd_logs(log_path: &Path, args: &[String]) -> Result<()> {
    let options = parse_log_options(args)?;
    if !log_path.exists() {
        println!("日志文件不存在，daemon 可能未曾运行过");
        return Ok(());
    }

    let content = std::fs::read_to_string(log_path)
        .with_context(|| format!("无法读取日志文件: {}", log_path.display()))?;
    if content.is_empty() {
        println!("日志为空");
        if options.follow {
            follow_log(log_path)?;
        }
        return Ok(());
    }

    print_tail(&content, options.line_count)?;

    if options.follow {
        follow_log(log_path)?;
    }

    Ok(())
}

fn parse_log_options(args: &[String]) -> Result<LogOptions> {
    let mut options = LogOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "-n" => {
                let value = args
                    .get(index + 1)
                    .context("用法: tm-watcher logs [-n <行数>] [--follow]")?;
                options.line_count = value
                    .parse::<usize>()
                    .with_context(|| format!("无效行数: {value}"))?;
                if options.line_count == 0 {
                    anyhow::bail!("行数必须大于 0");
                }
                index += 2;
            }
            "--follow" => {
                options.follow = true;
                index += 1;
            }
            other => anyhow::bail!("未知 logs 参数: {other}"),
        }
    }

    Ok(options)
}

fn print_tail(content: &str, line_count: usize) -> Result<()> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(line_count);
    let tail = lines[start..].join("\n");

    if !tail.is_empty() {
        println!("{tail}");
    }
    std::io::stdout().flush()?;

    Ok(())
}

fn follow_log(log_path: &Path) -> Result<()> {
    let mut offset = std::fs::metadata(log_path)?.len();

    loop {
        std::thread::sleep(FOLLOW_POLL_INTERVAL);

        let metadata = std::fs::metadata(log_path)
            .with_context(|| format!("无法读取日志文件状态: {}", log_path.display()))?;
        if metadata.len() < offset {
            offset = 0;
        }
        if metadata.len() == offset {
            continue;
        }

        let mut file = std::fs::File::open(log_path)
            .with_context(|| format!("无法读取日志文件: {}", log_path.display()))?;
        file.seek(SeekFrom::Start(offset))?;

        let mut chunk = String::new();
        file.read_to_string(&mut chunk)?;
        print!("{chunk}");
        std::io::stdout().flush()?;
        offset = metadata.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_log_options() {
        assert_eq!(parse_log_options(&[]).unwrap(), LogOptions::default());
    }

    #[test]
    fn parses_line_count_and_follow_in_any_order() {
        let args = vec!["--follow".to_string(), "-n".to_string(), "20".to_string()];

        assert_eq!(
            parse_log_options(&args).unwrap(),
            LogOptions {
                line_count: 20,
                follow: true,
            }
        );
    }
}
