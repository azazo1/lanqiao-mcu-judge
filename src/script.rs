pub(crate) mod run_target;
mod state_api;
pub(crate) mod state_target;

use std::{
    io::{self, BufRead, IsTerminal, Read, Write},
    ops::{Range, RangeInclusive},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use rhai::{
    Array, Dynamic, Engine, EvalAltResult, FnPtr, ImmutableString, Map, NativeCallContext,
    Position, Scope,
    debugger::{DebuggerCommand, DebuggerEvent},
};
use tracing::{debug, info, trace};

use crate::{
    chip::{
        DisplayNumber, LedWatchStats, NS_PER_MICROSECOND, NS_PER_MILLISECOND, NS_PER_SECOND,
        Simulator, UartConfig, UartParity, UartStopBits,
    },
    ids::{KeyId, KeyMode, LedId, ResetMode, SignalId, VoltageChannel},
    peripherals::Ds1302State,
    script::run_target::{RunToEdge, RunToTarget},
};

pub fn run_script(sim: Simulator, path: &Path) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("读取脚本失败: {}", path.display()))?;
    run_script_source(sim, &format!("file:{}", path.display()), &source)
}

pub fn run_script_stdin(sim: Simulator) -> Result<()> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("读取标准输入脚本失败")?;
    if source.trim().is_empty() {
        bail!("标准输入中没有 Rhai 脚本内容");
    }
    run_script_source(sim, "stdin", &source)
}

pub fn run_script_source(sim: Simulator, label: &str, source: &str) -> Result<()> {
    eval_source(sim, label, source)
}

pub fn run_repl(sim: Simulator) -> Result<()> {
    let shared = Arc::new(Mutex::new(sim));
    let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
    let checkpoint_state = Arc::new(Mutex::new(ScriptCheckpointState::default()));
    let engine = build_engine(&shared, &trace_state, &checkpoint_state);
    let mut scope = build_scope();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let interactive = io::stdin().is_terminal();
    let mut line = String::new();
    let mut line_no = 1_u64;

    loop {
        line.clear();
        if interactive {
            print!("judge> ");
            io::stdout().flush().context("刷新 REPL 提示符失败")?;
        }

        if reader.read_line(&mut line).context("读取 REPL 输入失败")? == 0 {
            break;
        }

        let statement = line.trim();
        if statement.is_empty() {
            continue;
        }
        if matches!(statement, ":quit" | ":exit") {
            break;
        }
        if statement == ":help" {
            println!("内置命令: :help, :quit, :exit");
            continue;
        }

        debug!(
            line_no,
            sim_time_ns = current_sim_time_ns(&shared),
            statement,
            "执行 REPL 语句"
        );
        if let Err(err) = eval_source_with_engine(
            &engine,
            &mut scope,
            &trace_state,
            &format!("repl:{line_no}"),
            &line,
        ) {
            eprintln!("{err}");
        }
        line_no = line_no.saturating_add(1);
    }

    Ok(())
}

fn eval_source(sim: Simulator, label: &str, source: &str) -> Result<()> {
    let shared = Arc::new(Mutex::new(sim));
    let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
    let checkpoint_state = Arc::new(Mutex::new(ScriptCheckpointState::default()));
    let engine = build_engine(&shared, &trace_state, &checkpoint_state);
    let mut scope = build_scope();
    debug!(
        label,
        lines = source.lines().count(),
        sim_time_ns = current_sim_time_ns(&shared),
        "开始执行评测脚本"
    );
    let result = eval_source_with_engine(&engine, &mut scope, &trace_state, label, source);
    finalize_checkpoint_run(&checkpoint_state, result)
}

fn eval_source_with_engine(
    engine: &Engine,
    scope: &mut Scope<'_>,
    trace_state: &Arc<Mutex<ScriptTraceState>>,
    label: &str,
    source: &str,
) -> Result<()> {
    update_script_trace_state(trace_state, label, source);
    let mut ast = engine
        .compile_with_scope(scope, source)
        .map_err(|err| anyhow!(err.to_string()))?;
    ast.set_source(label);
    let _ = engine
        .eval_ast_with_scope::<Dynamic>(scope, &ast)
        .map_err(|err| anyhow!(err.to_string()))?;
    Ok(())
}

fn build_engine(
    sim: &Arc<Mutex<Simulator>>,
    trace_state: &Arc<Mutex<ScriptTraceState>>,
    checkpoint_state: &Arc<Mutex<ScriptCheckpointState>>,
) -> Engine {
    let mut engine = Engine::new();
    engine.on_print(|text| println!("{text}"));
    register_script_progress_debugger(&mut engine, sim, trace_state);
    engine.register_type_with_name::<LedId>("Led");
    engine.register_type_with_name::<KeyId>("Key");
    engine.register_type_with_name::<KeyMode>("KeyMode");
    engine.register_type_with_name::<ResetMode>("ResetMode");
    engine.register_type_with_name::<VoltageChannel>("VoltageChannel");
    engine.register_type_with_name::<SignalId>("Signal");
    engine.register_type_with_name::<RunToTarget>("RunToTarget");
    engine.register_type_with_name::<RunToEdge>("RunToEdge");
    register_api(&mut engine, sim, checkpoint_state);
    engine
}

#[derive(Debug, Default)]
struct ScriptTraceState {
    label: String,
    lines: Vec<String>,
    step: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckpointStatus {
    Passed,
    Failed,
}

impl CheckpointStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "✅ 通过",
            Self::Failed => "❌ 失败",
        }
    }
}

#[derive(Debug, Clone)]
struct CheckpointRecord {
    index: i64,
    condition: String,
    expected: String,
    actual: String,
    actual_detail: Option<String>,
    status: CheckpointStatus,
}

#[derive(Debug, Default)]
struct ScriptCheckpointState {
    records: Vec<CheckpointRecord>,
}

#[derive(Debug, Clone)]
struct CheckpointSummary {
    total: usize,
    failed: usize,
    report: String,
}

impl CheckpointSummary {
    fn failure_message(&self) -> String {
        format!("ckpt 失败: {}/{}", self.failed, self.total)
    }
}

impl ScriptCheckpointState {
    fn record(
        &mut self,
        index: i64,
        condition: &str,
        expected: &str,
        actual: String,
        actual_detail: Option<String>,
        status: CheckpointStatus,
    ) {
        self.records.push(CheckpointRecord {
            index,
            condition: condition.to_owned(),
            expected: expected.to_owned(),
            actual,
            actual_detail,
            status,
        });
    }

    fn summary(&self) -> Option<CheckpointSummary> {
        if self.records.is_empty() {
            return None;
        }

        let total = self.records.len();
        let failed = self
            .records
            .iter()
            .filter(|record| record.status == CheckpointStatus::Failed)
            .count();
        let passed = total.saturating_sub(failed);

        let rows = self
            .records
            .iter()
            .map(|record| {
                vec![
                    record.index.to_string(),
                    checkpoint_table_cell(&record.condition),
                    checkpoint_table_cell(&record.expected),
                    checkpoint_table_cell(&record.actual),
                    record.status.as_str().to_owned(),
                ]
            })
            .collect::<Vec<_>>();

        let mut report = format_checkpoint_report(passed, failed, &rows);
        report.push_str(&format_checkpoint_failure_records(&self.records));

        Some(CheckpointSummary {
            total,
            failed,
            report,
        })
    }
}

fn finalize_checkpoint_run(
    checkpoint_state: &Arc<Mutex<ScriptCheckpointState>>,
    result: Result<()>,
) -> Result<()> {
    let summary = checkpoint_state
        .lock()
        .expect("checkpoint state lock")
        .summary();

    if let Some(summary) = &summary {
        print!("{}", summary.report);
    }

    match result {
        Ok(()) => {
            if let Some(summary) = summary
                && summary.failed > 0
            {
                bail!("{}", summary.failure_message());
            }
            Ok(())
        }
        Err(err) => {
            if let Some(summary) = summary
                && summary.failed > 0
            {
                return Err(err.context(summary.failure_message()));
            }
            Err(err)
        }
    }
}

fn checkpoint_table_cell(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let text = text.replace('\n', " / ");
    if text.trim().is_empty() {
        "-".to_owned()
    } else {
        text
    }
}

fn format_checkpoint_report(passed: usize, failed: usize, rows: &[Vec<String>]) -> String {
    let headers = ["序号", "测试条件", "期望结果", "实际结果", "状态"];
    let mut report = String::new();
    report.push_str(&format!("评测统计: 通过 {passed} 项, 失败 {failed} 项\n\n"));
    report.push_str(&checkpoint_table_row(&headers));
    report.push('\n');
    report.push_str(&checkpoint_table_separator(headers.len()));
    report.push('\n');
    for row in rows {
        let cells = row.iter().map(String::as_str).collect::<Vec<_>>();
        report.push_str(&checkpoint_table_row(&cells));
        report.push('\n');
    }
    report
}

fn format_checkpoint_failure_records(records: &[CheckpointRecord]) -> String {
    let mut report = String::new();
    let failed_records = records
        .iter()
        .filter(|record| record.status == CheckpointStatus::Failed)
        .collect::<Vec<_>>();
    if failed_records.is_empty() {
        return report;
    }

    report.push('\n');
    report.push_str("失败详情:\n");
    for record in failed_records {
        report.push_str(&format!("\n[{}] {}\n", record.index, record.condition));
        let detail = record
            .actual_detail
            .as_deref()
            .unwrap_or(record.actual.as_str());
        report.push_str(&indent_checkpoint_detail(detail, "  "));
        report.push('\n');
    }
    report
}

fn checkpoint_table_separator(columns: usize) -> String {
    let mut border = String::new();
    border.push('|');
    for _ in 0..columns {
        border.push_str(" --- |");
    }
    border
}

fn checkpoint_table_row(cells: &[impl AsRef<str>]) -> String {
    let mut row = String::new();
    row.push('|');
    for cell in cells {
        row.push(' ');
        row.push_str(cell.as_ref());
        row.push(' ');
        row.push('|');
    }
    row
}

fn update_script_trace_state(
    trace_state: &Arc<Mutex<ScriptTraceState>>,
    label: &str,
    source: &str,
) {
    let mut state = trace_state.lock().expect("script trace state lock");
    state.label.clear();
    state.label.push_str(label);
    state.lines = source.lines().map(str::to_owned).collect();
    state.step = 0;
}

fn source_line_snippet(lines: &[String], pos: Position) -> String {
    let Some(line_no) = pos.line() else {
        return String::new();
    };
    lines
        .get(line_no.saturating_sub(1))
        .map(|line| line.trim().to_owned())
        .unwrap_or_default()
}

