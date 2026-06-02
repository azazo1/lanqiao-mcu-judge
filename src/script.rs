use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow};
use rhai::{Dynamic, Engine, EvalAltResult, ImmutableString, Scope};

use crate::machine::Simulator;

pub fn run_script(sim: Simulator, path: &Path) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("读取脚本失败: {}", path.display()))?;
    let shared = Arc::new(Mutex::new(sim));
    let mut engine = Engine::new();
    register_api(&mut engine, &shared);
    let mut scope = Scope::new();
    let _ = engine
        .eval_with_scope::<Dynamic>(&mut scope, &source)
        .map_err(|err| anyhow!(err.to_string()))?;
    Ok(())
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

    let sim_count_changes = Arc::clone(sim);
    engine.register_fn(
        "count_line_changes",
        move |name: ImmutableString, duration_ms: i64| -> Result<i64, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let changes = sim_count_changes
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .count_line_changes(name.as_str(), duration_ms)
                .map_err(|err| runtime_error(err.to_string()))?;
            i64::try_from(changes).map_err(|_| runtime_error("线路变化次数超出脚本整数范围"))
        },
    );

    let sim_change_frequency = Arc::clone(sim);
    engine.register_fn(
        "line_change_frequency_hz",
        move |name: ImmutableString, duration_ms: i64| -> Result<rhai::FLOAT, Box<EvalAltResult>> {
            let duration_ms = u64::try_from(duration_ms)
                .map_err(|_| runtime_error("duration_ms 参数必须 >= 0"))?;
            let frequency = sim_change_frequency
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?
                .line_change_frequency_hz(name.as_str(), duration_ms)
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
