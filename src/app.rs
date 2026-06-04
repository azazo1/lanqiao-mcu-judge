use std::{path::PathBuf, str::FromStr};

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
        hex: Option<PathBuf>,
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
        hex: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        trace_cpu: bool,
        #[command(flatten)]
        wave: WaveArgs,
    },
    Dump {
        #[arg(long)]
        hex: Option<PathBuf>,
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
    #[arg(long)]
    wave_msgpack: Option<PathBuf>,
    #[arg(
        long = "wave-start",
        value_name = "TIME",
        default_value = "0",
        help = "波形起始时间, 支持 ns/us/ms/s 后缀"
    )]
    wave_start: TimeNsArg,
    #[arg(
        long = "wave-end",
        value_name = "TIME",
        help = "波形结束时间, 支持 ns/us/ms/s 后缀"
    )]
    wave_end: Option<TimeNsArg>,
}

impl From<WaveArgs> for WaveCaptureOptions {
    fn from(value: WaveArgs) -> Self {
        Self {
            html_path: value.wave_html,
            json_path: value.wave_json,
            msgpack_path: value.wave_msgpack,
            start_ns: value.wave_start.into_ns(),
            end_ns: value.wave_end.map(TimeNsArg::into_ns),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TimeNsArg(u64);

impl TimeNsArg {
    fn into_ns(self) -> u64 {
        self.0
    }
}

impl FromStr for TimeNsArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        parse_time_ns_arg(value).map(Self)
    }
}

fn parse_time_ns_arg(value: &str) -> std::result::Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("时间参数不能为空".into());
    }
    let lowered = trimmed.to_ascii_lowercase();
    for (suffix, scale) in [
        ("ns", 1_u64),
        ("us", 1_000_u64),
        ("ms", 1_000_000_u64),
        ("s", 1_000_000_000_u64),
    ] {
        if let Some(number_part) = lowered.strip_suffix(suffix) {
            return parse_time_number(number_part, scale, trimmed);
        }
    }
    parse_time_number(&lowered, 1, trimmed)
}

fn parse_time_number(
    number_part: &str,
    scale_ns: u64,
    original: &str,
) -> std::result::Result<u64, String> {
    let normalized = number_part.replace('_', "");
    if normalized.is_empty() {
        return Err(format!("时间参数缺少数值: {original}"));
    }
    if normalized.matches('.').count() > 1 {
        return Err(format!("时间参数格式错误: {original}"));
    }
    if let Some((integer_part, fraction_part)) = normalized.split_once('.') {
        if fraction_part.is_empty() {
            return Err(format!("时间参数小数点后不能为空: {original}"));
        }
        if !integer_part.is_empty() && !integer_part.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(format!("时间参数格式错误: {original}"));
        }
        if !fraction_part.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(format!("时间参数格式错误: {original}"));
        }
        let integer = if integer_part.is_empty() {
            0_u128
        } else {
            integer_part
                .parse::<u128>()
                .map_err(|_| format!("时间参数数值过大: {original}"))?
        };
        let fraction = fraction_part
            .parse::<u128>()
            .map_err(|_| format!("时间参数数值过大: {original}"))?;
        let denominator = 10_u128
            .checked_pow(fraction_part.len() as u32)
            .ok_or_else(|| format!("时间参数小数位过多: {original}"))?;
        let scale = u128::from(scale_ns);
        let integer_ns = integer
            .checked_mul(scale)
            .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
        let fraction_scaled = fraction
            .checked_mul(scale)
            .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
        if fraction_scaled % denominator != 0 {
            return Err(format!("时间参数精度不能小于 1ns: {original}"));
        }
        let total = integer_ns
            .checked_add(fraction_scaled / denominator)
            .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
        return u64::try_from(total).map_err(|_| format!("时间参数数值过大: {original}"));
    }
    if !normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("时间参数格式错误: {original}"));
    }
    let value = normalized
        .parse::<u128>()
        .map_err(|_| format!("时间参数数值过大: {original}"))?;
    let total = value
        .checked_mul(u128::from(scale_ns))
        .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
    u64::try_from(total).map_err(|_| format!("时间参数数值过大: {original}"))
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
            let sim = load_simulator(hex.as_ref(), trace_cpu, wave.into())?;
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
            let sim = load_simulator(hex.as_ref(), trace_cpu, wave.into())?;
            crate::script::run_repl(sim).context("进入交互式 Rhai 失败")?;
        }
        Command::Dump { hex, ms, wave } => {
            let mut sim = load_simulator(hex.as_ref(), false, wave.into())?;
            sim.run_ms(ms)?;
            println!("{}", sim.snapshot_text());
        }
    }
    Ok(())
}