fn script_event_name(event: DebuggerEvent<'_>) -> &'static str {
    match event {
        DebuggerEvent::Start => "start",
        DebuggerEvent::Step => "step",
        DebuggerEvent::BreakPoint(_) => "breakpoint",
        DebuggerEvent::FunctionExitWithValue(_) => "fn_return",
        DebuggerEvent::FunctionExitWithError(_) => "fn_error",
        DebuggerEvent::End => "end",
        _ => "other",
    }
}

fn current_sim_time_ns(sim: &Arc<Mutex<Simulator>>) -> u64 {
    sim.lock().expect("sim lock").sim_time_ns()
}

fn register_script_progress_debugger(
    engine: &mut Engine,
    sim: &Arc<Mutex<Simulator>>,
    trace_state: &Arc<Mutex<ScriptTraceState>>,
) {
    let sim = Arc::clone(sim);
    let trace_state = Arc::clone(trace_state);
    #[allow(deprecated)]
    engine.register_debugger(
        |_, debugger| debugger,
        move |context, event, node, source, pos| {
            let mut label = source.unwrap_or("").to_owned();
            let mut step = 0_u64;
            let mut snippet = String::new();

            if let Ok(mut state) = trace_state.lock() {
                if label.is_empty() {
                    label = state.label.clone();
                }
                if matches!(event, DebuggerEvent::End) {
                    step = state.step;
                } else {
                    state.step = state.step.saturating_add(1);
                    step = state.step;
                }
                snippet = source_line_snippet(&state.lines, pos);
            }
            let sim_time_ns = current_sim_time_ns(&sim);

            match event {
                DebuggerEvent::Start | DebuggerEvent::Step | DebuggerEvent::BreakPoint(_) => {
                    debug!(
                        target: "script_progress",
                        snippet,
                        sim_time_ns,
                        line = pos.line().unwrap_or(0),
                        event = script_event_name(event),
                        step,
                        column = pos.position().unwrap_or(0),
                        call_level = context.call_level(),
                        is_stmt = node.is_stmt(),
                        label,
                        "执行评测脚本语句"
                    );
                    Ok(DebuggerCommand::Next)
                }
                DebuggerEvent::FunctionExitWithValue(_)
                | DebuggerEvent::FunctionExitWithError(_) => {
                    trace!(
                        target: "script_progress",
                        label,
                        event = script_event_name(event),
                        sim_time_ns,
                        line = pos.line().unwrap_or(0),
                        column = pos.position().unwrap_or(0),
                        call_level = context.call_level(),
                        "脚本函数返回"
                    );
                    Ok(DebuggerCommand::Next)
                }
                DebuggerEvent::End => {
                    debug!(
                        target: "script_progress",
                        label,
                        event = script_event_name(event),
                        sim_time_ns,
                        steps = step,
                        "评测脚本执行结束"
                    );
                    Ok(DebuggerCommand::Continue)
                }
                _ => Ok(DebuggerCommand::Next),
            }
        },
    );
}

fn build_scope() -> Scope<'static> {
    let mut scope = Scope::new();
    for (name, led) in [
        ("L1", LedId::L1),
        ("L2", LedId::L2),
        ("L3", LedId::L3),
        ("L4", LedId::L4),
        ("L5", LedId::L5),
        ("L6", LedId::L6),
        ("L7", LedId::L7),
        ("L8", LedId::L8),
    ] {
        scope.push_constant(name, led);
    }
    for (name, key) in [
        ("S4", KeyId::S4),
        ("S5", KeyId::S5),
        ("S6", KeyId::S6),
        ("S7", KeyId::S7),
        ("S8", KeyId::S8),
        ("S9", KeyId::S9),
        ("S10", KeyId::S10),
        ("S11", KeyId::S11),
        ("S12", KeyId::S12),
        ("S13", KeyId::S13),
        ("S14", KeyId::S14),
        ("S15", KeyId::S15),
        ("S16", KeyId::S16),
        ("S17", KeyId::S17),
        ("S18", KeyId::S18),
        ("S19", KeyId::S19),
    ] {
        scope.push_constant(name, key);
    }
    for (name, channel) in [
        ("RB2", VoltageChannel::Rb2),
        ("RD1", VoltageChannel::Rd1),
        ("AIN1", VoltageChannel::Rd1),
        ("AIN3", VoltageChannel::Rb2),
    ] {
        scope.push_constant(name, channel);
    }
    scope.push_constant("KEYBOARD", KeyMode::Keyboard);
    scope.push_constant("KBD", KeyMode::Keyboard);
    scope.push_constant("BUTTON", KeyMode::Button);
    scope.push_constant("BTN", KeyMode::Button);
    scope.push_constant("CPU_RESET", ResetMode::Cpu);
    scope.push_constant("RESET_CPU", ResetMode::Cpu);
    scope.push_constant("POWER_RESET", ResetMode::Power);
    scope.push_constant("RESET_POWER", ResetMode::Power);
    scope.push_constant("SIG_OUT", SignalId::SigOut);
    scope.push_constant("NET_SIG", SignalId::NetSig);
    scope.push_constant("UP", RunToEdge::Up);
    scope.push_constant("DOWN", RunToEdge::Down);
    scope.push_constant("FLIP", RunToEdge::Flip);
    for digit in 1_u8..=8 {
        scope.push_constant(format!("D{digit}"), RunToTarget::SegDigit(digit));
        scope.push_constant(format!("SEG{digit}"), RunToTarget::SegDigit(digit));
    }
    for port in 0_usize..=5 {
        for bit in 0_u8..=7 {
            scope.push_constant(format!("P{port}{bit}"), RunToTarget::Pin { port, bit });
        }
    }
    for (name, target) in [
        ("I2C_SCL", RunToTarget::I2cBusScl),
        ("I2C_SDA", RunToTarget::I2cBusSda),
        ("IIC_SCL", RunToTarget::I2cBusScl),
        ("IIC_SDA", RunToTarget::I2cBusSda),
        ("I2C_BUS_SCL", RunToTarget::I2cBusScl),
        ("I2C_BUS_SDA", RunToTarget::I2cBusSda),
        ("IIC_BUS_SCL", RunToTarget::I2cBusScl),
        ("IIC_BUS_SDA", RunToTarget::I2cBusSda),
        ("I2C_MASTER_SCL", RunToTarget::I2cMasterScl),
        ("I2C_MASTER_SDA", RunToTarget::I2cMasterSda),
        ("IIC_MASTER_SCL", RunToTarget::I2cMasterScl),
        ("IIC_MASTER_SDA", RunToTarget::I2cMasterSda),
        ("I2C_SLAVE_SCL_LOW", RunToTarget::I2cSlaveSclLow),
        ("I2C_SLAVE_SDA_LOW", RunToTarget::I2cSlaveSdaLow),
        ("IIC_SLAVE_SCL_LOW", RunToTarget::I2cSlaveSclLow),
        ("IIC_SLAVE_SDA_LOW", RunToTarget::I2cSlaveSdaLow),
        ("ONEWIRE_MASTER", RunToTarget::OnewireMasterHigh),
        ("ONEWIRE_BUS", RunToTarget::OnewireBusHigh),
        ("ONEWIRE_DEVICE_LOW", RunToTarget::OnewireDeviceLow),
        ("UART1_TX", RunToTarget::Uart1Tx),
        ("UART1_RX", RunToTarget::Uart1Rx),
        ("UART2_TX", RunToTarget::Uart2Tx),
        ("UART2_RX", RunToTarget::Uart2Rx),
        ("DS1302_CE", RunToTarget::Ds1302Ce),
        ("DS1302_CLK", RunToTarget::Ds1302Clk),
        ("DS1302_IO", RunToTarget::Ds1302Io),
        ("NE555_SIG_OUT", RunToTarget::Ne555SigOut),
    ] {
        scope.push_constant(name, target);
    }
    scope
}

fn led_stats_map(stats: LedWatchStats) -> Result<Map, Box<EvalAltResult>> {
    let mut map = Map::new();
    let changes =
        i64::try_from(stats.changes).map_err(|_| runtime_error("LED 变化次数超出脚本整数范围"))?;
    let rising_edges = i64::try_from(stats.rising_edges)
        .map_err(|_| runtime_error("LED 上升沿次数超出脚本整数范围"))?;
    map.insert("changes".into(), changes.into());
    map.insert(
        "change_frequency_hz".into(),
        stats
            .change_frequency_hz()
            .map_err(|err| runtime_error(err.to_string()))?
            .into(),
    );
    map.insert("rising_edges".into(), rising_edges.into());
    map.insert(
        "pwm_frequency_hz".into(),
        stats
            .pwm_frequency_hz()
            .map_err(|err| runtime_error(err.to_string()))?
            .into(),
    );
    map.insert(
        "duty_percent".into(),
        stats
            .duty_percent()
            .map_err(|err| runtime_error(err.to_string()))?
            .into(),
    );
    Ok(map)
}

