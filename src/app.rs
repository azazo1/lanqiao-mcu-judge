use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

use crate::{chip::Simulator, wave::WaveCaptureOptions};

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
        #[command(flatten)]
        wave: WaveArgs,
    },
    Repl {
        #[arg(long)]
        hex: PathBuf,
        #[arg(long, default_value_t = false)]
        trace_cpu: bool,
        #[command(flatten)]
        wave: WaveArgs,
    },
    Dump {
        #[arg(long)]
        hex: PathBuf,
        #[arg(long, default_value_t = 0)]
        ms: u64,
        #[command(flatten)]
        wave: WaveArgs,
    },
}

#[derive(Debug, Clone, Args, Default)]
struct WaveArgs {
    #[arg(long)]
    wave_html: Option<PathBuf>,
    #[arg(long)]
    wave_json: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    wave_start_ns: u64,
    #[arg(long)]
    wave_end_ns: Option<u64>,
}

impl From<WaveArgs> for WaveCaptureOptions {
    fn from(value: WaveArgs) -> Self {
        Self {
            html_path: value.wave_html,
            json_path: value.wave_json,
            start_ns: value.wave_start_ns,
            end_ns: value.wave_end_ns,
        }
    }
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
            wave,
        } => {
            let sim = Simulator::from_hex_path_with_options(&hex, trace_cpu, wave.into())
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
        Command::Repl {
            hex,
            trace_cpu,
            wave,
        } => {
            let sim = Simulator::from_hex_path_with_options(&hex, trace_cpu, wave.into())
                .with_context(|| format!("加载 HEX 失败: {}", hex.display()))?;
            crate::script::run_repl(sim).context("进入交互式 Rhai 失败")?;
        }
        Command::Dump { hex, ms, wave } => {
            let mut sim = Simulator::from_hex_path_with_options(&hex, false, wave.into())
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