fn load_simulator(
    hex: Option<&PathBuf>,
    trace_cpu: bool,
    wave_options: WaveCaptureOptions,
) -> Result<Simulator> {
    match hex {
        Some(path) => Simulator::from_hex_path_with_options(path, trace_cpu, wave_options)
            .with_context(|| format!("加载 HEX 失败: {}", path.display())),
        None => Ok(Simulator::nop_with_options(trace_cpu, wave_options)),
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, WaveCaptureOptions, parse_time_ns_arg};
    use clap::Parser;

    fn dump_wave_options(args: &[&str]) -> WaveCaptureOptions {
        let cli = Cli::try_parse_from(args).expect("parse cli");
        match cli.command {
            Command::Dump { wave, .. } => wave.into(),
            _ => panic!("expected dump command"),
        }
    }

    #[test]
    fn parse_time_ns_arg_supports_common_units() {
        assert_eq!(parse_time_ns_arg("1ns").unwrap(), 1);
        assert_eq!(parse_time_ns_arg("1us").unwrap(), 1_000);
        assert_eq!(parse_time_ns_arg("1ms").unwrap(), 1_000_000);
        assert_eq!(parse_time_ns_arg("1s").unwrap(), 1_000_000_000);
        assert_eq!(parse_time_ns_arg("42").unwrap(), 42);
    }

    #[test]
    fn parse_time_ns_arg_supports_fractional_values() {
        assert_eq!(parse_time_ns_arg("1.5us").unwrap(), 1_500);
        assert_eq!(parse_time_ns_arg("0.25ms").unwrap(), 250_000);
        assert_eq!(parse_time_ns_arg(".5s").unwrap(), 500_000_000);
    }

    #[test]
    fn parse_time_ns_arg_rejects_sub_nanosecond_precision() {
        let err = parse_time_ns_arg("0.5ns").unwrap_err();
        assert!(err.contains("精度"));
    }

    #[test]
    fn cli_accepts_wave_time_flags_without_ns_suffix() {
        let options = dump_wave_options(&[
            "stcjudge",
            "dump",
            "--hex",
            "sample.hex",
            "--wave-start",
            "1ms",
            "--wave-end",
            "2ms",
        ]);
        assert_eq!(options.start_ns, 1_000_000);
        assert_eq!(options.end_ns, Some(2_000_000));
    }

    #[test]
    fn cli_accepts_wave_msgpack_flag() {
        let options = dump_wave_options(&[
            "stcjudge",
            "dump",
            "--hex",
            "sample.hex",
            "--wave-msgpack",
            "/tmp/out.msgpack",
        ]);
        assert_eq!(
            options.msgpack_path.as_deref(),
            Some(std::path::Path::new("/tmp/out.msgpack"))
        );
    }

    #[test]
    fn cli_allows_dump_without_hex() {
        let cli = Cli::try_parse_from(["stcjudge", "dump", "--ms", "5"]).expect("parse cli");
        match cli.command {
            Command::Dump { hex, ms, .. } => {
                assert_eq!(hex, None);
                assert_eq!(ms, 5);
            }
            _ => panic!("expected dump command"),
        }
    }
}