fn register_api(
    engine: &mut Engine,
    sim: &Arc<Mutex<Simulator>>,
    checkpoint_state: &Arc<Mutex<ScriptCheckpointState>>,
) {
    let sim_run = Arc::clone(sim);
    engine.register_fn("run_ms", move |ms: i64| -> Result<(), Box<EvalAltResult>> {
        let ms = u64::try_from(ms).map_err(|_| runtime_error("run_ms 参数必须 >= 0"))?;
        sim_run
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .run_ms(ms)
            .map_err(|err| runtime_error(err.to_string()))
    });

    let sim_run_us = Arc::clone(sim);
    engine.register_fn("run_us", move |us: i64| -> Result<(), Box<EvalAltResult>> {
        let us = u64::try_from(us).map_err(|_| runtime_error("run_us 参数必须 >= 0"))?;
        sim_run_us
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .run_us(us)
            .map_err(|err| runtime_error(err.to_string()))
    });

    let sim_run_to_ns = Arc::clone(sim);
    engine.register_fn(
        "run_to_ns",
        move |target_ns: i64| -> Result<i64, Box<EvalAltResult>> {
            let target_ns =
                u64::try_from(target_ns).map_err(|_| runtime_error("run_to_ns 参数必须 >= 0"))?;
            let elapsed_ns = sim_run_to_ns
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "run_to_ns 返回值超出脚本整数范围")
        },
    );

    let sim_run_to_us = Arc::clone(sim);
    engine.register_fn(
        "run_to_us",
        move |target_us: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_us =
                u64::try_from(target_us).map_err(|_| runtime_error("run_to_us 参数必须 >= 0"))?;
            let target_ns = target_us.saturating_mul(NS_PER_MICROSECOND);
            let elapsed_ns = sim_run_to_us
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_MICROSECOND as rhai::FLOAT)
        },
    );

    let sim_run_to_us_float = Arc::clone(sim);
    engine.register_fn(
        "run_to_us",
        move |target_us: rhai::FLOAT| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_ns = script_time_target_ns(target_us, NS_PER_MICROSECOND, "run_to_us")?;
            let elapsed_ns = sim_run_to_us_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_MICROSECOND as rhai::FLOAT)
        },
    );

    let sim_run_to_ms = Arc::clone(sim);
    engine.register_fn(
        "run_to_ms",
        move |target_ms: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_ms =
                u64::try_from(target_ms).map_err(|_| runtime_error("run_to_ms 参数必须 >= 0"))?;
            let target_ns = target_ms.saturating_mul(NS_PER_MILLISECOND);
            let elapsed_ns = sim_run_to_ms
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_MILLISECOND as rhai::FLOAT)
        },
    );

    let sim_run_to_ms_float = Arc::clone(sim);
    engine.register_fn(
        "run_to_ms",
        move |target_ms: rhai::FLOAT| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_ns = script_time_target_ns(target_ms, NS_PER_MILLISECOND, "run_to_ms")?;
            let elapsed_ns = sim_run_to_ms_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_MILLISECOND as rhai::FLOAT)
        },
    );

    let sim_run_to_s = Arc::clone(sim);
    engine.register_fn(
        "run_to_s",
        move |target_s: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_s =
                u64::try_from(target_s).map_err(|_| runtime_error("run_to_s 参数必须 >= 0"))?;
            let target_ns = target_s.saturating_mul(NS_PER_SECOND);
            let elapsed_ns = sim_run_to_s
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_SECOND as rhai::FLOAT)
        },
    );

    let sim_run_to_s_float = Arc::clone(sim);
    engine.register_fn(
        "run_to_s",
        move |target_s: rhai::FLOAT| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let target_ns = script_time_target_ns(target_s, NS_PER_SECOND, "run_to_s")?;
            let elapsed_ns = sim_run_to_s_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .run_to_ns(target_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(elapsed_ns as rhai::FLOAT / NS_PER_SECOND as rhai::FLOAT)
        },
    );

    let sim_time_now = Arc::clone(sim);
    engine.register_fn("sim_time_ns", move || -> Result<i64, Box<EvalAltResult>> {
        let now_ns = sim_time_now
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .sim_time_ns();
        script_int(now_ns, "sim_time_ns 返回值超出脚本整数范围")
    });

    let sim_add_marker_now = Arc::clone(sim);
    engine.register_fn("add_marker", move || -> Result<(), Box<EvalAltResult>> {
        sim_add_marker_now
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .add_wave_marker(None);
        Ok(())
    });

    let sim_add_marker_label = Arc::clone(sim);
    engine.register_fn(
        "add_marker",
        move |label: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            let label = label.trim();
            sim_add_marker_label
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .add_wave_marker((!label.is_empty()).then_some(label));
            Ok(())
        },
    );

    let sim_add_marker_at = Arc::clone(sim);
    engine.register_fn(
        "add_marker",
        move |time_ns: i64| -> Result<(), Box<EvalAltResult>> {
            let time_ns =
                u64::try_from(time_ns).map_err(|_| runtime_error("add_marker 时间戳必须 >= 0"))?;
            sim_add_marker_at
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .add_wave_marker_at(time_ns, None);
            Ok(())
        },
    );

    let sim_add_marker_at_label = Arc::clone(sim);
    engine.register_fn(
        "add_marker",
        move |time_ns: i64, label: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            let time_ns =
                u64::try_from(time_ns).map_err(|_| runtime_error("add_marker 时间戳必须 >= 0"))?;
            let label = label.trim();
            sim_add_marker_at_label
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .add_wave_marker_at(time_ns, (!label.is_empty()).then_some(label));
            Ok(())
        },
    );

    register_run_to_api(engine, sim);
    state_api::register_state_api(engine, sim);

    let sim_export_persistent = Arc::clone(sim);
    engine.register_fn(
        "export_persistent_state",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_export_persistent
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .export_persistent_state();
            Ok(text.into())
        },
    );

    let sim_load_persistent = Arc::clone(sim);
    engine.register_fn(
        "load_persistent_state",
        move |text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_load_persistent
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .load_persistent_state(text.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_reset = Arc::clone(sim);
    engine.register_fn("reset", move || -> Result<(), Box<EvalAltResult>> {
        sim_reset
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .reset()
            .map_err(|err| runtime_error(err.to_string()))
    });

    let sim_reset_mode = Arc::clone(sim);
    engine.register_fn(
        "reset",
        move |mode: ResetMode| -> Result<(), Box<EvalAltResult>> {
            sim_reset_mode
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .reset_with_mode(mode)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_key = Arc::clone(sim);
    engine.register_fn(
        "set_key",
        move |name: ImmutableString, pressed: bool| -> Result<(), Box<EvalAltResult>> {
            sim_key
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_key(name.as_str(), pressed)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_key_id = Arc::clone(sim);
    engine.register_fn(
        "set_key",
        move |key: KeyId, pressed: bool| -> Result<(), Box<EvalAltResult>> {
            sim_key_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_key_id(key, pressed);
            Ok(())
        },
    );

    let sim_key_tap = Arc::clone(sim);
    engine.register_fn(
        "tap_key",
        move |name: ImmutableString, hold_ms: i64| -> Result<(), Box<EvalAltResult>> {
            let hold_ms =
                u64::try_from(hold_ms).map_err(|_| runtime_error("hold_ms 参数必须 >= 0"))?;
            let mut sim = sim_key_tap
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            sim.tap_key(name.as_str(), hold_ms)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_key_tap_id = Arc::clone(sim);
    engine.register_fn(
        "tap_key",
        move |key: KeyId, hold_ms: i64| -> Result<(), Box<EvalAltResult>> {
            let hold_ms =
                u64::try_from(hold_ms).map_err(|_| runtime_error("hold_ms 参数必须 >= 0"))?;
            let mut sim = sim_key_tap_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            sim.tap_key_id(key, hold_ms)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_key_mode = Arc::clone(sim);
    engine.register_fn(
        "key_mode",
        move |mode: KeyMode| -> Result<(), Box<EvalAltResult>> {
            sim_key_mode
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .key_mode(mode);
            Ok(())
        },
    );

    let sim_key_mode_name = Arc::clone(sim);
    engine.register_fn(
        "key_mode",
        move |mode: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            let mode =
                KeyMode::parse(mode.as_str()).map_err(|err| runtime_error(err.to_string()))?;
            sim_key_mode_name
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .key_mode(mode);
            Ok(())
        },
    );

    let sim_rtc = Arc::clone(sim);
    engine.register_fn(
        "set_rtc",
        move |hour: i64, minute: i64, second: i64| -> Result<(), Box<EvalAltResult>> {
            let (hour, minute, second) = (
                u8::try_from(hour).map_err(|_| runtime_error("hour 越界"))?,
                u8::try_from(minute).map_err(|_| runtime_error("minute 越界"))?,
                u8::try_from(second).map_err(|_| runtime_error("second 越界"))?,
            );
            sim_rtc
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_rtc(hour, minute, second)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_rtc_patch = Arc::clone(sim);
    engine.register_fn(
        "set_rtc",
        move |state: Map| -> Result<(), Box<EvalAltResult>> {
            let state = script_rtc_state(state)?;
            sim_rtc_patch
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_rtc_state(state)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_temp = Arc::clone(sim);
    engine.register_fn(
        "set_temperature_c",
        move |temp: i64| -> Result<(), Box<EvalAltResult>> {
            sim_temp
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_temperature_c(temp as f32);
            Ok(())
        },
    );

    let sim_temp_float = Arc::clone(sim);
    engine.register_fn(
        "set_temperature_c",
        move |temp: rhai::FLOAT| -> Result<(), Box<EvalAltResult>> {
            sim_temp_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_temperature_c(temp as f32);
            Ok(())
        },
    );

    let sim_ds18b20_rom = Arc::clone(sim);
    engine.register_fn(
        "set_ds18b20_rom",
        move |rom: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_ds18b20_rom
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_ds18b20_rom(rom.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_ds18b20_parasite = Arc::clone(sim);
    engine.register_fn(
        "set_ds18b20_parasite_power",
        move |enabled: bool| -> Result<(), Box<EvalAltResult>> {
            sim_ds18b20_parasite
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_ds18b20_parasite_power(enabled);
            Ok(())
        },
    );

    let sim_distance = Arc::clone(sim);
    engine.register_fn(
        "set_distance_cm",
        move |distance: i64| -> Result<(), Box<EvalAltResult>> {
            sim_distance
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_distance_cm(distance as f32);
            Ok(())
        },
    );

    let sim_frequency = Arc::clone(sim);
    engine.register_fn(
        "set_frequency_hz",
        move |hz: i64| -> Result<(), Box<EvalAltResult>> {
            sim_frequency
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_frequency_hz(hz as f32);
            Ok(())
        },
    );

    let sim_jumper_on = Arc::clone(sim);
    engine.register_fn(
        "jumper_on",
        move |left: SignalId, right: SignalId| -> Result<(), Box<EvalAltResult>> {
            sim_jumper_on
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_on(left, right)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_jumper_on_name = Arc::clone(sim);
    engine.register_fn(
        "jumper_on",
        move |left: ImmutableString, right: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_jumper_on_name
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_on_named(left.as_str(), right.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_jumper_off = Arc::clone(sim);
    engine.register_fn(
        "jumper_off",
        move |left: SignalId, right: SignalId| -> Result<(), Box<EvalAltResult>> {
            sim_jumper_off
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_off(left, right)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_jumper_off_name = Arc::clone(sim);
    engine.register_fn(
        "jumper_off",
        move |left: ImmutableString, right: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_jumper_off_name
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_off_named(left.as_str(), right.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_jumper_installed = Arc::clone(sim);
    engine.register_fn(
        "jumper_installed",
        move |left: SignalId, right: SignalId| -> Result<bool, Box<EvalAltResult>> {
            let installed = sim_jumper_installed
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_installed(left, right);
            Ok(installed)
        },
    );

    let sim_jumper_installed_name = Arc::clone(sim);
    engine.register_fn(
        "jumper_installed",
        move |left: ImmutableString, right: ImmutableString| -> Result<bool, Box<EvalAltResult>> {
            sim_jumper_installed_name
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .jumper_installed_named(left.as_str(), right.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_voltage = Arc::clone(sim);
    engine.register_fn(
        "set_voltage",
        move |name: ImmutableString, voltage: rhai::FLOAT| -> Result<(), Box<EvalAltResult>> {
            sim_voltage
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_voltage(name.as_str(), voltage as f32)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_voltage_id = Arc::clone(sim);
    engine.register_fn(
        "set_voltage",
        move |channel: VoltageChannel, voltage: rhai::FLOAT| -> Result<(), Box<EvalAltResult>> {
            sim_voltage_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_voltage_channel(channel, voltage as f32);
            Ok(())
        },
    );

    let sim_uart1_config = Arc::clone(sim);
    engine.register_fn(
        "uart1_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: i64,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config =
                script_uart_config(data_bits, baud_rate, stop_bits as rhai::FLOAT, &parity)?;
            sim_uart1_config
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart1(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart1_config_float = Arc::clone(sim);
    engine.register_fn(
        "uart1_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: rhai::FLOAT,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config = script_uart_config(data_bits, baud_rate, stop_bits, &parity)?;
            sim_uart1_config_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart1(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart2_config = Arc::clone(sim);
    engine.register_fn(
        "uart2_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: i64,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config =
                script_uart_config(data_bits, baud_rate, stop_bits as rhai::FLOAT, &parity)?;
            sim_uart2_config
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart2(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart2_config_float = Arc::clone(sim);
    engine.register_fn(
        "uart2_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: rhai::FLOAT,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config = script_uart_config(data_bits, baud_rate, stop_bits, &parity)?;
            sim_uart2_config_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart2(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart_config_alias = Arc::clone(sim);
    engine.register_fn(
        "uart_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: i64,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config =
                script_uart_config(data_bits, baud_rate, stop_bits as rhai::FLOAT, &parity)?;
            sim_uart_config_alias
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart1(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart_config_alias_float = Arc::clone(sim);
    engine.register_fn(
        "uart_config",
        move |data_bits: i64,
              baud_rate: i64,
              stop_bits: rhai::FLOAT,
              parity: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            let config = script_uart_config(data_bits, baud_rate, stop_bits, &parity)?;
            sim_uart_config_alias_float
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .configure_uart1(config)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart_write = Arc::clone(sim);
    engine.register_fn(
        "uart_write",
        move |text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_uart_write
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_write(text.as_bytes())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart1_write = Arc::clone(sim);
    engine.register_fn(
        "uart1_write",
        move |text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_uart1_write
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_write(text.as_bytes())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart2_write = Arc::clone(sim);
    engine.register_fn(
        "uart2_write",
        move |text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_uart2_write
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_write(text.as_bytes())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart_write_raw = Arc::clone(sim);
    engine.register_fn(
        "uart_write_raw",
        move |symbols: Array| -> Result<(), Box<EvalAltResult>> {
            let symbols = script_uart_raw_values(symbols)?;
            sim_uart_write_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_write_raw(&symbols)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart1_write_raw = Arc::clone(sim);
    engine.register_fn(
        "uart1_write_raw",
        move |symbols: Array| -> Result<(), Box<EvalAltResult>> {
            let symbols = script_uart_raw_values(symbols)?;
            sim_uart1_write_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_write_raw(&symbols)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_uart2_write_raw = Arc::clone(sim);
    engine.register_fn(
        "uart2_write_raw",
        move |symbols: Array| -> Result<(), Box<EvalAltResult>> {
            let symbols = script_uart_raw_values(symbols)?;
            sim_uart2_write_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_write_raw(&symbols)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_da_value = Arc::clone(sim);
    engine.register_fn("da_value", move || -> Result<i64, Box<EvalAltResult>> {
        let value = sim_da_value
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .da_value();
        Ok(i64::from(value))
    });

    let sim_eeprom_byte = Arc::clone(sim);
    engine.register_fn(
        "eeprom_byte",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("EEPROM 地址必须在 0..=255"))?;
            let value = sim_eeprom_byte
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .eeprom_byte(addr);
            Ok(i64::from(value))
        },
    );

    let sim_set_eeprom_byte = Arc::clone(sim);
    engine.register_fn(
        "set_eeprom",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("EEPROM 地址必须在 0..=255"))?;
            let value =
                u8::try_from(value).map_err(|_| runtime_error("EEPROM 字节必须在 0..=255"))?;
            sim_set_eeprom_byte
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_eeprom_byte(addr, value);
            Ok(())
        },
    );

    let sim_set_eeprom_block = Arc::clone(sim);
    engine.register_fn(
        "set_eeprom",
        move |addr: i64, values: Array| -> Result<(), Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("EEPROM 地址必须在 0..=255"))?;
            let values = script_byte_array(values, "set_eeprom")?;
            sim_set_eeprom_block
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_eeprom_bytes(addr, &values)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_set_eeprom_from_zero = Arc::clone(sim);
    engine.register_fn(
        "set_eeprom",
        move |values: Array| -> Result<(), Box<EvalAltResult>> {
            let values = script_byte_array(values, "set_eeprom")?;
            sim_set_eeprom_from_zero
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_eeprom_bytes(0, &values)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_peek_iram = Arc::clone(sim);
    engine.register_fn(
        "peek_iram",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("IRAM 地址必须在 0..=255"))?;
            let value = sim_peek_iram
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_iram(addr);
            Ok(i64::from(value))
        },
    );

    let sim_peek_idata = Arc::clone(sim);
    engine.register_fn(
        "peek_idata",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("IDATA 地址必须在 0..=255"))?;
            let value = sim_peek_idata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_iram(addr);
            Ok(i64::from(value))
        },
    );

    let sim_peek_data = Arc::clone(sim);
    engine.register_fn(
        "peek_data",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("DATA 地址必须在 0..=127"))?;
            if addr > 0x7F {
                return Err(runtime_error("DATA 地址必须在 0..=127"));
            }
            let value = sim_peek_data
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_iram(addr);
            Ok(i64::from(value))
        },
    );

    let sim_poke_iram = Arc::clone(sim);
    engine.register_fn(
        "poke_iram",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("IRAM 地址必须在 0..=255"))?;
            let value =
                u8::try_from(value).map_err(|_| runtime_error("IRAM 字节必须在 0..=255"))?;
            sim_poke_iram
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_iram(addr, value);
            Ok(())
        },
    );

    let sim_poke_idata = Arc::clone(sim);
    engine.register_fn(
        "poke_idata",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("IDATA 地址必须在 0..=255"))?;
            let value =
                u8::try_from(value).map_err(|_| runtime_error("IDATA 字节必须在 0..=255"))?;
            sim_poke_idata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_iram(addr, value);
            Ok(())
        },
    );

    let sim_poke_data = Arc::clone(sim);
    engine.register_fn(
        "poke_data",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("DATA 地址必须在 0..=127"))?;
            if addr > 0x7F {
                return Err(runtime_error("DATA 地址必须在 0..=127"));
            }
            let value =
                u8::try_from(value).map_err(|_| runtime_error("DATA 字节必须在 0..=255"))?;
            sim_poke_data
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_iram(addr, value);
            Ok(())
        },
    );

    let sim_peek_sfr = Arc::clone(sim);
    engine.register_fn(
        "peek_sfr",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("SFR 地址必须在 0x80..=0xFF"))?;
            let value = sim_peek_sfr
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_sfr(addr)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(i64::from(value))
        },
    );

    let sim_peek_sfr_latch = Arc::clone(sim);
    engine.register_fn(
        "peek_sfr_latch",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("SFR 地址必须在 0x80..=0xFF"))?;
            let value = sim_peek_sfr_latch
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_sfr_latch(addr)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(i64::from(value))
        },
    );

    let sim_poke_sfr = Arc::clone(sim);
    engine.register_fn(
        "poke_sfr",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr =
                u8::try_from(addr).map_err(|_| runtime_error("SFR 地址必须在 0x80..=0xFF"))?;
            let value = u8::try_from(value).map_err(|_| runtime_error("SFR 字节必须在 0..=255"))?;
            sim_poke_sfr
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_sfr(addr, value)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_peek_xdata = Arc::clone(sim);
    engine.register_fn(
        "peek_xdata",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr =
                u16::try_from(addr).map_err(|_| runtime_error("XDATA 地址必须在 0..=65535"))?;
            let value = sim_peek_xdata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_xdata(addr);
            Ok(i64::from(value))
        },
    );

    let sim_peek_pdata = Arc::clone(sim);
    engine.register_fn(
        "peek_pdata",
        move |addr: i64| -> Result<i64, Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("PDATA 地址必须在 0..=255"))?;
            let value = sim_peek_pdata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .peek_xdata(u16::from(addr));
            Ok(i64::from(value))
        },
    );

    let sim_poke_xdata = Arc::clone(sim);
    engine.register_fn(
        "poke_xdata",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr =
                u16::try_from(addr).map_err(|_| runtime_error("XDATA 地址必须在 0..=65535"))?;
            let value =
                u8::try_from(value).map_err(|_| runtime_error("XDATA 字节必须在 0..=255"))?;
            sim_poke_xdata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_xdata(addr, value);
            Ok(())
        },
    );

    let sim_poke_pdata = Arc::clone(sim);
    engine.register_fn(
        "poke_pdata",
        move |addr: i64, value: i64| -> Result<(), Box<EvalAltResult>> {
            let addr = u8::try_from(addr).map_err(|_| runtime_error("PDATA 地址必须在 0..=255"))?;
            let value =
                u8::try_from(value).map_err(|_| runtime_error("PDATA 字节必须在 0..=255"))?;
            sim_poke_pdata
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .poke_xdata(u16::from(addr), value);
            Ok(())
        },
    );

    let sim_uart_take = Arc::clone(sim);
    engine.register_fn(
        "uart_take",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart_take
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_take_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart_take_segment = Arc::clone(sim);
    engine.register_fn(
        "uart_take",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart_take_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_take_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart1_take = Arc::clone(sim);
    engine.register_fn(
        "uart1_take",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart1_take
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_take_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart1_take_segment = Arc::clone(sim);
    engine.register_fn(
        "uart1_take",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart1_take_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_take_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart2_take = Arc::clone(sim);
    engine.register_fn(
        "uart2_take",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart2_take
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_take_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart2_take_segment = Arc::clone(sim);
    engine.register_fn(
        "uart2_take",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart2_take_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_take_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart_take_raw = Arc::clone(sim);
    engine.register_fn(
        "uart_take_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart_take_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_take_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart_take_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart_take_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart_take_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_take_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart1_take_raw = Arc::clone(sim);
    engine.register_fn(
        "uart1_take_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart1_take_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_take_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart1_take_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart1_take_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart1_take_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_take_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart2_take_raw = Arc::clone(sim);
    engine.register_fn(
        "uart2_take_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart2_take_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_take_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart2_take_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart2_take_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart2_take_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_take_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart_peek = Arc::clone(sim);
    engine.register_fn(
        "uart_peek",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart_peek
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_peek_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart_peek_segment = Arc::clone(sim);
    engine.register_fn(
        "uart_peek",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart_peek_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_peek_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart1_peek = Arc::clone(sim);
    engine.register_fn(
        "uart1_peek",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart1_peek
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_peek_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart1_peek_segment = Arc::clone(sim);
    engine.register_fn(
        "uart1_peek",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart1_peek_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_peek_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart2_peek = Arc::clone(sim);
    engine.register_fn(
        "uart2_peek",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_uart2_peek
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_peek_string()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart2_peek_segment = Arc::clone(sim);
    engine.register_fn(
        "uart2_peek",
        move |idle_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let text = sim_uart2_peek_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_peek_string_segment(idle_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_uart_peek_raw = Arc::clone(sim);
    engine.register_fn(
        "uart_peek_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart_peek_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_peek_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart_peek_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart_peek_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart_peek_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_peek_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart1_peek_raw = Arc::clone(sim);
    engine.register_fn(
        "uart1_peek_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart1_peek_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_peek_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart1_peek_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart1_peek_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart1_peek_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart1_peek_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart2_peek_raw = Arc::clone(sim);
    engine.register_fn(
        "uart2_peek_raw",
        move || -> Result<Array, Box<EvalAltResult>> {
            let symbols = sim_uart2_peek_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_peek_raw();
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_uart2_peek_raw_segment = Arc::clone(sim);
    engine.register_fn(
        "uart2_peek_raw",
        move |idle_ms: i64| -> Result<Array, Box<EvalAltResult>> {
            let idle_ms = script_duration_ms(idle_ms, "idle_ms")?;
            let symbols = sim_uart2_peek_raw_segment
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart2_peek_raw_segment(idle_ms);
            Ok(script_uart_raw_array(&symbols))
        },
    );

    let sim_display = Arc::clone(sim);
    engine.register_fn(
        "display_text",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_display
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .display_text();
            Ok(text.into())
        },
    );

    let sim_display_window = Arc::clone(sim);
    engine.register_fn(
        "display_text",
        move |duration_ms: i64| -> Result<ImmutableString, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let text = sim_display_window
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .observe_display_text(duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(text.into())
        },
    );

    let sim_display_number = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move || -> Result<Dynamic, Box<EvalAltResult>> {
            let value = sim_display_number
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .display_number()
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(display_number_dynamic(value))
        },
    );

    let sim_display_number_window = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |duration_ms: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let value = sim_display_number_window
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .observe_display_number(duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(display_number_dynamic(value))
        },
    );

    let sim_display_number_range = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |start: i64, end: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let (start, end) = script_range(start, end)?;
            let value = sim_display_number_range
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .display_number_in_range(start, end)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(display_number_dynamic(value))
        },
    );

    let sim_display_number_range_window = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |start: i64, end: i64, duration_ms: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let (start, end) = script_range(start, end)?;
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let value = sim_display_number_range_window
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .observe_display_number_in_range(start, end, duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(display_number_dynamic(value))
        },
    );

    engine.register_fn(
        "regex_is_match",
        move |text: ImmutableString,
              pattern: ImmutableString|
              -> Result<bool, Box<EvalAltResult>> {
            let regex = Regex::new(pattern.as_str())
                .map_err(|err| runtime_error(format!("正则表达式编译失败: {err}")))?;
            Ok(regex.is_match(text.as_str()))
        },
    );

    engine.register_fn(
        "regex_match",
        move |text: ImmutableString,
              pattern: ImmutableString|
              -> Result<bool, Box<EvalAltResult>> {
            let regex = Regex::new(pattern.as_str())
                .map_err(|err| runtime_error(format!("正则表达式编译失败: {err}")))?;
            Ok(regex.is_match(text.as_str()))
        },
    );

    let sim_seg_decode = Arc::clone(sim);
    engine.register_fn(
        "set_seg_decode",
        move |pattern: i64, text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            let pattern = u8::try_from(pattern).map_err(|_| runtime_error("pattern 越界"))?;
            sim_seg_decode
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_seg_decode(pattern, text.as_str())
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_seg_blank = Arc::clone(sim);
    engine.register_fn(
        "set_seg_blank",
        move |pattern: i64| -> Result<(), Box<EvalAltResult>> {
            let pattern = u8::try_from(pattern).map_err(|_| runtime_error("pattern 越界"))?;
            sim_seg_blank
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_seg_blank(pattern);
            Ok(())
        },
    );

    let sim_seg_raw = Arc::clone(sim);
    engine.register_fn(
        "seg_raw",
        move |index: i64| -> Result<i64, Box<EvalAltResult>> {
            let index = usize::try_from(index).map_err(|_| runtime_error("数码管编号必须 >= 0"))?;
            let value = sim_seg_raw
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .seg_raw(index)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(i64::from(value))
        },
    );

    let sim_seg_pattern = Arc::clone(sim);
    engine.register_fn(
        "seg_pattern",
        move |index: i64| -> Result<i64, Box<EvalAltResult>> {
            let index = usize::try_from(index).map_err(|_| runtime_error("数码管编号必须 >= 0"))?;
            let value = sim_seg_pattern
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .seg_pattern(index)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(i64::from(value))
        },
    );

    let sim_snapshot = Arc::clone(sim);
    engine.register_fn(
        "snapshot_text",
        move || -> Result<ImmutableString, Box<EvalAltResult>> {
            let text = sim_snapshot
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .snapshot_text();
            Ok(text.into())
        },
    );

    let sim_relay = Arc::clone(sim);
    engine.register_fn("relay_on", move || -> Result<bool, Box<EvalAltResult>> {
        let value = sim_relay
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .relay_on();
        Ok(value)
    });

    let sim_buzzer = Arc::clone(sim);
    engine.register_fn("buzzer_on", move || -> Result<bool, Box<EvalAltResult>> {
        let value = sim_buzzer
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .buzzer_on();
        Ok(value)
    });

    let sim_motor = Arc::clone(sim);
    engine.register_fn("motor_on", move || -> Result<bool, Box<EvalAltResult>> {
        let value = sim_motor
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .motor_on();
        Ok(value)
    });

    let sim_led = Arc::clone(sim);
    engine.register_fn(
        "led_on",
        move |index: i64| -> Result<bool, Box<EvalAltResult>> {
            let index = usize::try_from(index).map_err(|_| runtime_error("LED 编号必须 >= 0"))?;
            let value = sim_led
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .led_on(index)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(value)
        },
    );

    let sim_led_id = Arc::clone(sim);
    engine.register_fn(
        "led_on",
        move |led: LedId| -> Result<bool, Box<EvalAltResult>> {
            let value = sim_led_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .led_on_id(led);
            Ok(value)
        },
    );

    let sim_watch_led_stats = Arc::clone(sim);
    engine.register_fn(
        "watch_led_stats",
        move |name: ImmutableString, duration_ms: i64| -> Result<Map, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let stats = sim_watch_led_stats
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_stats(
                    LedId::parse(name.as_str()).map_err(|err| runtime_error(err.to_string()))?,
                    duration_ms,
                )
                .map_err(|err| runtime_error(err.to_string()))?;
            led_stats_map(stats)
        },
    );

    let sim_watch_led_stats_id = Arc::clone(sim);
    engine.register_fn(
        "watch_led_stats",
        move |led: LedId, duration_ms: i64| -> Result<Map, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let stats = sim_watch_led_stats_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_stats(led, duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            led_stats_map(stats)
        },
    );

    engine.register_fn(
        "assert",
        move |cond: bool, message: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            if cond {
                return Ok(());
            }
            Err(runtime_error(message))
        },
    );

    engine.register_fn(
        "assert_eq",
        move |actual: Dynamic,
              expected: Dynamic,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_eq_dynamic(&actual, &expected, label.as_str())
        },
    );

    engine.register_fn(
        "assert_regex",
        move |actual: ImmutableString,
              pattern: ImmutableString,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_regex_match(actual.as_str(), pattern.as_str(), label.as_str())
        },
    );

    engine.register_fn(
        "assert_in",
        move |actual: i64,
              range: Range<i64>,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_in_int_exclusive(actual, &range, label.as_str())
        },
    );

    engine.register_fn(
        "assert_in",
        move |actual: i64,
              range: RangeInclusive<i64>,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_in_int_inclusive(actual, &range, label.as_str())
        },
    );

    engine.register_fn(
        "assert_in",
        move |actual: f64,
              range: Range<i64>,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_in_float_exclusive(actual, &range, label.as_str())
        },
    );

    engine.register_fn(
        "assert_in",
        move |actual: f64,
              range: RangeInclusive<i64>,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            assert_in_float_inclusive(actual, &range, label.as_str())
        },
    );

    register_checkpoint_api(engine, sim, checkpoint_state);
}

fn register_checkpoint_api(
    engine: &mut Engine,
    sim: &Arc<Mutex<Simulator>>,
    checkpoint_state: &Arc<Mutex<ScriptCheckpointState>>,
) {
    let sim = Arc::clone(sim);
    let checkpoint_state = Arc::clone(checkpoint_state);
    engine.register_fn(
        "ckpt",
        move |ctx: NativeCallContext,
              index: i64,
              condition: ImmutableString,
              expected: ImmutableString,
              body: FnPtr|
              -> Result<(), Box<EvalAltResult>> {
            let result = body.call_within_context::<Dynamic>(&ctx, ());
            let (actual, actual_detail, status) = match result {
                Ok(value) => {
                    let text = checkpoint_success_message(&value);
                    (text.clone(), text, CheckpointStatus::Passed)
                }
                Err(err) => {
                    let detail = checkpoint_failure_detail(err.as_ref());
                    let summary = checkpoint_failure_summary(&detail);
                    (summary, detail, CheckpointStatus::Failed)
                }
            };
            let mut checkpoint_state = checkpoint_state
                .lock()
                .map_err(|_| runtime_error("评测点状态锁已损坏"))?;
            checkpoint_state.record(
                index,
                condition.as_str(),
                expected.as_str(),
                actual.clone(),
                if status == CheckpointStatus::Failed {
                    Some(actual_detail.clone())
                } else {
                    None
                },
                status,
            );
            drop(checkpoint_state);

            match status {
                CheckpointStatus::Passed => {
                    info!(
                        ckpt_index = index,
                        ckpt_status = status.as_str(),
                        ckpt_condition = condition.as_str(),
                        ckpt_expected = expected.as_str(),
                        ckpt_actual = actual.as_str(),
                        sim_time_ns = current_sim_time_ns(&sim),
                        "评测点执行结束"
                    );
                }
                CheckpointStatus::Failed => {
                    info!(
                        ckpt_index = index,
                        ckpt_status = status.as_str(),
                        ckpt_condition = condition.as_str(),
                        ckpt_expected = expected.as_str(),
                        ckpt_actual = actual.as_str(),
                        sim_time_ns = current_sim_time_ns(&sim),
                        "评测点执行结束"
                    );
                    info!("{}", format_checkpoint_stack_log(index, &actual_detail));
                }
            }

            Ok(())
        },
    );
}

fn register_run_to_api(engine: &mut Engine, sim: &Arc<Mutex<Simulator>>) {
    let sim_target_edge = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: RunToTarget, edge: RunToEdge| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(&sim_target_edge, target, edge, None)
        },
    );

    let sim_target_edge_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: RunToTarget,
              edge: RunToEdge,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_target_edge_timeout,
                target,
                edge,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_target_edge_name = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: RunToTarget, edge: ImmutableString| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_target_edge_name,
                target,
                parse_run_to_edge(edge.as_str())?,
                None,
            )
        },
    );

    let sim_target_edge_name_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: RunToTarget,
              edge: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_target_edge_name_timeout,
                target,
                parse_run_to_edge(edge.as_str())?,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_led_edge = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |led: LedId, edge: RunToEdge| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(&sim_led_edge, RunToTarget::Led(led), edge, None)
        },
    );

    let sim_led_edge_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |led: LedId, edge: RunToEdge, timeout_ns: i64| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_led_edge_timeout,
                RunToTarget::Led(led),
                edge,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_led_edge_name = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |led: LedId, edge: ImmutableString| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_led_edge_name,
                RunToTarget::Led(led),
                parse_run_to_edge(edge.as_str())?,
                None,
            )
        },
    );

    let sim_led_edge_name_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |led: LedId,
              edge: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_led_edge_name_timeout,
                RunToTarget::Led(led),
                parse_run_to_edge(edge.as_str())?,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_key_edge = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |key: KeyId, edge: RunToEdge| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(&sim_key_edge, RunToTarget::Key(key), edge, None)
        },
    );

    let sim_key_edge_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |key: KeyId, edge: RunToEdge, timeout_ns: i64| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_key_edge_timeout,
                RunToTarget::Key(key),
                edge,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_key_edge_name = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |key: KeyId, edge: ImmutableString| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_key_edge_name,
                RunToTarget::Key(key),
                parse_run_to_edge(edge.as_str())?,
                None,
            )
        },
    );

    let sim_key_edge_name_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |key: KeyId,
              edge: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_key_edge_name_timeout,
                RunToTarget::Key(key),
                parse_run_to_edge(edge.as_str())?,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_signal_edge = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |signal: SignalId, edge: RunToEdge| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(&sim_signal_edge, signal_run_to_target(signal), edge, None)
        },
    );

    let sim_signal_edge_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |signal: SignalId,
              edge: RunToEdge,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_signal_edge_timeout,
                signal_run_to_target(signal),
                edge,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_signal_edge_name = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |signal: SignalId, edge: ImmutableString| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_signal_edge_name,
                signal_run_to_target(signal),
                parse_run_to_edge(edge.as_str())?,
                None,
            )
        },
    );

    let sim_signal_edge_name_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |signal: SignalId,
              edge: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_signal_edge_name_timeout,
                signal_run_to_target(signal),
                parse_run_to_edge(edge.as_str())?,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_name_edge = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: ImmutableString, edge: RunToEdge| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_name_edge,
                parse_run_to_target(target.as_str())?,
                edge,
                None,
            )
        },
    );

    let sim_name_edge_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: ImmutableString,
              edge: RunToEdge,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_name_edge_timeout,
                parse_run_to_target(target.as_str())?,
                edge,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_name_edge_name = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: ImmutableString, edge: ImmutableString| -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_name_edge_name,
                parse_run_to_target(target.as_str())?,
                parse_run_to_edge(edge.as_str())?,
                None,
            )
        },
    );

    let sim_name_edge_name_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |target: ImmutableString,
              edge: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_target_wait(
                &sim_name_edge_name_timeout,
                parse_run_to_target(target.as_str())?,
                parse_run_to_edge(edge.as_str())?,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );

    let sim_callback = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |ctx: NativeCallContext, predicate: FnPtr| -> Result<i64, Box<EvalAltResult>> {
            run_to_callback_wait(ctx, &sim_callback, predicate, None)
        },
    );

    let sim_callback_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to",
        move |ctx: NativeCallContext,
              predicate: FnPtr,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            run_to_callback_wait(
                ctx,
                &sim_callback_timeout,
                predicate,
                Some(script_duration_ns(timeout_ns, "timeout_ns")?),
            )
        },
    );
}

fn signal_run_to_target(signal: SignalId) -> RunToTarget {
    match signal {
        SignalId::SigOut => RunToTarget::Pin { port: 3, bit: 4 },
        SignalId::NetSig => RunToTarget::Ne555SigOut,
    }
}

fn parse_run_to_target(target: &str) -> Result<RunToTarget, Box<EvalAltResult>> {
    RunToTarget::parse(target).map_err(|err| runtime_error(err.to_string()))
}

fn parse_run_to_edge(edge: &str) -> Result<RunToEdge, Box<EvalAltResult>> {
    RunToEdge::parse(edge).map_err(|err| runtime_error(err.to_string()))
}

fn script_duration_ns(value: i64, label: &str) -> Result<u64, Box<EvalAltResult>> {
    u64::try_from(value).map_err(|_| runtime_error(format!("{label} 参数必须 >= 0")))
}

fn script_duration_ms(value: i64, label: &str) -> Result<u64, Box<EvalAltResult>> {
    let value = u64::try_from(value).map_err(|_| runtime_error(format!("{label} 参数必须 > 0")))?;
    if value == 0 {
        return Err(runtime_error(format!("{label} 参数必须 > 0")));
    }
    Ok(value)
}

fn run_to_target_wait(
    sim: &Arc<Mutex<Simulator>>,
    target: RunToTarget,
    edge: RunToEdge,
    timeout_ns: Option<u64>,
) -> Result<i64, Box<EvalAltResult>> {
    let elapsed_ns = sim
        .lock()
        .map_err(|_| runtime_error("仿真器锁已损坏"))?
        .run_to_target_with_timeout(target, edge, timeout_ns)
        .map_err(|err| runtime_error(err.to_string()))?;
    script_int(elapsed_ns, "run_to 返回值超出脚本整数范围")
}

fn run_to_callback_wait(
    ctx: NativeCallContext,
    sim: &Arc<Mutex<Simulator>>,
    predicate: FnPtr,
    timeout_ns: Option<u64>,
) -> Result<i64, Box<EvalAltResult>> {
    let start_ns = current_sim_time_ns(sim);
    loop {
        let ready = predicate
            .call_within_context::<bool>(&ctx, ())
            .map_err(|err| runtime_error(err.to_string()))?;
        let elapsed_ns = current_sim_time_ns(sim).saturating_sub(start_ns);
        if ready {
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns > timeout_ns
            {
                return Err(runtime_error(format!(
                    "run_to 回调等待超时: timeout_ns={timeout_ns}"
                )));
            }
            return script_int(elapsed_ns, "run_to 返回值超出脚本整数范围");
        }
        if let Some(timeout_ns) = timeout_ns
            && elapsed_ns >= timeout_ns
        {
            return Err(runtime_error(format!(
                "run_to 回调等待超时: timeout_ns={timeout_ns}"
            )));
        }
        sim.lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .step_once()
            .map_err(|err| runtime_error(err.to_string()))?;
    }
}

fn runtime_error(message: impl Into<String>) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(message.into().into(), rhai::Position::NONE).into()
}

fn checkpoint_failure_detail(err: &EvalAltResult) -> String {
    format_checkpoint_failure_detail(&err.to_string())
}

fn checkpoint_failure_summary(detail: &str) -> String {
    let normalized = detail.replace("\r\n", "\n").replace('\r', "\n");
    let first_line = normalized.lines().next().unwrap_or("").trim();
    let cutoff = [" @ '", " @ \"", " @ ", " / in ", " (line ", " [line "]
        .into_iter()
        .filter_map(|marker| first_line.find(marker))
        .min()
        .unwrap_or(first_line.len());
    let summary = first_line[..cutoff]
        .strip_prefix("Runtime error:")
        .map(str::trim)
        .unwrap_or(&first_line[..cutoff])
        .trim()
        .trim_end_matches('/')
        .trim();
    let summary = if summary.is_empty() {
        "运行时错误"
    } else {
        summary
    };
    summary.to_owned()
}

fn format_checkpoint_failure_detail(detail: &str) -> String {
    let normalized = detail
        .trim()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace(" / in ", "\nin ");
    if normalized.is_empty() {
        return "运行时错误".to_owned();
    }

    let frames = split_checkpoint_failure_frames(&normalized);
    if frames.len() <= 1 {
        return simplify_checkpoint_failure_head(&normalized);
    }

    let mut lines = Vec::with_capacity(frames.len() + 1);
    lines.push(simplify_checkpoint_failure_head(&frames[0]));
    lines.push("调用栈:".to_owned());
    for (index, frame) in frames.iter().enumerate().skip(1) {
        lines.push(format!(
            "  {}. {}",
            index,
            simplify_checkpoint_stack_frame(frame)
        ));
    }
    lines.join("\n")
}

fn split_checkpoint_failure_frames(detail: &str) -> Vec<String> {
    let mut frames = Vec::new();
    let mut current = String::new();

    for line in detail.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("in ") && !current.is_empty() {
            frames.push(current);
            current = line.to_owned();
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        frames.push(current);
    }

    frames
}

fn simplify_checkpoint_failure_head(frame: &str) -> String {
    format_checkpoint_frame_text(
        frame
            .trim()
            .strip_prefix("Runtime error:")
            .map(str::trim)
            .unwrap_or_else(|| frame.trim()),
    )
}

fn simplify_checkpoint_stack_frame(frame: &str) -> String {
    let frame = normalize_checkpoint_source_label(frame.trim());

    if let Some(rest) = frame.strip_prefix("in call to function ") {
        return format_checkpoint_frame_text(&format!("函数 {}", rest.trim()));
    }
    if let Some(rest) = frame.strip_prefix("in closure call") {
        let rest = rest.trim();
        if rest.is_empty() {
            return format_checkpoint_frame_text("闭包调用");
        }
        return format_checkpoint_frame_text(&format!("闭包调用 {}", rest));
    }
    if let Some(rest) = frame.strip_prefix("in ") {
        return format_checkpoint_frame_text(&format!("调用 {}", rest.trim()));
    }

    format_checkpoint_frame_text(&frame)
}

fn normalize_checkpoint_source_label(text: &str) -> String {
    text.replace("'file:", "'").replace("\"file:", "\"")
}

fn format_checkpoint_frame_text(text: &str) -> String {
    let (cleaned, location) = extract_checkpoint_frame_location(text);
    let cleaned = strip_checkpoint_source_origin(&cleaned);
    match location {
        Some(location) => format!("{cleaned} @ {location}"),
        None => cleaned,
    }
}

fn extract_checkpoint_frame_location(text: &str) -> (String, Option<String>) {
    if let Some((cleaned, location)) = strip_checkpoint_location_suffix(
        text,
        r#" @ ['"](?P<path>[^'"]+)['"] \(line (?P<line>\d+), position (?P<column>\d+)\)"#,
    ) {
        return (
            normalize_checkpoint_source_label(cleaned.trim()),
            Some(location),
        );
    }

    if let Some((cleaned, location)) = strip_checkpoint_location_suffix(
        text,
        r#"\(from ['"](?P<path>[^'"]+)['"]\) \(line (?P<line>\d+), position (?P<column>\d+)\)"#,
    ) {
        return (
            normalize_checkpoint_source_label(cleaned.trim()),
            Some(location),
        );
    }

    (normalize_checkpoint_source_label(text.trim()), None)
}

fn strip_checkpoint_location_suffix(text: &str, pattern: &str) -> Option<(String, String)> {
    let regex = Regex::new(pattern).expect("valid checkpoint location regex");
    let captures = regex.captures(text)?;
    let matched = captures.get(0)?;
    let path = captures.name("path")?.as_str();
    let line = captures.name("line")?.as_str();
    let column = captures.name("column")?.as_str();

    let mut cleaned = String::with_capacity(text.len().saturating_sub(matched.as_str().len()));
    cleaned.push_str(text[..matched.start()].trim_end());
    cleaned.push_str(text[matched.end()..].trim_start());

    Some((
        cleaned,
        format_checkpoint_vscode_location(path, line, column),
    ))
}

fn format_checkpoint_vscode_location(path: &str, line: &str, column: &str) -> String {
    format!(
        "{}:{}:{}",
        normalize_checkpoint_source_path(path),
        line,
        column
    )
}

fn normalize_checkpoint_source_path(path: &str) -> &str {
    path.strip_prefix("file:").unwrap_or(path)
}

fn strip_checkpoint_source_origin(frame: &str) -> String {
    let mut result = String::with_capacity(frame.len());
    let mut remaining = frame;

    while let Some(start) = remaining.find(" (from ") {
        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + 1..];
        let Some(end) = after_start.find(')') else {
            result.push_str(&remaining[start..]);
            return result;
        };
        remaining = &after_start[end + 1..];
    }

    result.push_str(remaining);
    result
}

fn indent_checkpoint_detail(detail: &str, prefix: &str) -> String {
    detail
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_checkpoint_stack_log(index: i64, detail: &str) -> String {
    format!(
        "ckpt[{index}] 调用堆栈\n{}",
        indent_checkpoint_detail(detail, "  ")
    )
}

fn checkpoint_success_message(value: &Dynamic) -> String {
    if value.is::<()>() {
        return "符合期望".to_owned();
    }
    if value.is::<ImmutableString>() {
        return value.clone_cast::<ImmutableString>().to_string();
    }
    format!("{value}")
}

fn display_number_dynamic(value: DisplayNumber) -> Dynamic {
    match value {
        DisplayNumber::Integer(value) => value.into(),
        DisplayNumber::Float(value) => value.into(),
    }
}

fn assert_eq_dynamic(
    actual: &Dynamic,
    expected: &Dynamic,
    label: &str,
) -> Result<(), Box<EvalAltResult>> {
    if actual.type_id() != expected.type_id() {
        return Err(runtime_error(format!(
            "{label}: 期望 {} ({}) , 实际 {} ({})",
            format_dynamic_value(expected),
            expected.type_name(),
            format_dynamic_value(actual),
            actual.type_name()
        )));
    }
    if dynamic_values_equal(actual, expected) {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望 {} , 实际 {}",
        format_dynamic_value(expected),
        format_dynamic_value(actual)
    )))
}

fn assert_regex_match(actual: &str, pattern: &str, label: &str) -> Result<(), Box<EvalAltResult>> {
    let regex = Regex::new(pattern)
        .map_err(|err| runtime_error(format!("{label}: 正则表达式编译失败: {err}")))?;
    if regex.is_match(actual) {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望匹配正则 `{pattern}` , 实际 `{actual}`"
    )))
}

