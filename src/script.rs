use std::{
    io::{self, BufRead, IsTerminal, Read, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use rhai::{
    Dynamic, Engine, EvalAltResult, ImmutableString, Position, Scope,
    debugger::{DebuggerCommand, DebuggerEvent},
};
use tracing::{debug, trace};

use crate::{
    ids::{KeyId, KeyMode, LedId, SignalId, VoltageChannel},
    machine::Simulator,
};

pub fn run_script(sim: Simulator, path: &Path) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("读取脚本失败: {}", path.display()))?;
    eval_source(sim, &format!("file:{}", path.display()), &source)
}

pub fn run_script_stdin(sim: Simulator) -> Result<()> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("读取标准输入脚本失败")?;
    if source.trim().is_empty() {
        bail!("标准输入中没有 Rhai 脚本内容");
    }
    eval_source(sim, "stdin", &source)
}

pub fn run_repl(sim: Simulator) -> Result<()> {
    let shared = Arc::new(Mutex::new(sim));
    let trace_state = Arc::new(Mutex::new(ScriptTraceState::default()));
    let engine = build_engine(&shared, &trace_state);
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

        if reader
            .read_line(&mut line)
            .context("读取 REPL 输入失败")?
            == 0
        {
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

        debug!(line_no, statement, "执行 REPL 语句");
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
    let engine = build_engine(&shared, &trace_state);
    let mut scope = build_scope();
    debug!(label, lines = source.lines().count(), "开始执行评测脚本");
    eval_source_with_engine(&engine, &mut scope, &trace_state, label, source)?;
    Ok(())
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

fn build_engine(sim: &Arc<Mutex<Simulator>>, trace_state: &Arc<Mutex<ScriptTraceState>>) -> Engine {
    let mut engine = Engine::new();
    engine.on_print(|text| println!("{text}"));
    register_script_progress_debugger(&mut engine, trace_state);
    engine.register_type_with_name::<LedId>("Led");
    engine.register_type_with_name::<KeyId>("Key");
    engine.register_type_with_name::<KeyMode>("KeyMode");
    engine.register_type_with_name::<VoltageChannel>("VoltageChannel");
    engine.register_type_with_name::<SignalId>("Signal");
    register_api(&mut engine, sim);
    engine
}

#[derive(Debug, Default)]
struct ScriptTraceState {
    label: String,
    lines: Vec<String>,
    step: u64,
}

fn update_script_trace_state(trace_state: &Arc<Mutex<ScriptTraceState>>, label: &str, source: &str) {
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
    lines.get(line_no.saturating_sub(1))
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

fn register_script_progress_debugger(
    engine: &mut Engine,
    trace_state: &Arc<Mutex<ScriptTraceState>>,
) {
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

            match event {
                DebuggerEvent::Start | DebuggerEvent::Step | DebuggerEvent::BreakPoint(_) => {
                    debug!(
                        target: "script_progress",
                        label,
                        event = script_event_name(event),
                        step,
                        line = pos.line().unwrap_or(0),
                        column = pos.position().unwrap_or(0),
                        call_level = context.call_level(),
                        snippet,
                        is_stmt = node.is_stmt(),
                        "执行评测脚本语句"
                    );
                    Ok(DebuggerCommand::Next)
                }
                DebuggerEvent::FunctionExitWithValue(_) | DebuggerEvent::FunctionExitWithError(_) => {
                    trace!(
                        target: "script_progress",
                        label,
                        event = script_event_name(event),
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
        ("RB3", VoltageChannel::Rb3),
        ("RB4", VoltageChannel::Rb4),
        ("RD1", VoltageChannel::Rd1),
    ] {
        scope.push_constant(name, channel);
    }
    scope.push_constant("KEYBOARD", KeyMode::Keyboard);
    scope.push_constant("KBD", KeyMode::Keyboard);
    scope.push_constant("BUTTON", KeyMode::Button);
    scope.push_constant("BTN", KeyMode::Button);
    scope.push_constant("SIG_OUT", SignalId::SigOut);
    scope.push_constant("NET_SIG", SignalId::NetSig);
    scope
}

fn register_api(engine: &mut Engine, sim: &Arc<Mutex<Simulator>>) {
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
            let mode = KeyMode::parse(mode.as_str()).map_err(|err| runtime_error(err.to_string()))?;
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
        move |channel: VoltageChannel,
              voltage: rhai::FLOAT|
              -> Result<(), Box<EvalAltResult>> {
            sim_voltage_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .set_voltage_channel(channel, voltage as f32);
            Ok(())
        },
    );

    let sim_uart = Arc::clone(sim);
    engine.register_fn(
        "uart_write",
        move |text: ImmutableString| -> Result<(), Box<EvalAltResult>> {
            sim_uart
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .uart_write(text.as_bytes());
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
                .uart_take_string();
            Ok(text.into())
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
        move || -> Result<i64, Box<EvalAltResult>> {
            sim_display_number
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .display_number()
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_display_number_window = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |duration_ms: i64| -> Result<i64, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            sim_display_number_window
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .observe_display_number(duration_ms)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_display_number_range = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |start: i64, end: i64| -> Result<i64, Box<EvalAltResult>> {
            let start =
                usize::try_from(start).map_err(|_| runtime_error("start 参数必须 >= 0"))?;
            let end = usize::try_from(end).map_err(|_| runtime_error("end 参数必须 >= 0"))?;
            sim_display_number_range
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .display_number_in_range(start, end)
                .map_err(|err| runtime_error(err.to_string()))
        },
    );

    let sim_display_number_range_window = Arc::clone(sim);
    engine.register_fn(
        "display_number",
        move |start: i64, end: i64, duration_ms: i64| -> Result<i64, Box<EvalAltResult>> {
            let start =
                usize::try_from(start).map_err(|_| runtime_error("start 参数必须 >= 0"))?;
            let end = usize::try_from(end).map_err(|_| runtime_error("end 参数必须 >= 0"))?;
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            sim_display_number_range_window
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .observe_display_number_in_range(start, end, duration_ms)
                .map_err(|err| runtime_error(err.to_string()))
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
    engine.register_fn("led_on", move |led: LedId| -> Result<bool, Box<EvalAltResult>> {
        let value = sim_led_id
            .lock()
            .map_err(|_| runtime_error("仿真器锁已损坏"))?
            .led_on_id(led);
        Ok(value)
    });

    let sim_watch_led = Arc::clone(sim);
    engine.register_fn(
        "watch_led_changes",
        move |name: ImmutableString, duration_ms: i64| -> Result<i64, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let changes = sim_watch_led
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_changes_named(name.as_str(), duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            i64::try_from(changes).map_err(|_| runtime_error("LED 变化次数超出脚本整数范围"))
        },
    );

    let sim_watch_led_id = Arc::clone(sim);
    engine.register_fn(
        "watch_led_changes",
        move |led: LedId, duration_ms: i64| -> Result<i64, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let changes = sim_watch_led_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_changes(led, duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            i64::try_from(changes).map_err(|_| runtime_error("LED 变化次数超出脚本整数范围"))
        },
    );

    let sim_watch_led_frequency = Arc::clone(sim);
    engine.register_fn(
        "watch_led_frequency_hz",
        move |name: ImmutableString, duration_ms: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let frequency = sim_watch_led_frequency
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_frequency_hz_named(name.as_str(), duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(frequency)
        },
    );

    let sim_watch_led_frequency_id = Arc::clone(sim);
    engine.register_fn(
        "watch_led_frequency_hz",
        move |led: LedId, duration_ms: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let frequency = sim_watch_led_frequency_id
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .watch_led_frequency_hz(led, duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            Ok(frequency)
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
        "assert_eq_str",
        move |actual: ImmutableString,
              expected: ImmutableString,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            if actual == expected {
                return Ok(());
            }
            Err(runtime_error(format!(
                "{label}: 期望 `{expected}`, 实际 `{actual}`"
            )))
        },
    );

    engine.register_fn(
        "assert_eq_int",
        move |actual: i64,
              expected: i64,
              label: ImmutableString|
              -> Result<(), Box<EvalAltResult>> {
            if actual == expected {
                return Ok(());
            }
            Err(runtime_error(format!(
                "{label}: 期望 {expected}, 实际 {actual}"
            )))
        },
    );
}

fn runtime_error(message: impl Into<String>) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(message.into().into(), rhai::Position::NONE).into()
}
