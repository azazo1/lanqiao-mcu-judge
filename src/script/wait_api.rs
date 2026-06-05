use std::sync::{Arc, Mutex};

use rhai::{Engine, EvalAltResult, ImmutableString, Map};

use crate::{
    chip::{ObservedEvent, Simulator},
    script::{
        event_track::EventTrack,
        run_target::RunToTarget,
        state_target::{BoolStateTarget, IntStateTarget, TextStateTarget},
    },
};

use super::{runtime_error, script_duration_ns, script_int};

pub(super) fn register_wait_api(engine: &mut Engine, sim: &Arc<Mutex<Simulator>>) {
    let sim_bool_target = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: RunToTarget, expected: bool| -> Result<i64, Box<EvalAltResult>> {
            let mut sim = sim_bool_target
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_bool_state_with_timeout(BoolStateTarget::Signal(target), expected, None)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_bool_target_timeout = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: RunToTarget,
              expected: bool,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            let timeout_ns = Some(script_duration_ns(timeout_ns, "timeout_ns")?);
            let mut sim = sim_bool_target_timeout
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_bool_state_with_timeout(
                    BoolStateTarget::Signal(target),
                    expected,
                    timeout_ns,
                )
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_bool = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString, expected: bool| -> Result<i64, Box<EvalAltResult>> {
            let target = BoolStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let mut sim = sim_name_bool
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_bool_state_with_timeout(target, expected, None)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_bool_timeout = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString,
              expected: bool,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            let target = BoolStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let timeout_ns = Some(script_duration_ns(timeout_ns, "timeout_ns")?);
            let mut sim = sim_name_bool_timeout
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_bool_state_with_timeout(target, expected, timeout_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_int = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString, expected: i64| -> Result<i64, Box<EvalAltResult>> {
            let target = IntStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let mut sim = sim_name_int
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_int_state_with_timeout(target, expected, None)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_int_timeout = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString,
              expected: i64,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            let target = IntStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let timeout_ns = Some(script_duration_ns(timeout_ns, "timeout_ns")?);
            let mut sim = sim_name_int_timeout
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_int_state_with_timeout(target, expected, timeout_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_text = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString,
              expected: ImmutableString|
              -> Result<i64, Box<EvalAltResult>> {
            let target = TextStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let mut sim = sim_name_text
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_text_state_with_timeout(target, expected.as_str(), None)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_name_text_timeout = Arc::clone(sim);
    engine.register_fn(
        "wait_until",
        move |target: ImmutableString,
              expected: ImmutableString,
              timeout_ns: i64|
              -> Result<i64, Box<EvalAltResult>> {
            let target = TextStateTarget::parse(target.as_str())
                .map_err(|err| runtime_error(err.to_string()))?;
            let timeout_ns = Some(script_duration_ns(timeout_ns, "timeout_ns")?);
            let mut sim = sim_name_text_timeout
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let elapsed_ns = sim
                .wait_until_text_state_with_timeout(target, expected.as_str(), timeout_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            script_int(elapsed_ns, "wait_until 返回值超出脚本整数范围")
        },
    );

    let sim_event = Arc::clone(sim);
    engine.register_fn(
        "run_to_event",
        move |track: ImmutableString| -> Result<Map, Box<EvalAltResult>> {
            let track =
                EventTrack::parse(track.as_str()).map_err(|err| runtime_error(err.to_string()))?;
            let mut sim = sim_event
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let event = sim
                .run_to_event_with_timeout(track, None)
                .map_err(|err| runtime_error(err.to_string()))?;
            observed_event_map(event)
        },
    );

    let sim_event_timeout = Arc::clone(sim);
    engine.register_fn(
        "run_to_event",
        move |track: ImmutableString, timeout_ns: i64| -> Result<Map, Box<EvalAltResult>> {
            let track =
                EventTrack::parse(track.as_str()).map_err(|err| runtime_error(err.to_string()))?;
            let timeout_ns = Some(script_duration_ns(timeout_ns, "timeout_ns")?);
            let mut sim = sim_event_timeout
                .lock()
                .map_err(|_| runtime_error("仿真器锁已损坏"))?;
            let event = sim
                .run_to_event_with_timeout(track, timeout_ns)
                .map_err(|err| runtime_error(err.to_string()))?;
            observed_event_map(event)
        },
    );
}

fn observed_event_map(event: ObservedEvent) -> Result<Map, Box<EvalAltResult>> {
    let mut map = Map::new();
    map.insert("track".into(), event.track_id.into());
    map.insert(
        "time_ns".into(),
        script_int(event.time_ns, "event.time_ns 超出脚本整数范围")?.into(),
    );
    map.insert(
        "elapsed_ns".into(),
        script_int(event.elapsed_ns, "event.elapsed_ns 超出脚本整数范围")?.into(),
    );
    map.insert("label".into(), event.label.into());
    map.insert("detail".into(), event.detail.unwrap_or_default().into());
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::super::eval_source;
    use crate::{chip::Simulator, wave::WaveCaptureOptions};

    fn sample_hex_path(sample: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("sample")
            .join(sample)
            .join("prj")
            .join("Objects")
            .join(format!("{sample}.hex"))
    }

    fn dual_uart_echo_sim() -> Simulator {
        let code = vec![
            0x75, 0x98, 0x10, 0x75, 0x9A, 0x10, 0xE5, 0x98, 0x54, 0x01, 0x60, 0x07, 0xE5, 0x99,
            0x53, 0x98, 0xFE, 0xF5, 0x99, 0xE5, 0x9A, 0x54, 0x01, 0x60, 0x07, 0xE5, 0x9B, 0x53,
            0x9A, 0xFE, 0xF5, 0x9B, 0x80, 0xE4,
        ];
        Simulator::from_code_with_options(code, false, WaveCaptureOptions::default())
    }

    #[test]
    fn rhai_wait_until_supports_bool_state_targets() {
        let sim = Simulator::nop(false);
        let script = r#"
            let dt_latch = wait_until("board.effective.ctrl", 0x70, 1_000);
            assert_eq(dt_latch, 0, "上电控制锁存器应直接处于默认值");

            set_frequency_hz(2000);
            jumper_on(NET_SIG, SIG_OUT);

            let dt0 = wait_until("pin.p3.4", true, 2_000_000);
            assert_in(dt0, 0..=2_000_000, "SIG_OUT 应在超时内拉高");

            let dt1 = wait_until("SIG_OUT", false, 2_000_000);
            assert_in(dt1, 0..=2_000_000, "SIG_OUT 应在超时内再次翻到低电平");
        "#;
        eval_source(sim, "test:wait_until_bool_targets", script).expect("run bool wait script");
    }

    #[test]
    fn rhai_wait_until_supports_seg_text_and_pattern_targets() {
        let sim = Simulator::from_hex_path_with_options(
            &sample_hex_path("key_seg"),
            false,
            WaveCaptureOptions::default(),
        )
        .expect("load key_seg sample");
        let script = r#"
            let dt0 = wait_until("seg.text", "       0", 300_000_000);
            assert_in(dt0, 0..=300_000_000, "上电后整屏文本应显示 0");

            let dt1 = wait_until("seg.d8.visible", true, 20_000_000);
            assert_in(dt1, 0..=20_000_000, "上电后 D8 应处于可见状态");

            let dt2 = wait_until("seg.d8.text", "0", 20_000_000);
            assert_in(dt2, 0..=20_000_000, "上电后 D8 文本应显示 0");

            // S4: 按下后最低位应切到 1.
            set_key(S4, true);

            let dt3 = wait_until("seg.d8.visible", true, 100_000_000);
            assert_in(dt3, 0..=100_000_000, "S4 按下后 D8 应回到可见状态");

            let dt4 = wait_until("seg.text", "       1", 300_000_000);
            assert_in(dt4, 0..=300_000_000, "S4 按下后整屏文本应显示 1");

            let dt5 = wait_until("seg.d8.text", "1", 20_000_000);
            assert_in(dt5, 0..=20_000_000, "S4 按下后 D8 文本应显示 1");

            // S4: 释放后显示应恢复为 0.
            set_key(S4, false);

            let dt6 = wait_until("seg.d8.visible", true, 100_000_000);
            assert_in(dt6, 0..=100_000_000, "S4 释放后 D8 应回到可见状态");

            let dt7 = wait_until("seg.text", "       0", 300_000_000);
            assert_in(dt7, 0..=300_000_000, "S4 释放后整屏文本应恢复 0");
        "#;
        eval_source(sim, "test:wait_until_seg_targets", script).expect("run seg wait script");
    }

    #[test]
    fn rhai_run_to_event_supports_wave_event_tracks_without_wave_export() {
        let sim = dual_uart_echo_sim();
        let script = r#"
            uart1_config(8, 9600, 1, "none");
            uart_write("A");

            let event = run_to_event("uart1", 20_000_000);
            assert_eq(event.track, "event.uart1", "事件轨应返回规范 id");
            assert_regex(event.label, "^(RX|TX) 0x41( 'A')?$", "UART1 事件标签");
            assert_eq(event.detail, "bits=8", "UART1 事件细节");
            assert_in(event.elapsed_ns, 0..=20_000_000, "UART1 事件应在超时内出现");
        "#;
        eval_source(sim, "test:run_to_event_tracks", script).expect("run event wait script");
    }
}