fn dynamic_values_equal(actual: &Dynamic, expected: &Dynamic) -> bool {
    if actual.is::<ImmutableString>() {
        return actual.clone_cast::<ImmutableString>() == expected.clone_cast::<ImmutableString>();
    }
    if actual.is::<i64>() {
        return actual.clone_cast::<i64>() == expected.clone_cast::<i64>();
    }
    if actual.is::<f64>() {
        return actual.clone_cast::<f64>() == expected.clone_cast::<f64>();
    }
    if actual.is::<bool>() {
        return actual.clone_cast::<bool>() == expected.clone_cast::<bool>();
    }
    if actual.is::<char>() {
        return actual.clone_cast::<char>() == expected.clone_cast::<char>();
    }
    if actual.is::<Range<i64>>() {
        return actual.clone_cast::<Range<i64>>() == expected.clone_cast::<Range<i64>>();
    }
    if actual.is::<RangeInclusive<i64>>() {
        return actual.clone_cast::<RangeInclusive<i64>>()
            == expected.clone_cast::<RangeInclusive<i64>>();
    }
    if actual.is::<()>() {
        return true;
    }
    format!("{actual}") == format!("{expected}")
}

fn assert_in_int_exclusive(
    actual: i64,
    range: &Range<i64>,
    label: &str,
) -> Result<(), Box<EvalAltResult>> {
    if range.start >= range.end {
        return Err(runtime_error(format!(
            "{label}: 非法区间 {}..{}",
            range.start, range.end
        )));
    }
    if range.contains(&actual) {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望 {}..{} , 实际 {}",
        range.start, range.end, actual
    )))
}

