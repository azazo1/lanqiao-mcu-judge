use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

use crate::chip::Simulator;

#[derive(Debug, Parser)]
#[command(author, version, about = "STC15F2K60S2 + 蓝桥杯 4T 开发板仿真评测工具")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        hex: PathBuf,
        #[arg(long, conflicts_with = "stdin")]
        script: Option<PathBuf>,
        #[arg(long, default_value_t = false, conflicts_with = "script")]
        stdin: bool,
        #[arg(long, default_value_t = false)]
        trace_cpu: bool,
    },
    Repl {
        #[arg(long)]
        hex: PathBuf,
        #[arg(long, default_value_t = false)]
        trace_cpu: bool,
    },
    Dump {
        #[arg(long)]
        hex: PathBuf,
        #[arg(long, default_value_t = 0)]
        ms: u64,
    },
}

pub async fn run() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            hex,
            script,
            stdin,
            trace_cpu,
        } => {
            let sim = Simulator::from_hex_path(&hex, trace_cpu)
                .with_context(|| format!("加载 HEX 失败: {}", hex.display()))?;
            match (script, stdin) {
                (Some(path), false) if path.as_os_str() == "-" => {
                    crate::script::run_script_stdin(sim).context("执行标准输入脚本失败")?;
                }
                (Some(path), false) => {
                    crate::script::run_script(sim, &path)
                        .with_context(|| format!("执行脚本失败: {}", path.display()))?;
                }
                (None, true) => {
                    crate::script::run_script_stdin(sim).context("执行标准输入脚本失败")?;
                }
                (None, false) => {
                    crate::script::run_repl(sim).context("进入交互式 Rhai 失败")?;
                }
                (Some(_), true) => unreachable!(),
            }
        }
        Command::Repl { hex, trace_cpu } => {
            let sim = Simulator::from_hex_path(&hex, trace_cpu)
                .with_context(|| format!("加载 HEX 失败: {}", hex.display()))?;
            crate::script::run_repl(sim).context("进入交互式 Rhai 失败")?;
        }
        Command::Dump { hex, ms } => {
            let mut sim = Simulator::from_hex_path(&hex, false)
                .with_context(|| format!("加载 HEX 失败: {}", hex.display()))?;
            sim.run_ms(ms)?;
            println!("{}", sim.snapshot_text());
        }
    }
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