fn assert_in_int_inclusive(
    actual: i64,
    range: &RangeInclusive<i64>,
    label: &str,
) -> Result<(), Box<EvalAltResult>> {
    let start = *range.start();
    let end = *range.end();
    if start > end {
        return Err(runtime_error(format!(
            "{label}: 非法区间 {}..={}",
            start, end
        )));
    }
    if range.contains(&actual) {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望 {}..={} , 实际 {}",
        start, end, actual
    )))
}

fn assert_in_float_exclusive(
    actual: f64,
    range: &Range<i64>,
    label: &str,
) -> Result<(), Box<EvalAltResult>> {
    if range.start >= range.end {
        return Err(runtime_error(format!(
            "{label}: 非法区间 {}..{}",
            range.start, range.end
        )));
    }
    let lower = range.start as f64;
    let upper = range.end as f64;
    if actual >= lower && actual < upper {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望 {}..{} , 实际 {}",
        range.start, range.end, actual
    )))
}

fn assert_in_float_inclusive(
    actual: f64,
    range: &RangeInclusive<i64>,
    label: &str,
) -> Result<(), Box<EvalAltResult>> {
    let start = *range.start();
    let end = *range.end();
    if start > end {
        return Err(runtime_error(format!(
            "{label}: 非法区间 {}..={}",
            start, end
        )));
    }
    let lower = start as f64;
    let upper = end as f64;
    if actual >= lower && actual <= upper {
        return Ok(());
    }
    Err(runtime_error(format!(
        "{label}: 期望 {}..={} , 实际 {}",
        start, end, actual
    )))
}

fn format_dynamic_value(value: &Dynamic) -> String {
    if value.is::<ImmutableString>() {
        return format!("`{}`", value.clone_cast::<ImmutableString>());
    }
    format!("{value}")
}

fn script_bool_field(value: Dynamic, label: &str) -> Result<bool, Box<EvalAltResult>> {
    value
        .as_bool()
        .map_err(|_| runtime_error(format!("{label} 必须是布尔值")))
}

fn script_byte_field(value: Dynamic, label: &str) -> Result<u8, Box<EvalAltResult>> {
    let value = value
        .as_int()
        .map_err(|_| runtime_error(format!("{label} 必须是整数")))?;
    u8::try_from(value).map_err(|_| runtime_error(format!("{label} 必须在 0..=255")))
}

fn script_rtc_hour_mode(value: Dynamic) -> Result<bool, Box<EvalAltResult>> {
    if value.is::<ImmutableString>() {
        let mode = value.clone_cast::<ImmutableString>();
        return match mode.trim().to_ascii_lowercase().as_str() {
            "12" | "12h" => Ok(true),
            "24" | "24h" => Ok(false),
            _ => Err(runtime_error("hour_mode 只支持 12, 24, `12h`, `24h`")),
        };
    }

    match value
        .as_int()
        .map_err(|_| runtime_error("hour_mode 只支持 12, 24, `12h`, `24h`"))?
    {
        12 => Ok(true),
        24 => Ok(false),
        _ => Err(runtime_error("hour_mode 只支持 12, 24, `12h`, `24h`")),
    }
}

fn script_rtc_state(state: Map) -> Result<Ds1302State, Box<EvalAltResult>> {
    if state.is_empty() {
        return Err(runtime_error("set_rtc 状态不能为空"));
    }

    let mut rtc_state = Ds1302State::default();
    let mut running = None;
    let mut hour_mode = None;

    for (key, value) in state {
        match key.as_str() {
            "hour" => rtc_state.hour = Some(script_byte_field(value, "hour")?),
            "minute" => rtc_state.minute = Some(script_byte_field(value, "minute")?),
            "second" => rtc_state.second = Some(script_byte_field(value, "second")?),
            "day_of_week" | "weekday" => {
                rtc_state.day_of_week = Some(script_byte_field(value, "day_of_week")?);
            }
            "date" => rtc_state.date = Some(script_byte_field(value, "date")?),
            "month" => rtc_state.month = Some(script_byte_field(value, "month")?),
            "year" => rtc_state.year = Some(script_byte_field(value, "year")?),
            "running" => running = Some(script_bool_field(value, "running")?),
            "halted" => rtc_state.halted = Some(script_bool_field(value, "halted")?),
            "hour_mode" => hour_mode = Some(script_rtc_hour_mode(value)?),
            "hour_mode_12" => {
                rtc_state.hour_mode_12 = Some(script_bool_field(value, "hour_mode_12")?);
            }
            "write_protect" => {
                rtc_state.write_protect = Some(script_bool_field(value, "write_protect")?);
            }
            "trickle_charge" => {
                rtc_state.trickle_charge = Some(script_byte_field(value, "trickle_charge")?);
            }
            _ => return Err(runtime_error(format!("set_rtc 不支持的字段: {key}"))),
        }
    }

    if let Some(running) = running {
        let halted = !running;
        if let Some(existing) = rtc_state.halted
            && existing != halted
        {
            return Err(runtime_error("running 和 halted 不能冲突"));
        }
        rtc_state.halted = Some(halted);
    }

    if let Some(hour_mode_12) = hour_mode {
        if let Some(existing) = rtc_state.hour_mode_12
            && existing != hour_mode_12
        {
            return Err(runtime_error("hour_mode 和 hour_mode_12 不能冲突"));
        }
        rtc_state.hour_mode_12 = Some(hour_mode_12);
    }

    Ok(rtc_state)
}

fn script_uart_config(
    data_bits: i64,
    baud_rate: i64,
    stop_bits: rhai::FLOAT,
    parity: &str,
) -> Result<UartConfig, Box<EvalAltResult>> {
    if !stop_bits.is_finite() {
        return Err(runtime_error("stop_bits 必须是有限数值"));
    }
    let data_bits = u8::try_from(data_bits).map_err(|_| runtime_error("data_bits 必须在 5..=9"))?;
    let baud_rate =
        u32::try_from(baud_rate).map_err(|_| runtime_error("baud_rate 必须在 1..=4294967295"))?;
    let stop_bits = match stop_bits {
        value if (value - 1.0).abs() < 1e-6 => UartStopBits::One,
        value if (value - 1.5).abs() < 1e-6 => UartStopBits::OnePointFive,
        value if (value - 2.0).abs() < 1e-6 => UartStopBits::Two,
        _ => return Err(runtime_error("stop_bits 只支持 1, 1.5, 2")),
    };
    let parity = match parity.trim().to_ascii_lowercase().as_str() {
        "n" | "none" => UartParity::None,
        "o" | "odd" => UartParity::Odd,
        "e" | "even" => UartParity::Even,
        "m" | "mark" => UartParity::Mark,
        "s" | "space" => UartParity::Space,
        _ => return Err(runtime_error("parity 只支持 none, odd, even, mark, space")),
    };
    let config = UartConfig {
        data_bits,
        baud_rate,
        stop_bits,
        parity,
    };
    config
        .validate()
        .map_err(|err| runtime_error(err.to_string()))?;
    Ok(config)
}

fn script_uart_raw_values(values: Array) -> Result<Vec<u16>, Box<EvalAltResult>> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value
                .as_int()
                .map_err(|_| runtime_error(format!("uart raw 第 {} 项必须是整数", index + 1)))?;
            u16::try_from(value)
                .map_err(|_| runtime_error(format!("uart raw 第 {} 项必须在 0..=65535", index + 1)))
        })
        .collect()
}

fn script_byte_array(values: Array, label: &str) -> Result<Vec<u8>, Box<EvalAltResult>> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value
                .as_int()
                .map_err(|_| runtime_error(format!("{label} 第 {} 项必须是整数", index + 1)))?;
            u8::try_from(value)
                .map_err(|_| runtime_error(format!("{label} 第 {} 项必须在 0..=255", index + 1)))
        })
        .collect()
}

fn script_uart_raw_array(values: &[u16]) -> Array {
    values
        .iter()
        .map(|value| Dynamic::from(i64::from(*value)))
        .collect()
}

fn script_range(start: i64, end: i64) -> Result<(usize, usize), Box<EvalAltResult>> {
    let start = usize::try_from(start).map_err(|_| runtime_error("start 参数必须 >= 0"))?;
    let end = usize::try_from(end).map_err(|_| runtime_error("end 参数必须 >= 0"))?;
    Ok((start, end))
}

fn script_int(value: u64, overflow_message: &str) -> Result<i64, Box<EvalAltResult>> {
    i64::try_from(value).map_err(|_| runtime_error(overflow_message))
}

fn script_time_target_ns(
    value: rhai::FLOAT,
    scale_ns: u64,
    label: &str,
) -> Result<u64, Box<EvalAltResult>> {
    if !value.is_finite() {
        return Err(runtime_error(format!("{label} 参数必须是有限数值")));
    }
    if value < 0.0 {
        return Err(runtime_error(format!("{label} 参数必须 >= 0")));
    }
    let total_ns = value * scale_ns as rhai::FLOAT;
    let rounded_ns = total_ns.round();
    if (total_ns - rounded_ns).abs() > 1e-6 {
        return Err(runtime_error(format!("{label} 参数精度不能小于 1ns")));
    }
    if rounded_ns > u64::MAX as rhai::FLOAT {
        return Err(runtime_error(format!("{label} 参数数值过大")));
    }
    Ok(rounded_ns as u64)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rhai::Scope;

    use super::{
        ScriptCheckpointState, ScriptTraceState, build_engine, build_scope, eval_source,
        eval_source_with_engine,
    };
    use crate::{chip::Simulator, persistent_state::PersistentState, wave::WaveCaptureOptions};

    fn dual_uart_echo_sim() -> Simulator {
        let code = vec![
            0x75, 0x98, 0x10, 0x75, 0x9A, 0x10, 0xE5, 0x98, 0x54, 0x01, 0x60, 0x07, 0xE5, 0x99,
            0x53, 0x98, 0xFE, 0xF5, 0x99, 0xE5, 0x9A, 0x54, 0x01, 0x60, 0x07, 0xE5, 0x9B, 0x53,
            0x9A, 0xFE, 0xF5, 0x9B, 0x80, 0xE4,
        ];
        Simulator::from_code_with_options(code, false, WaveCaptureOptions::default())
    }

    fn uart2_ninth_bit_echo_sim() -> Simulator {
        let code = vec![
            0x75, 0x9A, 0x10, 0xE5, 0x9A, 0x54, 0x01, 0x60, 0xFA, 0xE5, 0x9A, 0x54, 0x08, 0x60,
            0x05, 0x43, 0x9A, 0x04, 0x80, 0x03, 0x53, 0x9A, 0xFB, 0xE5, 0x9B, 0x53, 0x9A, 0xFE,
            0xF5, 0x9B, 0x80, 0xE3,
        ];
        Simulator::from_code_with_options(code, false, WaveCaptureOptions::default())
    }

    fn checkpoint_state() -> Arc<Mutex<ScriptCheckpointState>> {
        Arc::new(Mutex::new(ScriptCheckpointState::default()))
    }

    #[test]
    fn rhai_run_to_supports_signal_constants_and_absolute_time() {
        let sim = Simulator::nop(false);
        let script = r#"
            set_frequency_hz(2000);
            jumper_on(NET_SIG, SIG_OUT);
            let t0 = run_to_ns(1000);
            assert(t0 >= 1000, "run_to_ns should advance to target time");
            let t1 = run_to(NET_SIG, FLIP);
            assert(t1 > 0, "run_to should detect NET_SIG flip");
            let t2 = run_to(SIG_OUT, FLIP);
            assert(t2 > 0, "run_to should detect SIG_OUT flip");
        "#;
        eval_source(sim, "test:run_to", script).expect("run rhai script");
    }

    #[test]
    fn rhai_run_to_supports_timeout_and_callback_predicate() {
        let sim = Simulator::nop(false);
        let script = r#"
            set_frequency_hz(2000);
            jumper_on(NET_SIG, SIG_OUT);

            let dt0 = run_to(NET_SIG, FLIP, 1_000_000);
            assert(dt0 > 0, "edge run_to with timeout should succeed");

            let dt1 = run_to(|| led_on(L1), 10_000);
            assert_eq(dt1, 0, "callback run_to should return immediately when already true");

            let target_ns = sim_time_ns() + 20_000;
            let dt2 = run_to(|| sim_time_ns() >= target_ns, 30_000);
            assert_in(dt2, 20_000..=30_000, "callback run_to should wait until condition becomes true");
        "#;
        eval_source(sim, "test:run_to_timeout", script).expect("run rhai timeout script");
    }

    #[test]
    fn rhai_run_to_timeout_reports_failure() {
        let sim = Simulator::nop(false);
        let script = r#"
            run_to(|| false, 1000);
        "#;
        let err = eval_source(sim, "test:run_to_timeout_fail", script).unwrap_err();
        assert!(err.to_string().contains("超时"));
    }

    #[test]
    fn rhai_assert_regex_reports_pattern_and_actual() {
        let sim = Simulator::nop(false);
        let script = r#"
            assert_regex("123", "^\\d+$", "digits ok");
            assert_regex("abc", "^\\d+$", "digits bad");
        "#;
        let err = eval_source(sim, "test:assert_regex", script).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("digits bad"));
        assert!(message.contains("^\\d+$"));
        assert!(message.contains("abc"));
    }

    #[test]
    fn rhai_reset_supports_explicit_modes() {
        let sim = Simulator::nop(false);
        let script = r#"
            run_us(10);
            let before = sim_time_ns();
            set_frequency_hz(1500);
            reset(CPU_RESET);
            assert_eq(sim_time_ns(), before, "cpu reset should keep current timestamp");
            reset(POWER_RESET);
            assert_eq(sim_time_ns(), 0, "power reset should restart simulator time");
        "#;
        eval_source(sim, "test:reset_modes", script).expect("run reset mode script");
    }

    #[test]
    fn rhai_uart_api_supports_dual_channels_and_raw_config() {
        let sim = dual_uart_echo_sim();
        let script = r#"
            uart1_config(8, 9600, 1, "none");
            uart2_config(8, 19200, 1.5, "even");

            uart_write("A");
            uart1_write("B");
            uart2_write("Z");

            run_ms(40);

            assert_eq(uart_take(), "AB", "uart alias should drain uart1 text queue");
            assert_eq(uart1_take(), "", "uart1 queue should already be empty");
            assert_eq(uart2_take(), "Z", "uart2 text queue");
            assert_eq(uart2_take(), "", "uart2 queue should be empty after take");
        "#;

        eval_source(sim, "test:dual_uart_api", script).expect("run dual uart script");
    }

    #[test]
    fn rhai_uart2_raw_api_preserves_ninth_bit() {
        let sim = uart2_ninth_bit_echo_sim();
        let script = r#"
            uart2_config(9, 19200, 1, "none");
            uart2_write_raw([0x141, 0x156]);
            run_ms(40);

            let raw = uart2_take_raw();
            assert_eq(len(raw), 2, "uart2 raw queue length");
            assert_eq(raw[0], 0x141, "uart2 first 9-bit symbol");
            assert_eq(raw[1], 0x156, "uart2 second 9-bit symbol");
        "#;

        eval_source(sim, "test:uart2_raw_api", script).expect("run uart2 raw script");
    }

    #[test]
    fn rhai_uart_take_without_idle_window_still_drains_full_queue() {
        let sim = dual_uart_echo_sim();
        let script = r#"
            uart1_config(8, 9600, 1, "none");

            uart_write("OK");
            run_ms(20);
            uart_write("ERROR");
            run_ms(20);

            assert_eq(uart_take(), "OKERROR", "uart_take should still drain the full queue");
            assert_eq(uart_take(), "", "uart_take should clear the queue");
        "#;

        eval_source(sim, "test:uart_take_full_queue", script)
            .expect("run uart take full queue script");
    }

    #[test]
    fn rhai_uart_segment_take_and_peek_split_by_idle_gap() {
        let sim = dual_uart_echo_sim();
        let script = r#"
            uart1_config(8, 9600, 1, "none");

            uart_write("OK");
            run_ms(20);
            uart_write("ERROR");
            run_ms(20);
            uart_write("OK");
            run_ms(20);

            assert_eq(uart_peek(), "OKERROROK", "uart_peek should expose the full host receive queue");
            assert_eq(uart_peek(10), "OK", "uart_peek(idle_ms) should only read the first segment");
            assert_eq(uart_take(10), "OK", "first segmented take");
            assert_eq(uart_peek(10), "ERROR", "peek should advance after the first segment is consumed");
            assert_eq(uart_take(10), "ERROR", "second segmented take");
            assert_eq(uart_take(10), "OK", "third segmented take");
            assert_eq(uart_take(10), "", "segmented take should clear the queue");
            assert_eq(uart_peek(), "", "peek should see an empty queue after segmented drains");
        "#;

        eval_source(sim, "test:uart_segment_take_peek", script)
            .expect("run uart segment take and peek script");
    }

    #[test]
    fn rhai_add_marker_overloads_record_expected_markers() {
        let unique_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let sim = Arc::new(Mutex::new(Simulator::nop_with_options(
            false,
            WaveCaptureOptions {
                html_path: None,
                json_path: None,
                msgpack_path: Some(
                    std::env::temp_dir()
                        .join(format!("stcjudge-script-marker-{unique_ns}.msgpack")),
                ),
                start_ns: 0,
                end_ns: None,
            },
        )));
        let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
        let checkpoint_state = checkpoint_state();
        let engine = build_engine(&sim, &trace_state, &checkpoint_state);
        let mut scope: Scope<'static> = build_scope();
        let script = r#"
            add_marker();
            run_us(5);
            add_marker("after_5us");
            add_marker(1000);
            add_marker(2000, "label_2us");
        "#;

        eval_source_with_engine(&engine, &mut scope, &trace_state, "test:add_marker", script)
            .expect("run add_marker script");

        let markers = sim.lock().expect("lock sim").recorded_wave_markers();
        assert_eq!(
            markers,
            vec![
                (0, None),
                (5_000, Some(String::from("after_5us"))),
                (1_000, None),
                (2_000, Some(String::from("label_2us"))),
            ]
        );
    }

    #[test]
    fn rhai_add_marker_rejects_negative_timestamp() {
        let sim = Simulator::nop(false);
        let script = r#"
            add_marker(-1, "bad");
        "#;
        let err = eval_source(sim, "test:add_marker_negative", script).unwrap_err();
        assert!(err.to_string().contains("add_marker 时间戳必须 >= 0"));
    }

    #[test]
    fn rhai_set_rtc_map_supports_partial_state_control() {
        let sim = Arc::new(Mutex::new(Simulator::nop(false)));
        let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
        let checkpoint_state = checkpoint_state();
        let engine = build_engine(&sim, &trace_state, &checkpoint_state);
        let mut scope: Scope<'static> = build_scope();
        let script = r#"
            set_rtc(#{
                hour: 11,
                minute: 22,
                second: 33,
                year: 24,
                month: 12,
                date: 31,
                weekday: 2,
                running: false,
                hour_mode: "12h",
                write_protect: true,
                trickle_charge: 0xA5,
            });
            set_rtc(#{
                running: true,
                hour_mode: 24,
                second: 40,
            });
        "#;

        eval_source_with_engine(
            &engine,
            &mut scope,
            &trace_state,
            "test:set_rtc_map",
            script,
        )
        .expect("run set_rtc map script");

        let encoded = sim.lock().expect("lock sim").export_persistent_state();
        let state = PersistentState::decode(&encoded).expect("decode persistent state");
        assert_eq!(state.ds1302.hour, 11);
        assert_eq!(state.ds1302.minute, 22);
        assert_eq!(state.ds1302.second, 40);
        assert_eq!(state.ds1302.year, 24);
        assert_eq!(state.ds1302.month, 12);
        assert_eq!(state.ds1302.date, 31);
        assert_eq!(state.ds1302.day_of_week, 2);
        assert!(!state.ds1302.halted);
        assert!(!state.ds1302.hour_mode_12);
        assert!(state.ds1302.write_protect);
        assert_eq!(state.ds1302.trickle_charge, 0xA5);
    }

    #[test]
    fn rhai_set_eeprom_supports_single_and_block_writes() {
        let sim = Arc::new(Mutex::new(Simulator::nop(false)));
        let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
        let checkpoint_state = checkpoint_state();
        let engine = build_engine(&sim, &trace_state, &checkpoint_state);
        let mut scope: Scope<'static> = build_scope();
        let script = r#"
            set_eeprom(0x10, 0xAB);
            set_eeprom(0x20, [1, 2, 3, 255]);
            set_eeprom([0x55, 0x66]);
        "#;

        eval_source_with_engine(&engine, &mut scope, &trace_state, "test:set_eeprom", script)
            .expect("run set_eeprom script");

        let encoded = sim.lock().expect("lock sim").export_persistent_state();
        let state = PersistentState::decode(&encoded).expect("decode persistent state");
        assert_eq!(state.at24c02.memory[0x00], 0x55);
        assert_eq!(state.at24c02.memory[0x01], 0x66);
        assert_eq!(state.at24c02.memory[0x10], 0xAB);
        assert_eq!(state.at24c02.memory[0x20], 0x01);
        assert_eq!(state.at24c02.memory[0x21], 0x02);
        assert_eq!(state.at24c02.memory[0x22], 0x03);
        assert_eq!(state.at24c02.memory[0x23], 0xFF);
    }

    #[test]
    fn rhai_memory_access_supports_iram_sfr_and_xdata() {
        let sim = Simulator::nop(false);
        let script = r#"
            assert_eq(peek_iram(0x30), 0, "IRAM 默认值错误");
            poke_iram(0x30, 0x5A);
            assert_eq(peek_iram(0x30), 0x5A, "IRAM 回读错误");
            assert_eq(peek_idata(0x30), 0x5A, "IDATA 别名回读错误");
            poke_data(0x31, 0x66);
            assert_eq(peek_data(0x31), 0x66, "DATA 别名回读错误");

            assert_eq(peek_sfr(0x8E), 0x01, "AUXR 默认值错误");
            poke_sfr(0x8E, 0x34);
            assert_eq(peek_sfr(0x8E), 0x34, "AUXR 回读错误");

            poke_sfr(0x90, 0x55);
            assert_eq(peek_sfr_latch(0x90), 0x55, "P1 锁存值错误");

            assert_eq(peek_xdata(0x1234), 0, "XDATA 默认值错误");
            poke_xdata(0x1234, 0xAB);
            assert_eq(peek_xdata(0x1234), 0xAB, "XDATA 回读错误");
            poke_pdata(0x32, 0x77);
            assert_eq(peek_pdata(0x32), 0x77, "PDATA 别名回读错误");
        "#;

        eval_source(sim, "test:memory_access", script).expect("run memory access script");
    }

    #[test]
    fn rhai_ckpt_collects_failures_and_continues() {
        let sim = Simulator::nop(false);
        let script = r#"
            let visited = 0;
            ckpt(1, "第一项", "应该失败", || {
                assert_eq(1, 2, "ckpt bad");
            });
            ckpt(2, "第二项", "应该通过", || {
                visited += 1;
                assert_eq(visited, 1, "ckpt good");
            });
        "#;
        let err = eval_source(sim, "test:ckpt", script).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("ckpt 失败: 1/2"), "{message}");
    }

    #[test]
    fn checkpoint_failure_summary_drops_rhai_stack_frames() {
        let detail = "Runtime error: 圆柱体 76cm 体积页 剩余空间 @ 'file:sample/na16/judge/4t.rhai' (line 172, position 5) / in call to function 'assert_volume_page' (from 'file:sample/na16/judge/4t.rhai') @ 'file:sample/na16/judge/4t.rhai' (line 801, position 5) / in closure call (from 'file:sample/na16/judge/4t.rhai') (line 798, position 1)";
        assert_eq!(
            super::checkpoint_failure_summary(detail),
            "圆柱体 76cm 体积页 剩余空间"
        );
    }

    #[test]
    fn checkpoint_failure_detail_formats_stack_as_multiline() {
        let detail = "Runtime error: bad: 期望 2 , 实际 1 @ 'file:test.rhai' (line 2, position 5)\nin call to function 'inner' (from 'file:test.rhai') @ 'file:test.rhai' (line 6, position 5)\nin closure call (from 'file:test.rhai') (line 5, position 1)";
        let formatted = super::format_checkpoint_failure_detail(detail);
        assert!(formatted.contains("调用栈:"), "{formatted}");
        assert!(
            formatted.contains("bad: 期望 2 , 实际 1 @ test.rhai:2:5"),
            "{formatted}"
        );
        assert!(
            formatted.contains("1. 函数 'inner' @ test.rhai:6:5"),
            "{formatted}"
        );
        assert!(
            formatted.contains("2. 闭包调用 @ test.rhai:5:1"),
            "{formatted}"
        );
        assert!(!formatted.contains("(from "), "{formatted}");
    }

}
