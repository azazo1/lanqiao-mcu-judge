use std::{cell::Cell, collections::VecDeque, fmt::Write as _, fs, path::Path, rc::Rc, sync::Arc};

use anyhow::{Context, Result, bail};
use i8051::{
    Cpu, CpuContext, CpuView, Flag, Interrupt as CpuInterrupt, MemoryMapper, PortMapper,
    ReadOnlyMemoryMapper, Register,
};
use tracing::{trace, warn};

pub const SYSTEM_HZ: u64 = 12_000_000;
pub const CPU_EXEC_HZ: u64 = 12_000_000;
pub const NS_PER_SECOND: u64 = 1_000_000_000;
pub const NS_PER_MILLISECOND: u64 = 1_000_000;
pub const NS_PER_MICROSECOND: u64 = 1_000;
const BOARD_POWER_ON_LATCHES: [u8; 4] = [0x00, 0x70, 0x00, 0x00];
const INTERRUPT_ENTRY_TICKS: u32 = 3;
const UART_INTERRUPT_REASSERT_INSTRUCTIONS: u8 = 2;
const WAVE_KEY_ORDER: [KeyId; 16] = [
    KeyId::S4,
    KeyId::S5,
    KeyId::S6,
    KeyId::S7,
    KeyId::S8,
    KeyId::S9,
    KeyId::S10,
    KeyId::S11,
    KeyId::S12,
    KeyId::S13,
    KeyId::S14,
    KeyId::S15,
    KeyId::S16,
    KeyId::S17,
    KeyId::S18,
    KeyId::S19,
];

use crate::{
    event::{
        gate::{EventGate, SharedEventGate},
        i2c::I2cEventDecoder,
        seg::SegEventDetector,
        track::EventTrack,
        tracker::EventTracker,
    },
    hex::load_ihex,
    ids::{KeyId, KeyMode, LedId, ResetMode, SignalId, VoltageChannel},
    jumper::{BoardJumpers, LineDrive, resolve_line},
    peripherals::{
        AnalogInputs, At24c02, Ds18b20, Ds1302, Ds1302State, I2cBus, Key, KeyboardLows, Ne555,
        Outputs, Pcf8591, SegmentDecoder, SignalTransitionIter, UltrasonicDevice,
    },
    persistent_state::PersistentState,
    script::{
        run_target::{RunToEdge, RunToTarget},
        state_target::{BoardLatchSource, BoolStateTarget, IntStateTarget, TextStateTarget},
    },
    wave::{
        TRACK_EVENT_CPU, WaveCaptureOptions, WaveCaptureWindow, WaveEventNote, WaveMarkerNote,
        WaveRecorder, WaveSnapshot,
    },
};

mod registers;
mod timers;

use registers::*;
use timers::TimerBlock;

pub struct Simulator {
    cpu: Cpu,
    ctx: MachineContext,
    code_image: Vec<u8>,
    trace_cpu: bool,
    interrupt_poll_blocked_instructions: u8,
    active_interrupts: Vec<InterruptSource>,
    seg_decoder: SegmentDecoder,
    wave: WaveRecorder,
    event_gate: SharedEventGate,
    i2c_event_decoder: I2cEventDecoder,
    seg_event_detector: SegEventDetector,
    script_event_tracker: EventTracker,
}

#[derive(Debug, Clone)]
struct BoardRetainedState {
    persistent_state: PersistentState,
    keys: Key,
    key_mode: KeyMode,
    analog: AnalogInputs,
    jumpers: BoardJumpers,
    ds18b20_temperature_c: f32,
    ds18b20_parasite_power: bool,
    ultrasonic_distance_cm: f32,
    ne555_frequency_hz: f32,
}

#[derive(Debug, Clone, Copy)]
enum InterruptSource {
    External0,
    Timer0,
    External1,
    Timer1,
    Serial,
    Serial2,
}

#[derive(Debug, Clone, Copy)]
struct PendingInterrupt {
    source: InterruptSource,
    tcon_clear_mask: u8,
}

impl PendingInterrupt {
    fn cpu_interrupt(self) -> CpuInterrupt {
        match self.source {
            InterruptSource::External0 => CpuInterrupt::External0,
            InterruptSource::Timer0 => CpuInterrupt::Timer0,
            InterruptSource::External1 => CpuInterrupt::External1,
            InterruptSource::Timer1 => CpuInterrupt::Timer1,
            InterruptSource::Serial | InterruptSource::Serial2 => CpuInterrupt::Serial,
        }
    }

    fn vector_addr(self) -> Option<u16> {
        match self.source {
            InterruptSource::Serial2 => Some(0x0043),
            _ => None,
        }
    }
}

fn enter_cpu_interrupt(cpu: &mut Cpu, pending: PendingInterrupt) -> bool {
    if !matches!(pending.source, InterruptSource::Serial2) {
        return cpu.interrupt(pending.cpu_interrupt());
    }

    let ie = cpu.register(Register::IE) as u8;
    if ie & IE_ES != 0 {
        return cpu.interrupt(CpuInterrupt::Serial);
    }

    cpu.register_set(Register::IE, u16::from(ie | IE_ES));
    let entered = cpu.interrupt(CpuInterrupt::Serial);
    cpu.register_set(Register::IE, u16::from(ie));
    entered
}

const LED_CHANGE_INTERVAL_MAX_RELATIVE_DEVIATION: f64 = 0.25;

#[derive(Debug, Clone, Default)]
pub(crate) struct LedWatchStats {
    pub(crate) on_time_ns: u64,
    pub(crate) observed_time_ns: u64,
    pub(crate) changes: u64,
    pub(crate) rising_edges: u64,
    pub(crate) change_intervals_ns: Vec<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ObservedEvent {
    pub(crate) track_id: &'static str,
    pub(crate) time_ns: u64,
    pub(crate) elapsed_ns: u64,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
}

impl LedWatchStats {
    fn ensure_positive_observed_time(&self) -> Result<()> {
        if self.observed_time_ns == 0 {
            bail!("统计时长必须 > 0");
        }
        Ok(())
    }

    fn stable_change_interval_ns(&self) -> Result<Option<f64>> {
        self.ensure_positive_observed_time()?;
        if self.change_intervals_ns.is_empty() {
            return Ok(None);
        }
        let total_interval_ns: u128 = self
            .change_intervals_ns
            .iter()
            .map(|&it| u128::from(it))
            .sum();
        if total_interval_ns == 0 {
            return Ok(None);
        }
        let mean_interval_ns = total_interval_ns as f64 / self.change_intervals_ns.len() as f64;
        for &interval_ns in &self.change_intervals_ns {
            let relative_deviation =
                (interval_ns as f64 - mean_interval_ns).abs() / mean_interval_ns;
            if relative_deviation > LED_CHANGE_INTERVAL_MAX_RELATIVE_DEVIATION {
                return Ok(None);
            }
        }
        Ok(Some(mean_interval_ns))
    }

    pub(crate) fn change_frequency_hz(&self) -> Result<Option<f64>> {
        self.ensure_positive_observed_time()?;
        if self.changes == 0 {
            return Ok(Some(0.0));
        }
        let Some(mean_interval_ns) = self.stable_change_interval_ns()? else {
            return Ok(None);
        };
        Ok(Some(NS_PER_SECOND as f64 / mean_interval_ns))
    }

    pub(crate) fn pwm_frequency_hz(&self) -> Result<f64> {
        self.ensure_positive_observed_time()?;
        Ok(self.rising_edges as f64 * NS_PER_SECOND as f64 / self.observed_time_ns as f64)
    }

    pub(crate) fn duty_percent(&self) -> Result<f64> {
        self.ensure_positive_observed_time()?;
        Ok(self.on_time_ns as f64 * 100.0 / self.observed_time_ns as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayNumber {
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Clone)]
pub struct BoardSnapshot {
    pub sim_time_ns: u64,
    pub cpu_cycles: u64,
    pub pc: u16,
    pub display_text: String,
    pub seg_chars: [char; 8],
    pub seg_raw: [u8; 8],
    pub led_states: [bool; 8],
    pub key_states: [bool; 16],
    pub key_mode: KeyMode,
    pub relay_on: bool,
    pub motor_on: bool,
    pub buzzer_on: bool,
    pub port_latch: [u8; 6],
    pub port_input: [u8; 6],
    pub board_latches_effective: [u8; 4],
    pub board_latches_port: [u8; 4],
    pub board_latches_xdata: [u8; 4],
    pub analog_rd1_v: f32,
    pub analog_rb2_v: f32,
    pub adc_code: u8,
    pub adc_channel: u8,
    pub adc_channel_voltage_v: f32,
    pub dac_code: u8,
    pub dac_voltage_v: f32,
    pub ne555_level: bool,
    pub ne555_frequency_hz: f32,
    pub ds18b20_temperature_c: f32,
    pub ultrasonic_distance_cm: f32,
    pub jumper_net_sig_to_sig_out: bool,
    pub uart1_text: String,
    pub uart1_text_error: Option<String>,
    pub uart1_raw: Vec<u16>,
    pub uart2_text: String,
    pub uart2_text_error: Option<String>,
    pub uart2_raw: Vec<u16>,
}

impl Simulator {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_hex_path(path: &Path, trace_cpu: bool) -> Result<Self> {
        Self::from_hex_path_with_options(path, trace_cpu, WaveCaptureOptions::default())
    }

    pub fn from_code_with_options(
        code: Vec<u8>,
        trace_cpu: bool,
        wave_options: WaveCaptureOptions,
    ) -> Self {
        let wave_window = wave_options.window();
        let event_gate = EventGate::shared(wave_window);
        let ctx = MachineContext::new_with_event_gate(code.clone(), Arc::clone(&event_gate));
        let mut sim = Self {
            cpu: Cpu::new(),
            ctx,
            code_image: code,
            trace_cpu,
            interrupt_poll_blocked_instructions: 0,
            active_interrupts: Vec::new(),
            seg_decoder: SegmentDecoder::default(),
            wave: WaveRecorder::new(wave_options),
            event_gate,
            i2c_event_decoder: I2cEventDecoder::default(),
            seg_event_detector: SegEventDetector::default(),
            script_event_tracker: EventTracker::default(),
        };
        sim.ctx.ports.sync_inputs(&sim.ctx.board);
        sim.capture_wave_snapshot();
        sim
    }

    pub fn nop_with_options(trace_cpu: bool, wave_options: WaveCaptureOptions) -> Self {
        Self::from_code_with_options(vec![0x00], trace_cpu, wave_options)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn nop(trace_cpu: bool) -> Self {
        Self::nop_with_options(trace_cpu, WaveCaptureOptions::default())
    }

    pub fn from_hex_path_with_options(
        path: &Path,
        trace_cpu: bool,
        wave_options: WaveCaptureOptions,
    ) -> Result<Self> {
        let hex = fs::read_to_string(path)
            .with_context(|| format!("读取 HEX 文件失败: {}", path.display()))?;
        let code = load_ihex(&hex)?;
        Ok(Self::from_code_with_options(code, trace_cpu, wave_options))
    }

    pub fn export_persistent_state(&self) -> String {
        self.ctx.board.persistent_state().encode()
    }

    pub fn load_persistent_state(&mut self, text: &str) -> Result<()> {
        let state = PersistentState::decode(text)?;
        self.ctx.board.load_persistent_state(&state);
        self.ctx.ports.sync_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        self.reset_with_mode(ResetMode::Power)
    }

    pub fn reset_with_mode(&mut self, mode: ResetMode) -> Result<()> {
        match mode {
            ResetMode::Cpu => self.reset_cpu_only(),
            ResetMode::Power => self.reset_power_cycle(),
        }
    }

    fn reset_cpu_only(&mut self) -> Result<()> {
        let board = std::mem::take(&mut self.ctx.board);
        let port_latches = LatchedBoardState::from_ports(&self.ctx.ports);
        let xdata_latches = LatchedBoardState::from_xdata(&self.ctx.xdata);
        self.cpu = Cpu::new();
        self.interrupt_poll_blocked_instructions = 0;
        self.active_interrupts.clear();
        self.i2c_event_decoder.reset();
        self.seg_event_detector.reset();
        self.ctx = MachineContext::new_with_event_gate(
            self.code_image.clone(),
            Arc::clone(&self.event_gate),
        );
        self.ctx.board = board;
        port_latches.apply_to_ports(&mut self.ctx.ports);
        xdata_latches.apply_to_xdata(&mut self.ctx.xdata);
        self.ctx.ports.sync_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        Ok(())
    }

    fn reset_power_cycle(&mut self) -> Result<()> {
        let retained = self.ctx.board.retained_state();
        let retained_sim_time_ns = self.ctx.board.sim_time_ns;
        let retained_sim_time_ns_remainder = self.ctx.board.sim_time_ns_remainder;
        let retained_system_cycle_remainder = self.ctx.board.system_cycle_remainder;
        self.cpu = Cpu::new();
        self.interrupt_poll_blocked_instructions = 0;
        self.active_interrupts.clear();
        self.i2c_event_decoder.reset();
        self.seg_event_detector.reset();
        self.ctx = MachineContext::new_with_event_gate(
            self.code_image.clone(),
            Arc::clone(&self.event_gate),
        );
        self.ctx.board.sim_time_ns = retained_sim_time_ns;
        self.ctx.board.sim_time_ns_remainder = retained_sim_time_ns_remainder;
        self.ctx.board.system_cycle_remainder = retained_system_cycle_remainder;
        self.ctx.board.load_retained_state(&retained);
        self.ctx.ports.sync_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        Ok(())
    }

    pub fn run_ms(&mut self, ms: u64) -> Result<()> {
        self.run_us(ms.saturating_mul(1_000))
    }

    pub fn run_us(&mut self, us: u64) -> Result<()> {
        let target = self
            .ctx
            .board
            .sim_time_ns
            .saturating_add(us.saturating_mul(NS_PER_MICROSECOND));
        while self.ctx.board.sim_time_ns < target {
            self.step_once()?;
        }
        Ok(())
    }

    pub fn run_to_ns(&mut self, target_ns: u64) -> Result<u64> {
        let start = self.ctx.board.sim_time_ns;
        if target_ns < start {
            bail!("run_to_ns 目标时间戳早于当前时间: target_ns={target_ns}, current_ns={start}");
        }
        while self.ctx.board.sim_time_ns < target_ns {
            self.step_once()?;
        }
        Ok(self.ctx.board.sim_time_ns.saturating_sub(start))
    }

    pub fn step(&mut self) -> Result<()> {
        self.step_once()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn run_to_target(&mut self, target: RunToTarget, edge: RunToEdge) -> Result<u64> {
        self.run_to_target_with_timeout(target, edge, None)
    }

    pub fn run_to_target_with_timeout(
        &mut self,
        target: RunToTarget,
        edge: RunToEdge,
        timeout_ns: Option<u64>,
    ) -> Result<u64> {
        let start = self.ctx.board.sim_time_ns;
        let mut previous = self.read_run_to_target(target);
        loop {
            let elapsed_ns = self.ctx.board.sim_time_ns.saturating_sub(start);
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns >= timeout_ns
            {
                bail!("run_to 等待超时: timeout_ns={timeout_ns}");
            }
            self.step_once()?;
            let current = self.read_run_to_target(target);
            let elapsed_ns = self.ctx.board.sim_time_ns.saturating_sub(start);
            let matched = match edge {
                RunToEdge::Up => !previous && current,
                RunToEdge::Down => previous && !current,
                RunToEdge::Flip => previous != current,
            };
            if matched {
                if let Some(timeout_ns) = timeout_ns
                    && elapsed_ns > timeout_ns
                {
                    bail!("run_to 等待超时: timeout_ns={timeout_ns}");
                }
                return Ok(elapsed_ns);
            }
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns >= timeout_ns
            {
                bail!("run_to 等待超时: timeout_ns={timeout_ns}");
            }
            previous = current;
        }
    }

    pub fn sim_time_ns(&self) -> u64 {
        self.ctx.board.sim_time_ns
    }

    pub fn add_wave_marker(&mut self, label: Option<&str>) {
        self.add_wave_marker_at(self.sim_time_ns(), label);
    }

    pub fn add_wave_marker_at(&mut self, time_ns: u64, label: Option<&str>) {
        let note = match label {
            Some(label) => WaveMarkerNote::named(time_ns, label),
            None => WaveMarkerNote::anonymous(time_ns),
        };
        self.wave.record_marker_note(note);
    }

    #[cfg(test)]
    pub(crate) fn recorded_wave_markers(&self) -> Vec<(u64, Option<String>)> {
        self.wave.marker_records()
    }

    pub fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.ctx.board.set_key(name, pressed)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn set_key_id(&mut self, key: KeyId, pressed: bool) {
        self.ctx.board.keys.set_key_id(key, pressed);
        self.capture_control_snapshot();
    }

    pub fn key_mode(&mut self, mode: KeyMode) {
        self.ctx.board.key_mode = mode;
        self.capture_control_snapshot();
    }

    pub fn tap_key(&mut self, name: &str, hold_ms: u64) -> Result<()> {
        self.set_key(name, true)?;
        self.run_ms(hold_ms)?;
        self.set_key(name, false)?;
        self.run_ms(30)?;
        Ok(())
    }

    pub fn tap_key_id(&mut self, key: KeyId, hold_ms: u64) -> Result<()> {
        self.set_key_id(key, true);
        self.run_ms(hold_ms)?;
        self.set_key_id(key, false);
        self.run_ms(30)?;
        Ok(())
    }

    pub fn set_rtc(&mut self, hour: u8, minute: u8, second: u8) -> Result<()> {
        self.set_rtc_state(Ds1302State {
            hour: Some(hour),
            minute: Some(minute),
            second: Some(second),
            ..Ds1302State::default()
        })?;
        Ok(())
    }

    pub fn set_rtc_state(&mut self, state: Ds1302State) -> Result<()> {
        self.ctx.board.ds1302.set_state(state)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn set_temperature_c(&mut self, value: f32) {
        self.ctx.board.ds18b20.temperature_c = value;
        self.capture_control_snapshot();
    }

    pub fn set_ds18b20_rom(&mut self, rom: &str) -> Result<()> {
        self.ctx.board.ds18b20.set_rom_hex(rom)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn set_ds18b20_parasite_power(&mut self, enabled: bool) {
        self.ctx.board.ds18b20.set_parasite_power(enabled);
        self.capture_control_snapshot();
    }

    pub fn set_distance_cm(&mut self, value: f32) {
        self.ctx.board.ultrasonic.distance_cm = value.max(0.0);
        self.capture_control_snapshot();
    }

    pub fn set_frequency_hz(&mut self, value: f32) {
        let now_ns = self.ctx.board.sim_time_ns;
        self.ctx.board.ne555.set_frequency_hz_at(now_ns, value);
        self.capture_control_snapshot();
    }

    pub fn jumper_on(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.ctx.board.jumper_on(left, right)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn jumper_on_named(&mut self, left: &str, right: &str) -> Result<()> {
        self.jumper_on(SignalId::parse(left)?, SignalId::parse(right)?)
    }

    pub fn jumper_off(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.ctx.board.jumper_off(left, right)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn jumper_off_named(&mut self, left: &str, right: &str) -> Result<()> {
        self.jumper_off(SignalId::parse(left)?, SignalId::parse(right)?)
    }

    pub fn jumper_installed(&self, left: SignalId, right: SignalId) -> bool {
        self.ctx.board.jumper_installed(left, right)
    }

    pub fn jumper_installed_named(&self, left: &str, right: &str) -> Result<bool> {
        Ok(self.jumper_installed(SignalId::parse(left)?, SignalId::parse(right)?))
    }

    pub fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.ctx.board.set_voltage(name, value)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn set_voltage_channel(&mut self, channel: VoltageChannel, value: f32) {
        self.ctx.board.analog.set_voltage_channel(channel, value);
        self.capture_control_snapshot();
    }

    pub fn da_value(&self) -> u8 {
        self.ctx.board.pcf8591.dac_value()
    }

    pub fn eeprom_byte(&self, addr: u8) -> u8 {
        self.ctx.board.at24c02.byte(addr)
    }

    pub fn set_eeprom_byte(&mut self, addr: u8, value: u8) {
        self.ctx.board.at24c02.set_byte(addr, value);
        self.capture_control_snapshot();
    }

    pub fn set_eeprom_bytes(&mut self, addr: u8, values: &[u8]) -> Result<()> {
        self.ctx.board.at24c02.set_bytes(addr, values)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn configure_uart1(&mut self, config: UartConfig) -> Result<()> {
        self.ctx.ports.uart1.configure(config)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn configure_uart2(&mut self, config: UartConfig) -> Result<()> {
        self.ctx.ports.uart2.configure(config)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn uart1_write(&mut self, bytes: &[u8]) -> Result<()> {
        self.ctx.ports.uart1.feed_rx_bytes(bytes)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn uart2_write(&mut self, bytes: &[u8]) -> Result<()> {
        self.ctx.ports.uart2.feed_rx_bytes(bytes)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn uart_write(&mut self, bytes: &[u8]) -> Result<()> {
        self.uart1_write(bytes)
    }

    pub fn uart1_write_raw(&mut self, symbols: &[u16]) -> Result<()> {
        self.ctx.ports.uart1.feed_rx_raw(symbols)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn uart2_write_raw(&mut self, symbols: &[u16]) -> Result<()> {
        self.ctx.ports.uart2.feed_rx_raw(symbols)?;
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn uart_write_raw(&mut self, symbols: &[u16]) -> Result<()> {
        self.uart1_write_raw(symbols)
    }

    pub fn uart1_take_string(&mut self) -> Result<String> {
        self.ctx.ports.uart1.take_tx_string()
    }

    pub fn uart2_take_string(&mut self) -> Result<String> {
        self.ctx.ports.uart2.take_tx_string()
    }

    pub fn uart_take_string(&mut self) -> Result<String> {
        self.uart1_take_string()
    }

    pub fn uart1_take_string_segment(&mut self, idle_ms: u64) -> Result<String> {
        self.ctx.ports.uart1.take_tx_string_segment(idle_ms)
    }

    pub fn uart2_take_string_segment(&mut self, idle_ms: u64) -> Result<String> {
        self.ctx.ports.uart2.take_tx_string_segment(idle_ms)
    }

    pub fn uart_take_string_segment(&mut self, idle_ms: u64) -> Result<String> {
        self.uart1_take_string_segment(idle_ms)
    }

    pub fn uart1_take_raw(&mut self) -> Vec<u16> {
        self.ctx.ports.uart1.take_tx_raw()
    }

    pub fn uart2_take_raw(&mut self) -> Vec<u16> {
        self.ctx.ports.uart2.take_tx_raw()
    }

    pub fn uart_take_raw(&mut self) -> Vec<u16> {
        self.uart1_take_raw()
    }

    pub fn uart1_take_raw_segment(&mut self, idle_ms: u64) -> Vec<u16> {
        self.ctx.ports.uart1.take_tx_raw_segment(idle_ms)
    }

    pub fn uart2_take_raw_segment(&mut self, idle_ms: u64) -> Vec<u16> {
        self.ctx.ports.uart2.take_tx_raw_segment(idle_ms)
    }

    pub fn uart_take_raw_segment(&mut self, idle_ms: u64) -> Vec<u16> {
        self.uart1_take_raw_segment(idle_ms)
    }

    pub fn uart1_clear_output(&mut self) {
        self.ctx.ports.uart1.clear_tx_output();
    }

    pub fn uart2_clear_output(&mut self) {
        self.ctx.ports.uart2.clear_tx_output();
    }

    pub fn uart_clear_output(&mut self) {
        self.uart1_clear_output();
    }

    pub fn uart1_peek_string(&self) -> Result<String> {
        self.ctx.ports.uart1.peek_tx_string()
    }

    pub fn uart2_peek_string(&self) -> Result<String> {
        self.ctx.ports.uart2.peek_tx_string()
    }

    pub fn uart_peek_string(&self) -> Result<String> {
        self.uart1_peek_string()
    }

    pub fn uart1_peek_string_segment(&self, idle_ms: u64) -> Result<String> {
        self.ctx.ports.uart1.peek_tx_string_segment(idle_ms)
    }

    pub fn uart2_peek_string_segment(&self, idle_ms: u64) -> Result<String> {
        self.ctx.ports.uart2.peek_tx_string_segment(idle_ms)
    }

    pub fn uart_peek_string_segment(&self, idle_ms: u64) -> Result<String> {
        self.uart1_peek_string_segment(idle_ms)
    }

    pub fn uart1_peek_raw(&self) -> Vec<u16> {
        self.ctx.ports.uart1.peek_tx_raw()
    }

    pub fn uart2_peek_raw(&self) -> Vec<u16> {
        self.ctx.ports.uart2.peek_tx_raw()
    }

    pub fn uart_peek_raw(&self) -> Vec<u16> {
        self.uart1_peek_raw()
    }

    pub fn uart1_peek_raw_segment(&self, idle_ms: u64) -> Vec<u16> {
        self.ctx.ports.uart1.peek_tx_raw_segment(idle_ms)
    }

    pub fn uart2_peek_raw_segment(&self, idle_ms: u64) -> Vec<u16> {
        self.ctx.ports.uart2.peek_tx_raw_segment(idle_ms)
    }

    pub fn uart_peek_raw_segment(&self, idle_ms: u64) -> Vec<u16> {
        self.uart1_peek_raw_segment(idle_ms)
    }

    pub fn peek_iram(&self, addr: u8) -> u8 {
        self.cpu.internal_ram(addr)
    }

    pub fn poke_iram(&mut self, addr: u8, value: u8) {
        self.cpu.internal_ram_write(addr, value);
        self.capture_control_snapshot();
    }

    pub fn peek_sfr(&self, addr: u8) -> Result<u8> {
        if addr < 0x80 {
            bail!("SFR 地址必须在 0x80..=0xFF");
        }
        Ok(self.cpu.sfr(addr, &self.ctx))
    }

    pub fn peek_sfr_latch(&self, addr: u8) -> Result<u8> {
        if addr < 0x80 {
            bail!("SFR 地址必须在 0x80..=0xFF");
        }
        let view = (&self.cpu, &self.ctx);
        Ok(self.ctx.ports.read_latch(&view, addr))
    }

    pub fn poke_sfr(&mut self, addr: u8, value: u8) -> Result<()> {
        if addr < 0x80 {
            bail!("SFR 地址必须在 0x80..=0xFF");
        }
        self.cpu.sfr_set(addr, value, &mut self.ctx);
        self.capture_control_snapshot();
        Ok(())
    }

    pub fn peek_xdata(&self, addr: u16) -> u8 {
        self.ctx.xdata.raw_read(addr)
    }

    pub fn poke_xdata(&mut self, addr: u16, value: u8) {
        self.ctx.xdata.raw_write(addr, value);
        self.capture_control_snapshot();
    }

    pub fn relay_on(&self) -> bool {
        self.ctx.board.outputs.relay_on
    }

    pub fn buzzer_on(&self) -> bool {
        self.ctx.board.outputs.buzzer_on
    }

    pub fn motor_on(&self) -> bool {
        self.ctx.board.outputs.motor_on
    }

    pub fn led_on(&self, index: usize) -> Result<bool> {
        if !(1..=8).contains(&index) {
            bail!("LED 编号必须在 1..=8");
        }
        Ok(self.ctx.board.outputs.leds[index - 1])
    }

    pub fn led_on_id(&self, led: LedId) -> bool {
        self.ctx.board.outputs.leds[led.index() - 1]
    }

    pub fn read_run_to_target(&mut self, target: RunToTarget) -> bool {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        match target {
            RunToTarget::Led(led) => self.led_on_id(led),
            RunToTarget::Key(key) => self.ctx.board.keys.pressed(key),
            RunToTarget::SegDigit(index) => {
                let latches = self.ctx.effective_board_latches();
                latches[2] & (1 << (index - 1)) != 0
            }
            RunToTarget::Pin { port, bit } => self.ctx.ports.port_input[port] & (1 << bit) != 0,
            RunToTarget::I2cMasterScl => self.ctx.ports.port_latch[2] & (1 << 0) != 0,
            RunToTarget::I2cMasterSda => self.ctx.ports.port_latch[2] & (1 << 1) != 0,
            RunToTarget::I2cBusScl => {
                let (scl_high, _) = self.ctx.board.read_i2c_lines(self.ctx.ports.port_latch[2]);
                scl_high
            }
            RunToTarget::I2cBusSda => {
                let (_, sda_high) = self.ctx.board.read_i2c_lines(self.ctx.ports.port_latch[2]);
                sda_high
            }
            RunToTarget::I2cSlaveSclLow => {
                let (scl_low, _) = self
                    .ctx
                    .board
                    .i2c
                    .slave_drives_low(&self.ctx.board.pcf8591, &self.ctx.board.at24c02);
                scl_low
            }
            RunToTarget::I2cSlaveSdaLow => {
                let (_, sda_low) = self
                    .ctx
                    .board
                    .i2c
                    .slave_drives_low(&self.ctx.board.pcf8591, &self.ctx.board.at24c02);
                sda_low
            }
            RunToTarget::OnewireMasterHigh => self.ctx.ports.port_latch[1] & (1 << 4) != 0,
            RunToTarget::OnewireBusHigh => self.ctx.ports.port_input[1] & (1 << 4) != 0,
            RunToTarget::OnewireDeviceLow => self.ctx.board.ds18b20.drive_low,
            RunToTarget::Uart1Tx => self.ctx.ports.uart1.tx_line_high(),
            RunToTarget::Uart1Rx => self.ctx.ports.uart1.rx_line_high(),
            RunToTarget::Uart2Tx => self.ctx.ports.uart2.tx_line_high(),
            RunToTarget::Uart2Rx => self.ctx.ports.uart2.rx_line_high(),
            RunToTarget::Ds1302Ce => self.ctx.ports.port_latch[1] & (1 << 3) != 0,
            RunToTarget::Ds1302Clk => self.ctx.ports.port_latch[1] & (1 << 7) != 0,
            RunToTarget::Ds1302Io => self.ctx.ports.port_input[2] & (1 << 3) != 0,
            RunToTarget::Ne555SigOut => self.ctx.board.frequency_level(),
        }
    }

    fn board_latch_value(&self, source: BoardLatchSource, slot: u8) -> u8 {
        match source {
            BoardLatchSource::Effective => self.ctx.effective_board_latches()[usize::from(slot)],
            BoardLatchSource::Port => self.ctx.ports.board_latches[usize::from(slot)],
            BoardLatchSource::Xdata => self.ctx.xdata.board_latches[usize::from(slot)],
        }
    }

    pub(crate) fn read_bool_state_target(&mut self, target: BoolStateTarget) -> bool {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        match target {
            BoolStateTarget::Signal(target) => self.read_run_to_target(target),
            BoolStateTarget::LatchBit { port, bit } => {
                self.ctx.ports.port_latch[port] & (1 << bit) != 0
            }
            BoolStateTarget::BoardBit { source, slot, bit } => {
                self.board_latch_value(source, slot) & (1 << bit) != 0
            }
            BoolStateTarget::SegVisible { digit } => {
                let index = usize::from(digit);
                if !(1..=8).contains(&index) {
                    return false;
                }
                let sample = self.ctx.board.outputs.digits[index - 1];
                sample.seen && self.seg_decoder.decode_char(sample) != ' '
            }
        }
    }

    pub(crate) fn read_int_state_target(&mut self, target: IntStateTarget) -> Result<i64> {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        match target {
            IntStateTarget::PinByte { port } => Ok(i64::from(self.ctx.ports.port_input[port])),
            IntStateTarget::LatchByte { port } => Ok(i64::from(self.ctx.ports.port_latch[port])),
            IntStateTarget::BoardByte { source, slot } => {
                Ok(i64::from(self.board_latch_value(source, slot)))
            }
            IntStateTarget::SegRaw { digit } => Ok(i64::from(self.seg_raw(usize::from(digit))?)),
            IntStateTarget::SegPattern { digit } => {
                Ok(i64::from(self.seg_pattern(usize::from(digit))?))
            }
        }
    }

    pub(crate) fn read_text_state_target(&mut self, target: TextStateTarget) -> Result<String> {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        match target {
            TextStateTarget::SegText => Ok(self.display_text()),
            TextStateTarget::SegDigitText { digit } => {
                let index = usize::from(digit);
                if !(1..=8).contains(&index) {
                    bail!("数码管编号必须在 1..=8");
                }
                let sample = self.ctx.board.outputs.digits[index - 1];
                Ok(self.seg_decoder.decode_text(sample))
            }
        }
    }

    fn run_to_state_value_with_timeout<T, F>(
        &mut self,
        mut read: F,
        expected: T,
        timeout_ns: Option<u64>,
    ) -> Result<u64>
    where
        T: PartialEq,
        F: FnMut(&mut Self) -> Result<T>,
    {
        let start = self.ctx.board.sim_time_ns;
        if read(self)? == expected {
            return Ok(0);
        }
        loop {
            let elapsed_ns = self.ctx.board.sim_time_ns.saturating_sub(start);
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns >= timeout_ns
            {
                bail!("run_to_state 等待超时: timeout_ns={timeout_ns}");
            }
            self.step_once()?;
            let elapsed_ns = self.ctx.board.sim_time_ns.saturating_sub(start);
            if read(self)? == expected {
                if let Some(timeout_ns) = timeout_ns
                    && elapsed_ns > timeout_ns
                {
                    bail!("run_to_state 等待超时: timeout_ns={timeout_ns}");
                }
                return Ok(elapsed_ns);
            }
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns >= timeout_ns
            {
                bail!("run_to_state 等待超时: timeout_ns={timeout_ns}");
            }
        }
    }

    pub(crate) fn run_to_bool_state_with_timeout(
        &mut self,
        target: BoolStateTarget,
        expected: bool,
        timeout_ns: Option<u64>,
    ) -> Result<u64> {
        self.run_to_state_value_with_timeout(
            |sim| Ok(sim.read_bool_state_target(target)),
            expected,
            timeout_ns,
        )
    }

    pub(crate) fn run_to_int_state_with_timeout(
        &mut self,
        target: IntStateTarget,
        expected: i64,
        timeout_ns: Option<u64>,
    ) -> Result<u64> {
        self.run_to_state_value_with_timeout(
            |sim| sim.read_int_state_target(target),
            expected,
            timeout_ns,
        )
    }

    pub(crate) fn run_to_text_state_with_timeout(
        &mut self,
        target: TextStateTarget,
        expected: &str,
        timeout_ns: Option<u64>,
    ) -> Result<u64> {
        self.run_to_state_value_with_timeout(
            |sim| sim.read_text_state_target(target),
            expected.to_owned(),
            timeout_ns,
        )
    }

    pub(crate) fn run_to_event_with_timeout(
        &mut self,
        track: EventTrack,
        timeout_ns: Option<u64>,
    ) -> Result<ObservedEvent> {
        let _track_guard = self.event_gate.enable_script_track(track);
        let start = self.ctx.board.sim_time_ns;
        let start_count = self.script_event_tracker.count(track);
        loop {
            let elapsed_ns = self.ctx.board.sim_time_ns.saturating_sub(start);
            if self.script_event_tracker.count(track) > start_count {
                let note = self
                    .script_event_tracker
                    .last_note(track)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("事件轨状态损坏: {}", track.track_id()))?;
                if let Some(timeout_ns) = timeout_ns
                    && elapsed_ns > timeout_ns
                {
                    bail!("run_to_event 等待超时: timeout_ns={timeout_ns}");
                }
                return Ok(ObservedEvent {
                    track_id: note.track_id,
                    time_ns: note.time_ns,
                    elapsed_ns,
                    label: note.label,
                    detail: note.detail,
                });
            }
            if let Some(timeout_ns) = timeout_ns
                && elapsed_ns >= timeout_ns
            {
                bail!("run_to_event 等待超时: timeout_ns={timeout_ns}");
            }
            self.step_once()?;
        }
    }

    pub(crate) fn watch_led_stats(
        &mut self,
        led: LedId,
        duration_ms: u64,
    ) -> Result<LedWatchStats> {
        let start = self.ctx.board.sim_time_ns;
        let target = start.saturating_add(duration_ms.saturating_mul(NS_PER_MILLISECOND));
        let mut stats = LedWatchStats::default();
        let mut last_change_time_ns = None;

        while self.ctx.board.sim_time_ns < target {
            let window_start = self.ctx.board.sim_time_ns;
            let was_on = self.led_on_id(led);
            self.step_once()?;
            let window_end = self.ctx.board.sim_time_ns.min(target);
            let is_on = self.led_on_id(led);
            if was_on {
                stats.on_time_ns = stats
                    .on_time_ns
                    .saturating_add(window_end.saturating_sub(window_start));
            }
            if self.ctx.board.sim_time_ns <= target && was_on != is_on {
                let change_time_ns = window_end;
                stats.changes = stats.changes.saturating_add(1);
                if let Some(prev_change_time_ns) = last_change_time_ns {
                    stats
                        .change_intervals_ns
                        .push(change_time_ns.saturating_sub(prev_change_time_ns));
                }
                last_change_time_ns = Some(change_time_ns);
                if !was_on && is_on {
                    stats.rising_edges = stats.rising_edges.saturating_add(1);
                }
            }
        }

        stats.observed_time_ns = target.saturating_sub(start);
        Ok(stats)
    }

    pub fn display_text(&self) -> String {
        self.ctx.board.outputs.display_text(&self.seg_decoder)
    }

    pub fn observe_display_text(&mut self, duration_ms: u64) -> Result<String> {
        if duration_ms == 0 {
            return Ok(self.display_text());
        }

        let stable_ns = duration_ms.saturating_mul(NS_PER_MILLISECOND);
        let start_ns = self.ctx.board.sim_time_ns;
        let mut stable_until_ns = start_ns.saturating_add(stable_ns);
        let mut last_change_ns = self.ctx.board.outputs.last_digits_change_ns();
        let mut text = self.display_text();

        while self.ctx.board.sim_time_ns < stable_until_ns {
            self.step_once()?;
            let current_change_ns = self.ctx.board.outputs.last_digits_change_ns();
            if current_change_ns != last_change_ns {
                last_change_ns = current_change_ns;
                text = self.display_text();
                stable_until_ns = current_change_ns.saturating_add(stable_ns);
            }
        }

        Ok(text)
    }

    pub fn display_number(&self) -> Result<DisplayNumber> {
        parse_display_number(&self.display_text())
    }

    pub fn observe_display_number(&mut self, duration_ms: u64) -> Result<DisplayNumber> {
        let text = self.observe_display_text(duration_ms)?;
        parse_display_number(&text)
    }

    pub fn display_number_in_range(&self, start: usize, end: usize) -> Result<DisplayNumber> {
        let text = self
            .ctx
            .board
            .outputs
            .display_text_in_range(&self.seg_decoder, start, end)?;
        parse_display_number(&text)
    }

    pub fn observe_display_number_in_range(
        &mut self,
        start: usize,
        end: usize,
        duration_ms: u64,
    ) -> Result<DisplayNumber> {
        let _ = self.observe_display_text(duration_ms)?;
        self.display_number_in_range(start, end)
    }

    pub fn set_seg_decode(&mut self, pattern: u8, text: &str) -> Result<()> {
        self.seg_decoder.set_mapping(pattern, text)
    }

    pub fn set_seg_blank(&mut self, pattern: u8) {
        self.seg_decoder.mark_blank(pattern);
    }

    pub fn seg_raw(&self, index: usize) -> Result<u8> {
        self.ctx.board.outputs.seg_raw(index)
    }

    pub fn seg_pattern(&self, index: usize) -> Result<u8> {
        self.ctx.board.outputs.seg_pattern(index)
    }

    pub fn snapshot(&self) -> BoardSnapshot {
        let adc_channel = self.ctx.board.pcf8591.selected_channel();
        let mut seg_chars = [' '; 8];
        let mut seg_raw = [0_u8; 8];
        for (index, digit) in self.ctx.board.outputs.digits.iter().copied().enumerate() {
            seg_chars[index] = self.seg_decoder.decode_char(digit);
            seg_raw[index] = digit.segments;
        }

        let mut key_states = [false; 16];
        for (index, key) in WAVE_KEY_ORDER.into_iter().enumerate() {
            key_states[index] = self.ctx.board.keys.pressed(key);
        }

        let (uart1_text, uart1_text_error) = match self.uart1_peek_string() {
            Ok(text) => (text, None),
            Err(err) => (String::new(), Some(err.to_string())),
        };
        let (uart2_text, uart2_text_error) = match self.uart2_peek_string() {
            Ok(text) => (text, None),
            Err(err) => (String::new(), Some(err.to_string())),
        };

        BoardSnapshot {
            sim_time_ns: self.ctx.board.sim_time_ns,
            cpu_cycles: self.ctx.board.cpu_cycles,
            pc: self.cpu.pc,
            display_text: self.display_text(),
            seg_chars,
            seg_raw,
            led_states: self.ctx.board.outputs.leds,
            key_states,
            key_mode: self.ctx.board.key_mode,
            relay_on: self.ctx.board.outputs.relay_on,
            motor_on: self.ctx.board.outputs.motor_on,
            buzzer_on: self.ctx.board.outputs.buzzer_on,
            port_latch: self.ctx.ports.port_latch,
            port_input: self.ctx.ports.port_input,
            board_latches_effective: self.ctx.effective_board_latches(),
            board_latches_port: self.ctx.ports.board_latches,
            board_latches_xdata: self.ctx.xdata.board_latches,
            analog_rd1_v: self.ctx.board.analog.channel_voltage(1),
            analog_rb2_v: self.ctx.board.analog.channel_voltage(3),
            adc_code: self.ctx.board.pcf8591.adc_data(),
            adc_channel,
            adc_channel_voltage_v: self.ctx.board.analog.channel_voltage(adc_channel),
            dac_code: self.ctx.board.pcf8591.dac_value(),
            dac_voltage_v: self.ctx.board.pcf8591.dac_voltage_v(),
            ne555_level: self.ctx.board.frequency_level(),
            ne555_frequency_hz: self.ctx.board.ne555.frequency_hz(),
            ds18b20_temperature_c: self.ctx.board.ds18b20.temperature_c,
            ultrasonic_distance_cm: self.ctx.board.ultrasonic.distance_cm,
            jumper_net_sig_to_sig_out: self
                .ctx
                .board
                .jumper_installed(SignalId::NetSig, SignalId::SigOut),
            uart1_text,
            uart1_text_error,
            uart1_raw: self.uart1_peek_raw(),
            uart2_text,
            uart2_text_error,
            uart2_raw: self.uart2_peek_raw(),
        }
    }

    pub fn snapshot_text(&self) -> String {
        let mut out = String::new();
        let board_latches = self.ctx.effective_board_latches();
        let timer = self.ctx.ports.timers.snapshot(&self.ctx.ports.generic);
        let _ = writeln!(out, "cpu_cycles: {}", self.ctx.board.cpu_cycles);
        let _ = writeln!(out, "sim_time_ns: {}", self.ctx.board.sim_time_ns);
        let _ = writeln!(out, "pc: 0x{:04X}", self.cpu.pc);
        let _ = writeln!(
            out,
            "cpu: IE={:02X} IP={:02X} SP={:02X}",
            self.cpu.register(Register::IE),
            self.cpu.register(Register::IP),
            self.cpu.register(Register::SP)
        );
        let _ = writeln!(
            out,
            "timer: TCON={:02X} TMOD={:02X} TL0={:02X} TH0={:02X} TL1={:02X} TH1={:02X} AUXR={:02X}",
            timer.tcon,
            timer.tmod,
            timer.tl0,
            timer.th0,
            timer.tl1,
            timer.th1,
            self.ctx.ports.generic_get(SFR_AUXR)
        );
        let _ = writeln!(
            out,
            "timer2_pca: T2H={:02X} T2L={:02X} CMOD={:02X} CCON={:02X} CH={:02X} CL={:02X}",
            timer.t2h, timer.t2l, timer.cmod, timer.ccon, timer.ch, timer.cl
        );
        let _ = writeln!(
            out,
            "ds1302: reg={:02X} reading={} io={} last_write={:02X}:{:02X}",
            self.ctx.board.ds1302.current_reg,
            self.ctx.board.ds1302.reading,
            self.ctx.board.ds1302.io_level,
            self.ctx.board.ds1302.last_write_reg,
            self.ctx.board.ds1302.last_write_value
        );
        let _ = writeln!(
            out,
            "ds1302_clock_write: {:02X}:{:02X}",
            self.ctx.board.ds1302.last_clock_write_reg,
            self.ctx.board.ds1302.last_clock_write_value
        );
        let _ = writeln!(
            out,
            "ds1302_read: {:02X}:{:02X}",
            self.ctx.board.ds1302.last_read_reg, self.ctx.board.ds1302.last_read_value
        );
        let _ = writeln!(
            out,
            "rtc: 20{:02}-{:02}-{:02} w{} {:02}:{:02}:{:02} halted={}",
            self.ctx.board.ds1302.year,
            self.ctx.board.ds1302.month,
            self.ctx.board.ds1302.date,
            self.ctx.board.ds1302.day_of_week,
            self.ctx.board.ds1302.hour,
            self.ctx.board.ds1302.minute,
            self.ctx.board.ds1302.second,
            self.ctx.board.ds1302.halted
        );
        let _ = writeln!(out, "display: {}", self.display_text());
        let digit_segments = self
            .ctx
            .board
            .outputs
            .digits
            .iter()
            .map(|digit| {
                if digit.seen {
                    format!("{:02X}", digit.segments)
                } else {
                    "--".to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(out, "digit_segments: [{digit_segments}]");
        let leds = self
            .ctx
            .board
            .outputs
            .leds
            .iter()
            .enumerate()
            .filter_map(|(index, on)| on.then_some((index + 1).to_string()))
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(out, "leds: [{}]", leds);
        let _ = writeln!(out, "relay: {}", self.relay_on());
        let _ = writeln!(out, "buzzer: {}", self.buzzer_on());
        let _ = writeln!(out, "motor: {}", self.motor_on());
        let _ = writeln!(out, "uart: {}", self.ctx.ports.uart1.tx_text());
        let _ = writeln!(
            out,
            "board_latches: [{:02X}, {:02X}, {:02X}, {:02X}]",
            board_latches[0], board_latches[1], board_latches[2], board_latches[3]
        );
        let _ = writeln!(
            out,
            "xdata_board_latches: [{:02X}, {:02X}, {:02X}, {:02X}]",
            self.ctx.xdata.board_latches[0],
            self.ctx.xdata.board_latches[1],
            self.ctx.xdata.board_latches[2],
            self.ctx.xdata.board_latches[3]
        );
        let _ = writeln!(
            out,
            "port_board_latches: [{:02X}, {:02X}, {:02X}, {:02X}]",
            self.ctx.ports.board_latches[0],
            self.ctx.ports.board_latches[1],
            self.ctx.ports.board_latches[2],
            self.ctx.ports.board_latches[3]
        );
        let _ = writeln!(out, "board_latch_source: {}", self.ctx.board_latch_source());
        let _ = writeln!(out, "jumpers: {}", self.ctx.board.jumpers.describe());
        let _ = writeln!(
            out,
            "port_latch: P0={:02X} P1={:02X} P2={:02X} P3={:02X} P4={:02X} P5={:02X}",
            self.ctx.ports.port_latch[0],
            self.ctx.ports.port_latch[1],
            self.ctx.ports.port_latch[2],
            self.ctx.ports.port_latch[3],
            self.ctx.ports.port_latch[4],
            self.ctx.ports.port_latch[5]
        );
        out
    }

    pub(crate) fn step_once(&mut self) -> Result<()> {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        let interrupt_poll_blocked = self.interrupt_poll_blocked_instructions != 0;
        if !interrupt_poll_blocked && self.try_enter_pending_interrupt()? {
            return Ok(());
        }
        let opcode = self.current_opcode();
        let ticks = approximate_instruction_ticks(opcode);
        if self.trace_cpu {
            let instruction = self.cpu.decode_pc(&self.ctx);
            trace!("{instruction:#}");
        }
        {
            let mut ctx = InterruptPollMaskContext::new(&mut self.ctx);
            let _ = self.cpu.step(&mut ctx);
            if interrupt_poll_blocked {
                self.interrupt_poll_blocked_instructions -= 1;
            }
        }
        if opcode == 0x32 {
            self.interrupt_poll_blocked_instructions = 1;
            self.handle_reti();
        } else {
            self.advance_uart_interrupt_reasserts();
        }
        self.tick_devices(ticks)
    }

    fn try_enter_pending_interrupt(&mut self) -> Result<bool> {
        let Some(pending) = self.pending_interrupt() else {
            return Ok(false);
        };
        if !enter_cpu_interrupt(&mut self.cpu, pending) {
            return Ok(false);
        }
        self.ack_interrupt_source(pending.source);
        self.active_interrupts.push(pending.source);
        if let Some(vector_addr) = pending.vector_addr() {
            self.cpu.pc = vector_addr;
        }
        self.note_interrupt_event(pending);
        if pending.tcon_clear_mask != 0 {
            let tcon = self.cpu.sfr(SFR_TCON, &self.ctx);
            self.cpu
                .sfr_set(SFR_TCON, tcon & !pending.tcon_clear_mask, &mut self.ctx);
        }
        trace!(
            pc = self.cpu.pc_ext(&self.ctx),
            interrupt = ?pending.source,
            delay_ticks = INTERRUPT_ENTRY_TICKS,
            "enter interrupt"
        );
        self.tick_devices(INTERRUPT_ENTRY_TICKS)?;
        Ok(true)
    }

    fn pending_interrupt(&self) -> Option<PendingInterrupt> {
        let ie = self.cpu.register(Register::IE) as u8;
        if ie & IE_EA == 0 {
            return None;
        }

        let ip = self.cpu.register(Register::IP) as u8;
        let ie2 = self.cpu.sfr(SFR_IE2, &self.ctx);
        let ip2 = self.cpu.sfr(SFR_IP2, &self.ctx);
        let tcon = self.cpu.sfr(SFR_TCON, &self.ctx);
        let scon = self.cpu.sfr(SFR_SCON, &self.ctx);
        let s2con = self.cpu.sfr(SFR_S2CON, &self.ctx);
        for high_priority in [true, false] {
            for candidate in [
                PendingInterrupt {
                    source: InterruptSource::External0,
                    tcon_clear_mask: TCON_IE0,
                },
                PendingInterrupt {
                    source: InterruptSource::Timer0,
                    tcon_clear_mask: TCON_TF0,
                },
                PendingInterrupt {
                    source: InterruptSource::External1,
                    tcon_clear_mask: TCON_IE1,
                },
                PendingInterrupt {
                    source: InterruptSource::Timer1,
                    tcon_clear_mask: TCON_TF1,
                },
                PendingInterrupt {
                    source: InterruptSource::Serial,
                    tcon_clear_mask: 0,
                },
                PendingInterrupt {
                    source: InterruptSource::Serial2,
                    tcon_clear_mask: 0,
                },
            ] {
                let (enable_mask, pending, priority_high) = match candidate.source {
                    InterruptSource::External0 => (IE_EX0, tcon & TCON_IE0 != 0, ip & IE_EX0 != 0),
                    InterruptSource::Timer0 => (IE_ET0, tcon & TCON_TF0 != 0, ip & IE_ET0 != 0),
                    InterruptSource::External1 => (IE_EX1, tcon & TCON_IE1 != 0, ip & IE_EX1 != 0),
                    InterruptSource::Timer1 => (IE_ET1, tcon & TCON_TF1 != 0, ip & IE_ET1 != 0),
                    InterruptSource::Serial => (
                        IE_ES,
                        self.ctx.ports.uart1.interrupt_requested()
                            && scon & (SCON_RI | SCON_TI) != 0,
                        ip & IE_ES != 0,
                    ),
                    InterruptSource::Serial2 => (
                        IE2_ES2,
                        self.ctx.ports.uart2.interrupt_requested()
                            && s2con & (S2CON_RI | S2CON_TI) != 0,
                        ip2 & IP2_PS2 != 0,
                    ),
                };
                let enabled = match candidate.source {
                    InterruptSource::Serial2 => ie2 & enable_mask != 0,
                    _ => ie & enable_mask != 0,
                };
                if !enabled || !pending || priority_high != high_priority {
                    continue;
                }
                return Some(candidate);
            }
        }

        None
    }

    fn tick_devices(&mut self, cycles: u32) -> Result<()> {
        if cycles == 0 {
            return Ok(());
        }

        let start_time_ns = self.ctx.board.sim_time_ns;
        let (elapsed_ns, system_cycles) = self.ctx.board.advance_cycles(u64::from(cycles));
        let t0_transitions = self.ctx.ports.t0_transition_iter(
            &self.ctx.board,
            start_time_ns,
            self.ctx.board.sim_time_ns,
        );
        self.ctx
            .ports
            .tick_timers01_t2(&self.ctx.board, system_cycles, t0_transitions)?;
        self.ctx
            .ports
            .tick_ultrasonic(&mut self.ctx.board, elapsed_ns);
        self.ctx.ports.tick_pca(system_cycles)?;
        self.ctx.ports.uart1.tick_ns(start_time_ns, elapsed_ns);
        self.ctx.ports.uart2.tick_ns(start_time_ns, elapsed_ns);
        let board_latches = self.ctx.effective_board_latches();
        let board_latch_versions = self.ctx.effective_board_latch_versions();
        self.ctx
            .board
            .tick_protocols(&self.ctx.ports, &board_latches, &board_latch_versions);
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        self.observe_i2c_events();
        self.observe_seg_events();
        self.drain_event_notes(start_time_ns, self.ctx.board.sim_time_ns);
        Ok(())
    }

    fn ack_interrupt_source(&mut self, source: InterruptSource) {
        match source {
            InterruptSource::Serial => self.ctx.ports.uart1.ack_interrupt(),
            InterruptSource::Serial2 => self.ctx.ports.uart2.ack_interrupt(),
            InterruptSource::External0
            | InterruptSource::Timer0
            | InterruptSource::External1
            | InterruptSource::Timer1 => {}
        }
    }

    fn handle_reti(&mut self) {
        match self.active_interrupts.pop() {
            Some(InterruptSource::Serial) => self
                .ctx
                .ports
                .uart1
                .arm_interrupt_reassert(UART_INTERRUPT_REASSERT_INSTRUCTIONS),
            Some(InterruptSource::Serial2) => self
                .ctx
                .ports
                .uart2
                .arm_interrupt_reassert(UART_INTERRUPT_REASSERT_INSTRUCTIONS),
            Some(
                InterruptSource::External0
                | InterruptSource::Timer0
                | InterruptSource::External1
                | InterruptSource::Timer1,
            )
            | None => {}
        }
    }

    fn advance_uart_interrupt_reasserts(&mut self) {
        self.ctx.ports.uart1.advance_interrupt_reassert();
        self.ctx.ports.uart2.advance_interrupt_reassert();
    }

    fn current_opcode(&self) -> u8 {
        self.ctx
            .code
            .code
            .get(usize::from(self.cpu.pc))
            .copied()
            .unwrap_or(0)
    }

    fn capture_control_snapshot(&mut self) {
        let time_ns = self.ctx.board.sim_time_ns;
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        self.observe_i2c_events();
        self.observe_seg_events();
        self.drain_event_notes(time_ns, time_ns);
    }

    fn current_i2c_lines(&self) -> (bool, bool, bool, bool) {
        let (i2c_slave_scl_low, i2c_slave_sda_low) = self
            .ctx
            .board
            .i2c
            .slave_drives_low(&self.ctx.board.pcf8591, &self.ctx.board.at24c02);
        let (i2c_bus_scl, i2c_bus_sda) = self.ctx.board.i2c.line_levels(
            self.ctx.ports.port_latch[2] & (1 << 0) != 0,
            self.ctx.ports.port_latch[2] & (1 << 1) != 0,
            i2c_slave_scl_low,
            i2c_slave_sda_low,
        );
        (
            i2c_slave_scl_low,
            i2c_slave_sda_low,
            i2c_bus_scl,
            i2c_bus_sda,
        )
    }

    fn capture_wave_snapshot(&mut self) {
        let time_ns = self.ctx.board.sim_time_ns;
        if !self.wave.captures_time(time_ns) {
            return;
        }
        let signal_sig_out = self.ctx.board.frequency_level();
        let adc_channel = self.ctx.board.pcf8591.selected_channel();
        let adc_channel_voltage_v = self.ctx.board.analog.channel_voltage(adc_channel);

        let effective_board_latches = self.ctx.effective_board_latches();
        let (i2c_slave_scl_low, i2c_slave_sda_low, i2c_bus_scl, i2c_bus_sda) =
            self.current_i2c_lines();

        let mut seg_chars = [' '; 8];
        let seg_text = self.ctx.board.outputs.display_text(&self.seg_decoder);
        let mut seg_raw = [0_u8; 8];
        for (index, digit) in self.ctx.board.outputs.digits.iter().copied().enumerate() {
            let ch = self.seg_decoder.decode_char(digit);
            seg_chars[index] = ch;
            seg_raw[index] = digit.segments;
        }

        let mut key_states = [false; 16];
        for (index, key) in WAVE_KEY_ORDER.into_iter().enumerate() {
            key_states[index] = self.ctx.board.keys.pressed(key);
        }

        let snapshot = WaveSnapshot {
            time_ns,
            port_latch: self.ctx.ports.port_latch,
            port_input: self.ctx.ports.port_input,
            board_latches_effective: effective_board_latches,
            board_latches_port: self.ctx.ports.board_latches,
            board_latches_xdata: self.ctx.xdata.board_latches,
            signal_sig_out,
            jumper_net_sig_to_sig_out: self
                .ctx
                .board
                .jumper_installed(SignalId::NetSig, SignalId::SigOut),
            i2c_master_scl: self.ctx.ports.port_latch[2] & (1 << 0) != 0,
            i2c_master_sda: self.ctx.ports.port_latch[2] & (1 << 1) != 0,
            i2c_bus_scl,
            i2c_bus_sda,
            i2c_slave_scl_low,
            i2c_slave_sda_low,
            onewire_master_high: self.ctx.ports.port_latch[1] & (1 << 4) != 0,
            onewire_bus_high: self.ctx.ports.port_input[1] & (1 << 4) != 0,
            onewire_device_low: self.ctx.board.ds18b20.drive_low,
            ds1302_ce: self.ctx.ports.port_latch[1] & (1 << 3) != 0,
            ds1302_clk: self.ctx.ports.port_latch[1] & (1 << 7) != 0,
            ds1302_io: self.ctx.ports.port_input[2] & (1 << 3) != 0,
            uart1_tx_high: self.ctx.ports.uart1.tx_line_high(),
            uart1_rx_high: self.ctx.ports.uart1.rx_line_high(),
            uart1_ti: self.ctx.ports.uart1.ti_pending(),
            uart1_ri: self.ctx.ports.uart1.ri_pending(),
            uart2_tx_high: self.ctx.ports.uart2.tx_line_high(),
            uart2_rx_high: self.ctx.ports.uart2.rx_line_high(),
            uart2_ti: self.ctx.ports.uart2.ti_pending(),
            uart2_ri: self.ctx.ports.uart2.ri_pending(),
            key_states,
            led_states: self.ctx.board.outputs.leds,
            relay_on: self.ctx.board.outputs.relay_on,
            motor_on: self.ctx.board.outputs.motor_on,
            buzzer_on: self.ctx.board.outputs.buzzer_on,
            seg_text,
            seg_chars,
            seg_raw,
            analog_rd1_v: self.ctx.board.analog.channel_voltage(1),
            analog_rb2_v: self.ctx.board.analog.channel_voltage(3),
            adc_code: self.ctx.board.pcf8591.adc_data(),
            adc_channel,
            adc_channel_voltage_v,
            dac_code: self.ctx.board.pcf8591.dac_value(),
            dac_voltage_v: self.ctx.board.pcf8591.dac_voltage_v(),
            ne555_level: signal_sig_out,
            ne555_frequency_hz: self.ctx.board.ne555.frequency_hz(),
        };
        self.wave.observe_snapshot(snapshot);
    }

    fn observe_i2c_events(&mut self) {
        let time_ns = self.ctx.board.sim_time_ns;
        if !self.event_gate.need_direct_event(EventTrack::I2c, time_ns) {
            self.i2c_event_decoder.reset();
            return;
        }

        let (_, _, i2c_bus_scl, i2c_bus_sda) = self.current_i2c_lines();
        for note in self
            .i2c_event_decoder
            .observe(time_ns, i2c_bus_scl, i2c_bus_sda)
        {
            self.record_observed_event(note);
        }
    }

    fn observe_seg_events(&mut self) {
        let time_ns = self.ctx.board.sim_time_ns;
        let need_wave_event = self.event_gate.need_wave_event(time_ns);
        if !need_wave_event && !self.event_gate.need_any_script_track() {
            self.seg_event_detector.reset();
            return;
        }

        let mut need_seg_events =
            need_wave_event || self.event_gate.need_script_track(EventTrack::SegChange);
        if !need_seg_events {
            for digit in 1..=8 {
                let Some(track) = EventTrack::seg_digit(digit) else {
                    continue;
                };
                if self.event_gate.need_script_track(track) {
                    need_seg_events = true;
                    break;
                }
            }
        }
        if !need_seg_events {
            self.seg_event_detector.reset();
            return;
        }

        let Some(change_set) = self
            .seg_event_detector
            .observe(self.ctx.board.outputs.digits)
        else {
            return;
        };
        if change_set.changed()
            && self
                .event_gate
                .need_direct_event(EventTrack::SegChange, time_ns)
        {
            self.record_observed_event(WaveEventNote::new(
                time_ns,
                EventTrack::SegChange.track_id(),
                "CHANGE",
            ));
        }
        for digit in 1..=8 {
            let Some(track) = EventTrack::seg_digit(digit) else {
                continue;
            };
            if change_set.digit_changed(digit - 1)
                && self.event_gate.need_direct_event(track, time_ns)
            {
                self.record_observed_event(WaveEventNote::new(
                    time_ns,
                    track.track_id(),
                    format!("D{digit} change"),
                ));
            }
        }
    }

    fn drain_event_notes(&mut self, start_time_ns: u64, end_time_ns: u64) {
        if !self
            .event_gate
            .need_any_direct_event_between(start_time_ns, end_time_ns)
        {
            return;
        }
        for note in self.ctx.board.ds18b20.take_event_notes() {
            self.record_observed_event(note);
        }
        for note in self.ctx.board.pcf8591.take_event_notes() {
            self.record_observed_event(note);
        }
        for note in self.ctx.board.ds1302.take_event_notes() {
            self.record_observed_event(note);
        }
        for note in self.ctx.ports.uart1.take_event_notes() {
            self.record_observed_event(note);
        }
        for note in self.ctx.ports.uart2.take_event_notes() {
            self.record_observed_event(note);
        }
    }

    fn note_interrupt_event(&mut self, pending: PendingInterrupt) {
        let time_ns = self.ctx.board.sim_time_ns;
        if !self.event_gate.need_direct_event(EventTrack::Cpu, time_ns) {
            return;
        }
        let label = match pending.source {
            InterruptSource::External0 => "INT0 enter",
            InterruptSource::Timer0 => "T0 enter",
            InterruptSource::External1 => "INT1 enter",
            InterruptSource::Timer1 => "T1 enter",
            InterruptSource::Serial => "UART enter",
            InterruptSource::Serial2 => "UART2 enter",
        };
        let note = WaveEventNote::with_detail(
            time_ns,
            TRACK_EVENT_CPU,
            label,
            format!("pc=0x{:04X}", self.cpu.pc_ext(&self.ctx)),
        );
        self.record_observed_event(note);
    }

    fn record_observed_event(&mut self, note: WaveEventNote) {
        let script_track = EventTrack::from_track_id(note.track_id)
            .filter(|track| self.event_gate.need_script_track(*track));
        let need_wave = self.event_gate.need_wave_event(note.time_ns);

        match (script_track, need_wave) {
            (Some(_), true) => {
                self.script_event_tracker.record(note.clone());
                self.wave.record_event_note(note);
            }
            (Some(_), false) => {
                self.script_event_tracker.record(note);
            }
            (None, true) => {
                self.wave.record_event_note(note);
            }
            (None, false) => {}
        }
    }
}

fn approximate_instruction_ticks(op: u8) -> u32 {
    match op {
        0x00 => 1,
        0x01 | 0x21 | 0x41 | 0x61 | 0x81 | 0xA1 | 0xC1 | 0xE1 => 3,
        0x11 | 0x31 | 0x51 | 0x71 | 0x91 | 0xB1 | 0xD1 | 0xF1 => 4,
        0x02 | 0x12 | 0x22 | 0x32 => 4,
        0x10 | 0x20 | 0x30 => 5,
        0x40 | 0x50 | 0x80 => 3,
        0x60 | 0x70 => 4,
        0x76 | 0x77 | 0x86 | 0x87 | 0x88..=0x8F | 0x90 | 0xA6..=0xAF => 2,
        0x05 | 0x15 | 0x42 | 0x45 | 0x52 | 0x55 | 0x62 | 0x65 | 0xA2 | 0xA3 | 0xB2 | 0xC2
        | 0xD2 | 0xE5 | 0xF5 => 1,
        0x43 | 0x53 | 0x63 | 0x75 | 0x85 | 0x92 => 2,
        0xB4 | 0xB8..=0xBF => 4,
        0xB5..=0xB7 => 5,
        0xC0 | 0xD0 => 2,
        0xD5 => 5,
        0xD8..=0xDF => 4,
        0x73 => 5,
        0x83 | 0x93 => 2,
        0xA4 | 0x84 => 4,
        0xE0 | 0xE2 | 0xE3 | 0xF0 | 0xF2 | 0xF3 => 2,
        _ => 1,
    }
}

fn uart_event_char(symbol: u16) -> Option<String> {
    let byte = u8::try_from(symbol).ok()?;
    Some(std::ascii::escape_default(byte).map(char::from).collect())
}

fn uart_event_label(direction: &str, symbol: u16) -> String {
    match uart_event_char(symbol) {
        Some(ch) => format!("{direction} 0x{symbol:02X} '{ch}'"),
        None => format!("{direction} 0x{symbol:03X}"),
    }
}

struct MachineContext {
    ports: MachinePorts,
    xdata: BoardXdata,
    code: CodeMemory,
    board: BoardModel,
}

#[derive(Debug, Clone, Copy)]
struct LatchedBoardState {
    board_latches: [u8; 4],
    board_latch_versions: [u64; 4],
    latch_used: bool,
}

impl LatchedBoardState {
    fn from_ports(ports: &MachinePorts) -> Self {
        Self {
            board_latches: ports.board_latches,
            board_latch_versions: ports.board_latch_versions,
            latch_used: ports.latch_used,
        }
    }

    fn from_xdata(xdata: &BoardXdata) -> Self {
        Self {
            board_latches: xdata.board_latches,
            board_latch_versions: xdata.board_latch_versions,
            latch_used: xdata.latch_used,
        }
    }

    fn apply_to_ports(self, ports: &mut MachinePorts) {
        ports.board_latches = self.board_latches;
        ports.board_latch_versions = self.board_latch_versions;
        ports.latch_used = self.latch_used;
    }

    fn apply_to_xdata(self, xdata: &mut BoardXdata) {
        xdata.board_latches = self.board_latches;
        xdata.board_latch_versions = self.board_latch_versions;
        xdata.latch_used = self.latch_used;
    }
}

impl MachineContext {
    #[cfg_attr(not(test), allow(dead_code))]
    fn new(code: Vec<u8>) -> Self {
        Self::new_with_wave_enabled(code, true)
    }

    fn new_with_wave_enabled(code: Vec<u8>, wave_enabled: bool) -> Self {
        Self::new_with_wave_window(code, WaveCaptureWindow::from_enabled(wave_enabled))
    }

    fn new_with_wave_window(code: Vec<u8>, wave_window: WaveCaptureWindow) -> Self {
        Self::new_with_event_gate(code, EventGate::shared(wave_window))
    }

    fn new_with_event_gate(code: Vec<u8>, event_gate: SharedEventGate) -> Self {
        let mut board = BoardModel::new_with_event_gate(Arc::clone(&event_gate));
        board
            .outputs
            .sample_from_latches(&BOARD_POWER_ON_LATCHES, &[0; 4], 0);
        Self {
            ports: MachinePorts::new_with_event_gate(event_gate),
            xdata: BoardXdata::default(),
            code: CodeMemory { code },
            board,
        }
    }

    fn effective_board_latches(&self) -> [u8; 4] {
        if self.ports.latch_used {
            self.ports.board_latches
        } else if self.xdata.latch_used {
            self.xdata.board_latches
        } else {
            BOARD_POWER_ON_LATCHES
        }
    }

    fn effective_board_latch_versions(&self) -> [u64; 4] {
        if self.ports.latch_used {
            self.ports.board_latch_versions
        } else if self.xdata.latch_used {
            self.xdata.board_latch_versions
        } else {
            [0; 4]
        }
    }

    fn board_latch_source(&self) -> &'static str {
        if self.ports.latch_used {
            "p0_p2"
        } else if self.xdata.latch_used {
            "xdata"
        } else {
            "none"
        }
    }
}

impl CpuContext for MachineContext {
    type Ports = MachinePorts;
    type Xdata = BoardXdata;
    type Code = CodeMemory;

    fn ports(&self) -> &Self::Ports {
        &self.ports
    }

    fn xdata(&self) -> &Self::Xdata {
        &self.xdata
    }

    fn code(&self) -> &Self::Code {
        &self.code
    }

    fn ports_mut(&mut self) -> &mut Self::Ports {
        &mut self.ports
    }

    fn xdata_mut(&mut self) -> &mut Self::Xdata {
        &mut self.xdata
    }

    fn code_mut(&mut self) -> &mut Self::Code {
        &mut self.code
    }
}

struct BoardXdata {
    ram: Vec<u8>,
    board_latches: [u8; 4],
    board_latch_versions: [u64; 4],
    latch_used: bool,
}

impl Default for BoardXdata {
    fn default() -> Self {
        Self {
            ram: Vec::new(),
            board_latches: BOARD_POWER_ON_LATCHES,
            board_latch_versions: [0; 4],
            latch_used: false,
        }
    }
}

impl MemoryMapper for BoardXdata {
    type WriteValue = (u16, u8);

    fn len(&self) -> u32 {
        0x10000
    }

    fn read<C: CpuView>(&self, cpu: &C, addr: u32) -> u8 {
        let addr = Self::effective_addr(cpu, addr as u16);
        let addr = Self::normalize_ram_addr(addr);
        self.ram.get(addr as usize).copied().unwrap_or(0)
    }

    fn prepare_write<C: CpuView>(&self, cpu: &C, addr: u32, value: u8) -> Self::WriteValue {
        (Self::effective_addr(cpu, addr as u16), value)
    }

    fn write(&mut self, value: Self::WriteValue) {
        let (addr, byte) = value;
        let index = usize::from(Self::normalize_ram_addr(addr));
        if self.ram.len() <= index {
            self.ram.resize(index + 1, 0);
        }
        self.ram[index] = byte;
        match addr & 0xE000 {
            0x8000 => {
                self.board_latches[0] = byte;
                self.board_latch_versions[0] = self.board_latch_versions[0].saturating_add(1);
                self.latch_used = true;
            }
            0xA000 => {
                self.board_latches[1] = byte;
                self.board_latch_versions[1] = self.board_latch_versions[1].saturating_add(1);
                self.latch_used = true;
            }
            0xC000 => {
                self.board_latches[2] = byte;
                self.board_latch_versions[2] = self.board_latch_versions[2].saturating_add(1);
                self.latch_used = true;
            }
            0xE000 => {
                self.board_latches[3] = byte;
                self.board_latch_versions[3] = self.board_latch_versions[3].saturating_add(1);
                self.latch_used = true;
            }
            _ => {}
        }
    }
}

impl BoardXdata {
    fn effective_addr<C: CpuView>(cpu: &C, addr: u16) -> u16 {
        if Self::is_short_movx(cpu) && cpu.sfr(SFR_AUXR) & AUXR_EXTRAM == 0 {
            addr & 0x00FF
        } else {
            addr
        }
    }

    fn is_short_movx<C: CpuView>(cpu: &C) -> bool {
        matches!(cpu.read_code(cpu.pc_ext()), 0xE2..=0xE3 | 0xF2..=0xF3)
    }

    fn normalize_ram_addr(addr: u16) -> u16 {
        if addr < 0x8000 { addr & 0x07FF } else { addr }
    }

    fn raw_read(&self, addr: u16) -> u8 {
        let index = usize::from(Self::normalize_ram_addr(addr));
        self.ram.get(index).copied().unwrap_or(0)
    }

    fn raw_write(&mut self, addr: u16, value: u8) {
        let index = usize::from(Self::normalize_ram_addr(addr));
        if self.ram.len() <= index {
            self.ram.resize(index + 1, 0);
        }
        self.ram[index] = value;
        match addr & 0xE000 {
            0x8000 => {
                self.board_latches[0] = value;
                self.board_latch_versions[0] = self.board_latch_versions[0].saturating_add(1);
                self.latch_used = true;
            }
            0xA000 => {
                self.board_latches[1] = value;
                self.board_latch_versions[1] = self.board_latch_versions[1].saturating_add(1);
                self.latch_used = true;
            }
            0xC000 => {
                self.board_latches[2] = value;
                self.board_latch_versions[2] = self.board_latch_versions[2].saturating_add(1);
                self.latch_used = true;
            }
            0xE000 => {
                self.board_latches[3] = value;
                self.board_latch_versions[3] = self.board_latch_versions[3].saturating_add(1);
                self.latch_used = true;
            }
            _ => {}
        }
    }
}

struct CodeMemory {
    code: Vec<u8>,
}

impl ReadOnlyMemoryMapper for CodeMemory {
    fn len(&self) -> u32 {
        self.code.len() as u32
    }

    fn read<C: CpuView>(&self, _cpu: &C, addr: u32) -> u8 {
        self.code.get(addr as usize).copied().unwrap_or(0)
    }
}

struct MachinePorts {
    generic: [u8; 128],
    port_latch: [u8; 6],
    port_input: [u8; 6],
    board_latches: [u8; 4],
    board_latch_versions: [u64; 4],
    latch_used: bool,
    timers: TimerBlock,
    uart1: Uart,
    uart2: Uart,
}

impl MachinePorts {
    #[allow(dead_code)]
    fn new() -> Self {
        Self::new_with_wave_enabled(true)
    }

    fn new_with_wave_enabled(wave_enabled: bool) -> Self {
        Self::new_with_wave_window(WaveCaptureWindow::from_enabled(wave_enabled))
    }

    fn new_with_wave_window(wave_window: WaveCaptureWindow) -> Self {
        Self::new_with_event_gate(EventGate::shared(wave_window))
    }

    fn new_with_event_gate(event_gate: SharedEventGate) -> Self {
        let mut generic = [0_u8; 128];
        let mut port_latch = [0xFF_u8; 6];
        port_latch[5] = 0x3F;
        generic[(SFR_AUXR - 0x80) as usize] = 0x01;
        generic[(SFR_P1 - 0x80) as usize] = 0xFF;
        generic[(SFR_P2 - 0x80) as usize] = 0xFF;
        generic[(SFR_P3 - 0x80) as usize] = 0xFF;
        generic[(SFR_P4 - 0x80) as usize] = 0xFF;
        generic[(SFR_P5 - 0x80) as usize] = 0x3F;
        Self {
            generic,
            port_latch,
            port_input: port_latch,
            board_latches: BOARD_POWER_ON_LATCHES,
            board_latch_versions: [0; 4],
            latch_used: false,
            timers: TimerBlock::default(),
            uart1: Uart::new(
                UART1_SFR_SCON,
                UART1_SFR_SBUF,
                EventTrack::Uart1,
                Arc::clone(&event_gate),
            ),
            uart2: Uart::new(
                UART2_SFR_S2CON,
                UART2_SFR_S2BUF,
                EventTrack::Uart2,
                event_gate,
            ),
        }
    }

    fn port_index(addr: u8) -> Option<usize> {
        match addr {
            SFR_P0 => Some(0),
            SFR_P1 => Some(1),
            SFR_P2 => Some(2),
            SFR_P3 => Some(3),
            SFR_P4 => Some(4),
            SFR_P5 => Some(5),
            _ => None,
        }
    }

    fn generic_get(&self, addr: u8) -> u8 {
        self.generic[usize::from(addr.wrapping_sub(0x80))]
    }

    fn generic_set(&mut self, addr: u8, value: u8) {
        self.generic[usize::from(addr.wrapping_sub(0x80))] = value;
    }

    fn rewrite_port_rmw<C: CpuView>(&self, cpu: &C, addr: u8, fallback: u8) -> u8 {
        let Some(index) = Self::port_index(addr) else {
            return fallback;
        };
        let latch = self.port_latch[index];
        let pc = cpu.pc_ext();
        let opcode = cpu.read_code(pc);
        let op_addr = cpu.read_code(pc + 1);
        match opcode {
            0x42 if op_addr == addr => latch | cpu.a(),
            0x43 if op_addr == addr => latch | cpu.read_code(pc + 2),
            0x52 if op_addr == addr => latch & cpu.a(),
            0x53 if op_addr == addr => latch & cpu.read_code(pc + 2),
            0x62 if op_addr == addr => latch ^ cpu.a(),
            0x63 if op_addr == addr => latch ^ cpu.read_code(pc + 2),
            0x05 if op_addr == addr => latch.wrapping_add(1),
            0x15 if op_addr == addr => latch.wrapping_sub(1),
            0xD5 if op_addr == addr => latch.wrapping_sub(1),
            0x92 => {
                let bit_addr = cpu.read_code(pc + 1);
                if bit_addr & 0xF8 != addr {
                    return fallback;
                }
                let mask = 1 << (bit_addr & 0x07);
                if cpu.psw(Flag::C) {
                    latch | mask
                } else {
                    latch & !mask
                }
            }
            _ => fallback,
        }
    }

    fn sample_port_p3(&self, board: &BoardModel) -> u8 {
        board.read_port(3, self.port_latch[3], &self.port_latch)
    }

    fn sync_inputs(&mut self, board: &BoardModel) {
        self.port_input = board.read_ports(&self.port_latch);
    }

    fn refresh_inputs(&mut self, board: &BoardModel) {
        let prev_p3 = self.port_input[3];
        self.sync_inputs(board);
        self.update_external_interrupt_flags(prev_p3, self.port_input[3]);
    }

    fn update_external_interrupt_flags(&mut self, prev_p3: u8, next_p3: u8) {
        let tcon = self
            .timers
            .read(&self.generic, SFR_TCON)
            .expect("TCON should be readable");
        let mut next_tcon = tcon;
        next_tcon =
            apply_external_interrupt_flag(next_tcon, TCON_IT0, TCON_IE0, P3_INT0, prev_p3, next_p3);
        next_tcon =
            apply_external_interrupt_flag(next_tcon, TCON_IT1, TCON_IE1, P3_INT1, prev_p3, next_p3);
        if next_tcon != tcon {
            let _ = self.timers.write(&mut self.generic, SFR_TCON, next_tcon);
        }
    }

    fn tick_ultrasonic(&mut self, board: &mut BoardModel, elapsed_ns: u64) {
        let tx_high = self.port_latch[1] & (1 << 0) != 0;
        board.ultrasonic.sample_trigger(tx_high);
        board.ultrasonic.tick_ns(elapsed_ns);
    }

    fn tick_timers01_t2(
        &mut self,
        board: &BoardModel,
        ticks: u32,
        t0_transitions: SignalTransitionIter,
    ) -> Result<()> {
        let p3 = self.sample_port_p3(board);
        let auxr = self.generic_get(SFR_AUXR);
        self.timers
            .tick_timers01_t2(p3, auxr, ticks, t0_transitions, &mut self.generic)
    }

    fn tick_pca(&mut self, ticks: u32) -> Result<()> {
        self.timers.tick_pca(ticks, &mut self.generic)
    }

    fn timer0_external_counter_enabled(&self) -> bool {
        let tcon = self
            .timers
            .read(&self.generic, SFR_TCON)
            .expect("TCON should be readable");
        let tmod = self
            .timers
            .read(&self.generic, SFR_TMOD)
            .expect("TMOD should be readable");
        tcon & TCON_TR0 != 0 && tmod & TMOD_C_T0 != 0
    }

    fn t0_transition_iter(
        &self,
        board: &BoardModel,
        start_time_ns: u64,
        end_time_ns: u64,
    ) -> SignalTransitionIter {
        if !self.timer0_external_counter_enabled() {
            return SignalTransitionIter::empty();
        }
        board.t0_transitions(&self.port_latch, start_time_ns, end_time_ns)
    }

    fn strobe_board_latch(&mut self, select: u8, value: u8) {
        let slot = match select & 0xE0 {
            0x80 => Some(0),
            0xA0 => Some(1),
            0xC0 => Some(2),
            0xE0 => Some(3),
            _ => None,
        };
        if let Some(slot) = slot {
            self.board_latches[slot] = value;
            self.board_latch_versions[slot] = self.board_latch_versions[slot].saturating_add(1);
            self.latch_used = true;
        }
    }
}

impl PortMapper for MachinePorts {
    type WriteValue = (u8, u8);

    fn interest<C: CpuView>(&self, _cpu: &C, _addr: u8) -> bool {
        true
    }

    fn read<C: CpuView>(&self, _cpu: &C, addr: u8) -> u8 {
        match addr {
            addr if TimerBlock::handles(addr) => self.timers.read(&self.generic, addr).unwrap_or(0),
            UART1_SFR_SCON | UART1_SFR_SBUF => self.uart1.read(addr),
            UART2_SFR_S2CON | UART2_SFR_S2BUF => self.uart2.read(addr),
            SFR_P0 | SFR_P1 | SFR_P2 | SFR_P3 | SFR_P4 | SFR_P5 => {
                let Some(index) = Self::port_index(addr) else {
                    unreachable!();
                };
                self.port_input[index]
            }
            _ => self.generic_get(addr),
        }
    }

    fn read_latch<C: CpuView>(&self, _cpu: &C, addr: u8) -> u8 {
        if let Some(index) = Self::port_index(addr) {
            return self.port_latch[index];
        }
        match addr {
            UART1_SFR_SCON | UART1_SFR_SBUF => return self.uart1.read(addr),
            UART2_SFR_S2CON | UART2_SFR_S2BUF => return self.uart2.read(addr),
            _ => {}
        }
        if TimerBlock::handles(addr) {
            return self.timers.read(&self.generic, addr).unwrap_or(0);
        }
        self.generic_get(addr)
    }

    fn prepare_write<C: CpuView>(&self, _cpu: &C, addr: u8, value: u8) -> Self::WriteValue {
        let value = self.rewrite_port_rmw(_cpu, addr, value);
        (addr, value)
    }

    fn write(&mut self, value: Self::WriteValue) {
        let (addr, byte) = value;
        match addr {
            addr if TimerBlock::handles(addr) => {
                let _ = self.timers.write(&mut self.generic, addr, byte);
            }
            UART1_SFR_SCON | UART1_SFR_SBUF => self.uart1.write(addr, byte),
            UART2_SFR_S2CON | UART2_SFR_S2BUF => self.uart2.write(addr, byte),
            SFR_P0 | SFR_P1 | SFR_P2 | SFR_P3 | SFR_P4 | SFR_P5 => {
                let index = Self::port_index(addr).expect("port index");
                self.port_latch[index] = byte;
                self.generic_set(addr, byte);
                if addr == SFR_P2 {
                    self.strobe_board_latch(byte, self.port_latch[0]);
                }
            }
            _ => self.generic_set(addr, byte),
        }
    }
}

struct InterruptPollMaskContext<'a> {
    ports: InterruptPollMaskPorts<'a>,
    xdata: InterruptPollMaskXdata<'a>,
    code: InterruptPollMaskCode<'a>,
}

impl<'a> InterruptPollMaskContext<'a> {
    fn new(ctx: &'a mut MachineContext) -> Self {
        let interrupt_poll_active = Rc::new(Cell::new(true));
        Self {
            ports: InterruptPollMaskPorts {
                inner: &mut ctx.ports,
                interrupt_poll_active: interrupt_poll_active.clone(),
            },
            xdata: InterruptPollMaskXdata {
                inner: &mut ctx.xdata,
            },
            code: InterruptPollMaskCode {
                inner: &mut ctx.code,
                interrupt_poll_active,
            },
        }
    }
}

impl<'a> CpuContext for InterruptPollMaskContext<'a> {
    type Ports = InterruptPollMaskPorts<'a>;
    type Xdata = InterruptPollMaskXdata<'a>;
    type Code = InterruptPollMaskCode<'a>;

    fn ports(&self) -> &Self::Ports {
        &self.ports
    }

    fn xdata(&self) -> &Self::Xdata {
        &self.xdata
    }

    fn code(&self) -> &Self::Code {
        &self.code
    }

    fn ports_mut(&mut self) -> &mut Self::Ports {
        &mut self.ports
    }

    fn xdata_mut(&mut self) -> &mut Self::Xdata {
        &mut self.xdata
    }

    fn code_mut(&mut self) -> &mut Self::Code {
        &mut self.code
    }
}

struct InterruptPollMaskPorts<'a> {
    inner: &'a mut MachinePorts,
    interrupt_poll_active: Rc<Cell<bool>>,
}

impl PortMapper for InterruptPollMaskPorts<'_> {
    type WriteValue = (u8, u8);

    fn interest<C: CpuView>(&self, cpu: &C, addr: u8) -> bool {
        self.inner.interest(cpu, addr)
    }

    fn extend_short_read<C: CpuView>(&self, cpu: &C, addr: u8) -> u16 {
        self.inner.extend_short_read(cpu, addr)
    }

    fn pc_extension<C: CpuView>(&self, cpu: &C) -> u16 {
        self.inner.pc_extension(cpu)
    }

    fn read<C: CpuView>(&self, cpu: &C, addr: u8) -> u8 {
        let value = self.inner.read(cpu, addr);
        if !self.interrupt_poll_active.get() {
            return value;
        }
        match addr {
            SFR_SCON => value & !(SCON_RI | SCON_TI),
            SFR_S2CON => value & !(S2CON_RI | S2CON_TI),
            SFR_TCON => value & !(TCON_IE0 | TCON_IE1 | TCON_TF0 | TCON_TF1),
            SFR_P3 => value | P3_INT0 | P3_INT1,
            _ => value,
        }
    }

    fn read_latch<C: CpuView>(&self, cpu: &C, addr: u8) -> u8 {
        self.inner.read_latch(cpu, addr)
    }

    fn prepare_write<C: CpuView>(&self, cpu: &C, addr: u8, value: u8) -> Self::WriteValue {
        self.inner.prepare_write(cpu, addr, value)
    }

    fn write(&mut self, value: Self::WriteValue) {
        self.inner.write(value);
    }
}

struct InterruptPollMaskXdata<'a> {
    inner: &'a mut BoardXdata,
}

impl MemoryMapper for InterruptPollMaskXdata<'_> {
    type WriteValue = (u16, u8);

    fn len(&self) -> u32 {
        self.inner.len()
    }

    fn read<C: CpuView>(&self, cpu: &C, addr: u32) -> u8 {
        self.inner.read(cpu, addr)
    }

    fn prepare_write<C: CpuView>(&self, cpu: &C, addr: u32, value: u8) -> Self::WriteValue {
        self.inner.prepare_write(cpu, addr, value)
    }

    fn write(&mut self, value: Self::WriteValue) {
        self.inner.write(value);
    }
}

struct InterruptPollMaskCode<'a> {
    inner: &'a mut CodeMemory,
    interrupt_poll_active: Rc<Cell<bool>>,
}

impl ReadOnlyMemoryMapper for InterruptPollMaskCode<'_> {
    fn len(&self) -> u32 {
        self.inner.len()
    }

    fn read<C: CpuView>(&self, cpu: &C, addr: u32) -> u8 {
        self.interrupt_poll_active.set(false);
        self.inner.read(cpu, addr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartParity {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartStopBits {
    One,
    OnePointFive,
    Two,
}

#[derive(Debug, Clone, Copy)]
pub struct UartConfig {
    pub data_bits: u8,
    pub baud_rate: u32,
    pub stop_bits: UartStopBits,
    pub parity: UartParity,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            data_bits: 8,
            baud_rate: 9_600,
            stop_bits: UartStopBits::One,
            parity: UartParity::None,
        }
    }
}

impl UartConfig {
    pub fn validate(self) -> Result<()> {
        if !(5..=9).contains(&self.data_bits) {
            bail!("串口数据位只支持 5..=9");
        }
        if self.baud_rate == 0 {
            bail!("串口波特率必须 > 0");
        }
        Ok(())
    }

    fn bit_ns(self) -> u64 {
        ((NS_PER_SECOND as f64) / f64::from(self.baud_rate))
            .round()
            .clamp(1.0, u64::MAX as f64) as u64
    }

    fn stop_ns(self) -> u64 {
        let bit_ns = self.bit_ns();
        match self.stop_bits {
            UartStopBits::One => bit_ns,
            UartStopBits::OnePointFive => {
                ((bit_ns as f64) * 1.5).round().clamp(1.0, u64::MAX as f64) as u64
            }
            UartStopBits::Two => bit_ns.saturating_mul(2),
        }
    }

    fn max_symbol(self) -> u16 {
        if self.data_bits >= 16 {
            u16::MAX
        } else {
            (1_u16 << self.data_bits) - 1
        }
    }

    fn validate_symbol(self, symbol: u16) -> Result<()> {
        let max_symbol = self.max_symbol();
        if symbol > max_symbol {
            bail!(
                "串口符号 0x{symbol:X} 超出当前数据位宽, data_bits={}, max=0x{max_symbol:X}",
                self.data_bits
            );
        }
        Ok(())
    }

    fn parity_bit(self, symbol: u16) -> Option<bool> {
        match self.parity {
            UartParity::None => None,
            UartParity::Mark => Some(true),
            UartParity::Space => Some(false),
            UartParity::Odd | UartParity::Even => {
                let ones = (0..self.data_bits)
                    .filter(|bit| symbol & (1_u16 << bit) != 0)
                    .count();
                let data_is_odd = ones % 2 == 1;
                Some(match self.parity {
                    UartParity::Odd => !data_is_odd,
                    UartParity::Even => data_is_odd,
                    UartParity::None | UartParity::Mark | UartParity::Space => unreachable!(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UartFrameSegment {
    level: bool,
    duration_ns: u64,
}

#[derive(Debug)]
struct UartFrame {
    symbol: u16,
    started_at_ns: u64,
    completed_at_ns: u64,
    segments: Vec<UartFrameSegment>,
    segment_index: usize,
    segment_remaining_ns: u64,
}

impl UartFrame {
    fn new(symbol: u16, config: UartConfig, started_at_ns: u64) -> Self {
        let bit_ns = config.bit_ns();
        let mut segments = Vec::with_capacity(1 + usize::from(config.data_bits) + 2);
        segments.push(UartFrameSegment {
            level: false,
            duration_ns: bit_ns,
        });
        for bit in 0..config.data_bits {
            segments.push(UartFrameSegment {
                level: symbol & (1_u16 << bit) != 0,
                duration_ns: bit_ns,
            });
        }
        if let Some(parity_bit) = config.parity_bit(symbol) {
            segments.push(UartFrameSegment {
                level: parity_bit,
                duration_ns: bit_ns,
            });
        }
        segments.push(UartFrameSegment {
            level: true,
            duration_ns: config.stop_ns(),
        });
        let completed_at_ns = segments.iter().fold(started_at_ns, |time_ns, segment| {
            time_ns.saturating_add(segment.duration_ns)
        });

        Self {
            symbol,
            started_at_ns,
            completed_at_ns,
            segment_remaining_ns: segments[0].duration_ns,
            segments,
            segment_index: 0,
        }
    }

    fn current_level(&self) -> bool {
        self.segments
            .get(self.segment_index)
            .map(|segment| segment.level)
            .unwrap_or(true)
    }

    fn advance(&mut self, elapsed_ns: u64) -> bool {
        let mut remaining = elapsed_ns;
        while remaining >= self.segment_remaining_ns {
            remaining -= self.segment_remaining_ns;
            self.segment_index = self.segment_index.saturating_add(1);
            if self.segment_index >= self.segments.len() {
                return true;
            }
            self.segment_remaining_ns = self.segments[self.segment_index].duration_ns;
        }
        self.segment_remaining_ns -= remaining;
        false
    }
}

#[derive(Debug, Clone, Copy)]
struct TimedUartSymbol {
    symbol: u16,
    started_at_ns: u64,
    completed_at_ns: u64,
}

#[derive(Debug)]
struct Uart {
    scon_addr: u8,
    sbuf_addr: u8,
    event_track: EventTrack,
    control: u8,
    interrupt_requested: bool,
    interrupt_reassert_countdown: Option<u8>,
    rx_sbuf: u8,
    tx_queue: VecDeque<TimedUartSymbol>,
    tx_pending: VecDeque<u16>,
    rx_queue: VecDeque<u16>,
    tx_frame: Option<UartFrame>,
    rx_frame: Option<UartFrame>,
    config: UartConfig,
    tx_line_high: bool,
    rx_line_high: bool,
    event_gate: SharedEventGate,
    event_notes: Vec<WaveEventNote>,
}

impl Uart {
    fn new(
        scon_addr: u8,
        sbuf_addr: u8,
        event_track: EventTrack,
        event_gate: SharedEventGate,
    ) -> Self {
        Self {
            scon_addr,
            sbuf_addr,
            event_track,
            control: 0,
            interrupt_requested: false,
            interrupt_reassert_countdown: None,
            rx_sbuf: 0,
            tx_queue: VecDeque::new(),
            tx_pending: VecDeque::new(),
            rx_queue: VecDeque::new(),
            tx_frame: None,
            rx_frame: None,
            config: UartConfig::default(),
            tx_line_high: true,
            rx_line_high: true,
            event_gate,
            event_notes: Vec::new(),
        }
    }

    fn read(&self, addr: u8) -> u8 {
        match addr {
            addr if addr == self.scon_addr => self.control,
            addr if addr == self.sbuf_addr => self.rx_sbuf,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u8, value: u8) {
        if addr == self.scon_addr {
            let old_control = self.control;
            self.control = value;
            self.note_interrupt_flag_edges(old_control, self.control);
        } else if addr == self.sbuf_addr {
            let ninth_bit = if self.config.data_bits >= 9 && self.tb8_set() {
                1_u16 << 8
            } else {
                0
            };
            self.tx_pending.push_back(u16::from(value) | ninth_bit);
        }
    }

    fn tick_ns(&mut self, start_time_ns: u64, elapsed_ns: u64) {
        if self.tx_frame.is_none()
            && let Some(symbol) = self.tx_pending.pop_front()
        {
            self.tx_frame = Some(UartFrame::new(symbol, self.config, start_time_ns));
            self.tx_line_high = false;
            let data_bits = self.config.data_bits;
            let track_id = self.event_track.track_id();
            self.push_event_note(start_time_ns, || {
                WaveEventNote::with_detail(
                    start_time_ns,
                    track_id,
                    uart_event_label("TX", symbol),
                    format!("bits={data_bits}"),
                )
            });
        }

        if let Some(frame) = self.tx_frame.as_mut() {
            if frame.advance(elapsed_ns) {
                let symbol = frame.symbol;
                let started_at_ns = frame.started_at_ns;
                let completed_at_ns = frame.completed_at_ns;
                self.tx_frame = None;
                self.tx_line_high = true;
                let old_control = self.control;
                self.control |= if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_TI
                } else {
                    SCON_TI
                };
                self.note_interrupt_flag_edges(old_control, self.control);
                self.tx_queue.push_back(TimedUartSymbol {
                    symbol,
                    started_at_ns,
                    completed_at_ns,
                });
            } else {
                self.tx_line_high = frame.current_level();
            }
        }

        if self.rx_frame.is_none()
            && let Some(symbol) = self.rx_queue.pop_front()
        {
            self.rx_frame = Some(UartFrame::new(symbol, self.config, start_time_ns));
            self.rx_line_high = false;
            let data_bits = self.config.data_bits;
            let track_id = self.event_track.track_id();
            self.push_event_note(start_time_ns, || {
                WaveEventNote::with_detail(
                    start_time_ns,
                    track_id,
                    uart_event_label("RX", symbol),
                    format!("bits={data_bits}"),
                )
            });
        }

        if let Some(frame) = self.rx_frame.as_mut() {
            if frame.advance(elapsed_ns) {
                let symbol = frame.symbol;
                self.rx_frame = None;
                self.rx_line_high = true;
                let ren_flag = if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_REN
                } else {
                    SCON_REN
                };
                let ri_flag = if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_RI
                } else {
                    SCON_RI
                };
                if self.control & ren_flag != 0 {
                    let old_control = self.control;
                    self.rx_sbuf = symbol as u8;
                    self.set_rb8(symbol & (1_u16 << 8) != 0);
                    self.control |= ri_flag;
                    self.note_interrupt_flag_edges(old_control, self.control);
                }
            } else {
                self.rx_line_high = frame.current_level();
            }
        }
    }

    fn configure(&mut self, config: UartConfig) -> Result<()> {
        config.validate()?;
        self.config = config;
        Ok(())
    }

    fn feed_rx_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let symbols = bytes.iter().copied().map(u16::from).collect::<Vec<_>>();
        self.feed_rx_raw(&symbols)
    }

    fn feed_rx_raw(&mut self, symbols: &[u16]) -> Result<()> {
        for symbol in symbols {
            self.config.validate_symbol(*symbol)?;
        }
        self.rx_queue.extend(symbols.iter().copied());
        Ok(())
    }

    fn take_event_notes(&mut self) -> Vec<WaveEventNote> {
        std::mem::take(&mut self.event_notes)
    }

    fn tx_line_high(&self) -> bool {
        self.tx_line_high
    }

    fn rx_line_high(&self) -> bool {
        self.rx_line_high
    }

    fn ti_pending(&self) -> bool {
        self.control
            & if self.scon_addr == UART2_SFR_S2CON {
                S2CON_TI
            } else {
                SCON_TI
            }
            != 0
    }

    fn ri_pending(&self) -> bool {
        self.control
            & if self.scon_addr == UART2_SFR_S2CON {
                S2CON_RI
            } else {
                SCON_RI
            }
            != 0
    }

    fn take_tx_string(&mut self) -> Result<String> {
        let symbols = self
            .tx_queue
            .drain(..)
            .map(|entry| entry.symbol)
            .collect::<Vec<_>>();
        Self::symbols_to_string(&symbols)
    }

    fn take_tx_raw(&mut self) -> Vec<u16> {
        self.tx_queue.drain(..).map(|entry| entry.symbol).collect()
    }

    fn clear_tx_output(&mut self) {
        self.tx_queue.clear();
    }

    fn take_tx_string_segment(&mut self, idle_ms: u64) -> Result<String> {
        let symbols = self.take_tx_raw_segment(idle_ms);
        Self::symbols_to_string(&symbols)
    }

    fn take_tx_raw_segment(&mut self, idle_ms: u64) -> Vec<u16> {
        let count = self.segment_symbol_count(idle_ms);
        self.tx_queue
            .drain(..count)
            .map(|entry| entry.symbol)
            .collect()
    }

    fn peek_tx_string(&self) -> Result<String> {
        let symbols = self
            .tx_queue
            .iter()
            .map(|entry| entry.symbol)
            .collect::<Vec<_>>();
        Self::symbols_to_string(&symbols)
    }

    fn peek_tx_raw(&self) -> Vec<u16> {
        self.tx_queue.iter().map(|entry| entry.symbol).collect()
    }

    fn peek_tx_string_segment(&self, idle_ms: u64) -> Result<String> {
        let symbols = self.peek_tx_raw_segment(idle_ms);
        Self::symbols_to_string(&symbols)
    }

    fn peek_tx_raw_segment(&self, idle_ms: u64) -> Vec<u16> {
        let count = self.segment_symbol_count(idle_ms);
        self.tx_queue
            .iter()
            .take(count)
            .map(|entry| entry.symbol)
            .collect()
    }

    fn segment_symbol_count(&self, idle_ms: u64) -> usize {
        let idle_ns = idle_ms.saturating_mul(NS_PER_MILLISECOND);
        let mut iter = self.tx_queue.iter();
        let Some(first) = iter.next() else {
            return 0;
        };
        let mut count = 1;
        let mut previous_completed_at_ns = first.completed_at_ns;
        for entry in iter {
            let idle_gap_ns = entry.started_at_ns.saturating_sub(previous_completed_at_ns);
            if idle_gap_ns >= idle_ns {
                break;
            }
            count += 1;
            previous_completed_at_ns = entry.completed_at_ns;
        }
        count
    }

    fn symbols_to_string(symbols: &[u16]) -> Result<String> {
        let mut bytes = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let byte = u8::try_from(*symbol).map_err(|_| {
                anyhow::anyhow!("当前串口包含超过 8 位的数据, 请改用 uart*_take_raw()")
            })?;
            bytes.push(byte);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn tx_text(&self) -> String {
        if self
            .tx_queue
            .iter()
            .all(|entry| entry.symbol <= u16::from(u8::MAX))
        {
            let bytes = self
                .tx_queue
                .iter()
                .map(|entry| entry.symbol as u8)
                .collect::<Vec<_>>();
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        self.tx_queue
            .iter()
            .map(|entry| format!("0x{:X}", entry.symbol))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn push_event_note<F>(&mut self, time_ns: u64, build: F)
    where
        F: FnOnce() -> WaveEventNote,
    {
        if self.event_gate.need_direct_event(self.event_track, time_ns) {
            self.event_notes.push(build());
        }
    }

    fn tb8_set(&self) -> bool {
        self.control
            & if self.scon_addr == UART2_SFR_S2CON {
                S2CON_TB8
            } else {
                SCON_TB8
            }
            != 0
    }

    fn irq_mask(&self) -> u8 {
        if self.scon_addr == UART2_SFR_S2CON {
            S2CON_RI | S2CON_TI
        } else {
            SCON_RI | SCON_TI
        }
    }

    fn irq_flags_high(&self) -> bool {
        self.control & self.irq_mask() != 0
    }

    fn set_rb8(&mut self, high: bool) {
        let rb8_flag = if self.scon_addr == UART2_SFR_S2CON {
            S2CON_RB8
        } else {
            SCON_RB8
        };
        if high {
            self.control |= rb8_flag;
        } else {
            self.control &= !rb8_flag;
        }
    }

    fn interrupt_requested(&self) -> bool {
        self.interrupt_requested
    }

    fn ack_interrupt(&mut self) {
        self.interrupt_requested = false;
        self.interrupt_reassert_countdown = None;
    }

    fn arm_interrupt_reassert(&mut self, instructions: u8) {
        if instructions == 0 || !self.irq_flags_high() || self.interrupt_requested {
            self.interrupt_reassert_countdown = None;
            return;
        }
        self.interrupt_reassert_countdown = Some(instructions);
    }

    fn advance_interrupt_reassert(&mut self) {
        let Some(remaining) = self.interrupt_reassert_countdown else {
            return;
        };
        if remaining > 1 {
            self.interrupt_reassert_countdown = Some(remaining - 1);
            return;
        }
        self.interrupt_reassert_countdown = None;
        if self.irq_flags_high() && !self.interrupt_requested {
            self.interrupt_requested = true;
        }
    }

    fn note_interrupt_flag_edges(&mut self, old_control: u8, new_control: u8) {
        let irq_mask = self.irq_mask();
        let old_flags = old_control & irq_mask;
        let new_flags = new_control & irq_mask;
        if new_flags == 0 {
            self.interrupt_reassert_countdown = None;
        }
        if new_flags & !old_flags != 0 {
            self.interrupt_requested = true;
            self.interrupt_reassert_countdown = None;
        }
    }
}

#[derive(Debug)]
struct BoardModel {
    cpu_cycles: u64,
    sim_time_ns: u64,
    sim_time_ns_remainder: u64,
    system_cycle_remainder: u64,
    outputs: Outputs,
    ds18b20: Ds18b20,
    ds1302: Ds1302,
    i2c: I2cBus,
    pcf8591: Pcf8591,
    at24c02: At24c02,
    ne555: Ne555,
    ultrasonic: UltrasonicDevice,
    keys: Key,
    key_mode: KeyMode,
    analog: AnalogInputs,
    jumpers: BoardJumpers,
    net_sig_sig_out_connected_since_ns: Option<u64>,
    p34_conflict_active: Cell<bool>,
}

impl BoardModel {
    #[cfg_attr(not(test), allow(dead_code))]
    fn new() -> Self {
        Self::new_with_wave_enabled(true)
    }

    fn new_with_wave_enabled(wave_enabled: bool) -> Self {
        Self::new_with_wave_window(WaveCaptureWindow::from_enabled(wave_enabled))
    }

    fn new_with_wave_window(wave_window: WaveCaptureWindow) -> Self {
        Self::new_with_event_gate(EventGate::shared(wave_window))
    }

    fn new_with_event_gate(event_gate: SharedEventGate) -> Self {
        Self {
            cpu_cycles: 0,
            sim_time_ns: 0,
            sim_time_ns_remainder: 0,
            system_cycle_remainder: 0,
            outputs: Outputs::default(),
            ds18b20: Ds18b20::new_with_event_gate(Arc::clone(&event_gate)),
            ds1302: Ds1302::new_with_event_gate(Arc::clone(&event_gate)),
            i2c: I2cBus,
            pcf8591: Pcf8591::new_with_event_gate(event_gate),
            at24c02: At24c02::default(),
            ne555: Ne555::default(),
            ultrasonic: UltrasonicDevice::default(),
            keys: Key::default(),
            key_mode: KeyMode::default(),
            analog: AnalogInputs::default(),
            jumpers: BoardJumpers::default(),
            net_sig_sig_out_connected_since_ns: None,
            p34_conflict_active: Cell::new(false),
        }
    }

    fn persistent_state(&self) -> PersistentState {
        PersistentState {
            ds18b20: self.ds18b20.persistent_state(),
            ds1302: self.ds1302.persistent_state(),
            at24c02: self.at24c02.persistent_state(),
        }
    }

    fn load_persistent_state(&mut self, state: &PersistentState) {
        self.ds18b20.load_persistent_state(&state.ds18b20);
        self.ds1302.load_persistent_state(&state.ds1302);
        self.at24c02.load_persistent_state(&state.at24c02);
    }

    fn retained_state(&self) -> BoardRetainedState {
        BoardRetainedState {
            persistent_state: self.persistent_state(),
            keys: self.keys.clone(),
            key_mode: self.key_mode,
            analog: self.analog.clone(),
            jumpers: self.jumpers.clone(),
            ds18b20_temperature_c: self.ds18b20.temperature_c,
            ds18b20_parasite_power: self.ds18b20.parasite_power(),
            ultrasonic_distance_cm: self.ultrasonic.distance_cm,
            ne555_frequency_hz: self.ne555.frequency_hz(),
        }
    }

    fn load_retained_state(&mut self, state: &BoardRetainedState) {
        self.load_persistent_state(&state.persistent_state);
        self.keys = state.keys.clone();
        self.key_mode = state.key_mode;
        self.analog = state.analog.clone();
        self.jumpers = state.jumpers.clone();
        self.net_sig_sig_out_connected_since_ns = self
            .jumpers
            .is_installed(SignalId::NetSig, SignalId::SigOut)
            .then_some(self.sim_time_ns);
        self.ds18b20.temperature_c = state.ds18b20_temperature_c;
        self.ds18b20
            .set_parasite_power(state.ds18b20_parasite_power);
        self.ultrasonic.distance_cm = state.ultrasonic_distance_cm.max(0.0);
        self.ne555
            .set_frequency_hz_at(self.sim_time_ns, state.ne555_frequency_hz);
    }

    fn advance_cycles(&mut self, cycles: u64) -> (u64, u32) {
        self.cpu_cycles = self.cpu_cycles.saturating_add(cycles);
        let total_ns = self
            .sim_time_ns_remainder
            .saturating_add(cycles.saturating_mul(NS_PER_SECOND));
        let elapsed_ns = total_ns / CPU_EXEC_HZ;
        self.sim_time_ns_remainder = total_ns % CPU_EXEC_HZ;
        self.sim_time_ns = self.sim_time_ns.saturating_add(elapsed_ns);
        self.ds1302.tick_ns(elapsed_ns);
        let total_system_cycles = self
            .system_cycle_remainder
            .saturating_add(cycles.saturating_mul(SYSTEM_HZ));
        let system_cycles = (total_system_cycles / CPU_EXEC_HZ).min(u64::from(u32::MAX)) as u32;
        self.system_cycle_remainder = total_system_cycles % CPU_EXEC_HZ;
        (elapsed_ns, system_cycles)
    }

    fn tick_protocols(
        &mut self,
        ports: &MachinePorts,
        board_latches: &[u8; 4],
        board_latch_versions: &[u64; 4],
    ) {
        let p1 = ports.port_latch[1];
        let p2 = ports.port_latch[2];
        self.ds1302.sample(
            self.sim_time_ns,
            (p1 & (1 << 3)) != 0,
            (p1 & (1 << 7)) != 0,
            (p2 & (1 << 3)) != 0,
        );
        self.ds18b20.sample(self.sim_time_ns, (p1 & (1 << 4)) != 0);
        self.i2c.sample(
            self.sim_time_ns,
            (p2 & (1 << 0)) != 0,
            (p2 & (1 << 1)) != 0,
            &self.analog,
            &mut self.pcf8591,
            &mut self.at24c02,
        );
        self.outputs
            .sample_from_latches(board_latches, board_latch_versions, self.sim_time_ns);
    }

    fn apply_i2c_lines(&self, mut value: u8) -> u8 {
        let (scl_high, sda_high) = self.read_i2c_lines(value);
        value = set_bit_level(value, 0, scl_high);
        value = set_bit_level(value, 1, sda_high);
        value
    }

    fn read_i2c_lines(&self, latch: u8) -> (bool, bool) {
        let (slave_scl_low, slave_sda_low) =
            self.i2c.slave_drives_low(&self.pcf8591, &self.at24c02);
        self.i2c.line_levels(
            (latch & (1 << 0)) != 0,
            (latch & (1 << 1)) != 0,
            slave_scl_low,
            slave_sda_low,
        )
    }

    fn read_ports(&self, latches: &[u8; 6]) -> [u8; 6] {
        let mut values = *latches;
        values[1] = self.read_port1(latches[1]);
        values[2] = self.read_port2(latches[2]);
        match self.key_mode {
            KeyMode::Keyboard => {
                let lows = self.keys.keyboard_lows(latches);
                values[3] =
                    self.read_port3_with_keyboard_lows(latches[3], latches, &lows, lows.cols[3]);
                values[4] = self.read_port4_with_keyboard_lows(latches[4], &lows);
            }
            KeyMode::Button => {
                let row_lows = self.keys.button_row_lows();
                values[3] = self.read_port3_with_button_lows(latches[3], latches, &row_lows);
            }
        }
        values
    }

    fn read_port(&self, index: usize, latch: u8, all_latches: &[u8; 6]) -> u8 {
        match index {
            1 => self.read_port1(latch),
            2 => self.read_port2(latch),
            3 => match self.key_mode {
                KeyMode::Keyboard => {
                    let lows = self.keys.keyboard_lows(all_latches);
                    self.read_port3_with_keyboard_lows(latch, all_latches, &lows, lows.cols[3])
                }
                KeyMode::Button => self.read_port3_with_button_lows(
                    latch,
                    all_latches,
                    &self.keys.button_row_lows(),
                ),
            },
            4 if self.key_mode == KeyMode::Keyboard => {
                self.read_port4_with_keyboard_lows(latch, &self.keys.keyboard_lows(all_latches))
            }
            _ => latch,
        }
    }

    fn read_port1(&self, latch: u8) -> u8 {
        let mut value = latch;
        value = apply_open_drain_bit(value, 4, self.ds18b20.drive_low);
        value = apply_push_pull_bit(value, 0, true);
        apply_push_pull_bit(value, 1, self.ultrasonic.rx_level())
    }

    fn read_port2(&self, latch: u8) -> u8 {
        let mut value = self.apply_i2c_lines(latch);
        value = apply_push_pull_bit(value, 3, self.ds1302.io_level);
        apply_push_pull_bit(value, 4, self.read_hall_level())
    }

    fn read_port3_base(&self, latch: u8, all_latches: &[u8; 6]) -> u8 {
        let value = set_bit_level(latch, 4, self.read_p34_level(latch, all_latches, None));
        set_bit_level(value, 5, true)
    }

    fn read_port3_with_keyboard_lows(
        &self,
        latch: u8,
        all_latches: &[u8; 6],
        lows: &KeyboardLows,
        col4_low: bool,
    ) -> u8 {
        let mut value = self.read_port3_base_with_col4_low(latch, all_latches, col4_low);
        for bit in 0..4 {
            value = set_bit_level(value, bit as u8, !lows.rows[bit]);
        }
        value
    }

    fn read_port3_base_with_col4_low(
        &self,
        latch: u8,
        all_latches: &[u8; 6],
        col4_low: bool,
    ) -> u8 {
        let value = set_bit_level(
            latch,
            4,
            self.read_p34_level(latch, all_latches, Some(col4_low)),
        );
        set_bit_level(value, 5, true)
    }

    fn read_port3_with_button_lows(
        &self,
        latch: u8,
        all_latches: &[u8; 6],
        row_lows: &[bool; 4],
    ) -> u8 {
        let mut value = self.read_port3_base(latch, all_latches);
        for (bit, low) in row_lows.iter().copied().enumerate() {
            value = set_bit_level(value, bit as u8, !low);
        }
        value
    }

    fn read_port4_with_keyboard_lows(&self, latch: u8, lows: &KeyboardLows) -> u8 {
        let mut value = latch;
        value = apply_open_drain_bit(value, 2, lows.cols[1]);
        apply_open_drain_bit(value, 4, lows.cols[0])
    }

    fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.keys.set_key(name, pressed)
    }

    fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.analog.set_voltage(name, value)
    }

    fn jumper_on(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.jumpers.install(left, right)?;
        if BoardJumpers::is_cap_pair(left, right, SignalId::NetSig, SignalId::SigOut)
            && self.net_sig_sig_out_connected_since_ns.is_none()
        {
            self.net_sig_sig_out_connected_since_ns = Some(self.sim_time_ns);
        }
        Ok(())
    }

    fn jumper_off(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.jumpers.remove(left, right)?;
        if BoardJumpers::is_cap_pair(left, right, SignalId::NetSig, SignalId::SigOut) {
            self.net_sig_sig_out_connected_since_ns = None;
        }
        Ok(())
    }

    fn jumper_installed(&self, left: SignalId, right: SignalId) -> bool {
        self.jumpers.is_installed(left, right)
    }

    fn frequency_level(&self) -> bool {
        self.ne555.level(self.sim_time_ns)
    }

    fn t0_transitions(
        &self,
        all_latches: &[u8; 6],
        start_time_ns: u64,
        end_time_ns: u64,
    ) -> SignalTransitionIter {
        if end_time_ns <= start_time_ns {
            return SignalTransitionIter::empty();
        }
        let Some(connected_since_ns) = self.net_sig_sig_out_connected_since_ns else {
            return SignalTransitionIter::empty();
        };
        if all_latches[3] & (1 << 4) == 0 {
            return SignalTransitionIter::empty();
        }
        if self.key_mode == KeyMode::Keyboard && self.keys.col_low(3, all_latches) {
            return SignalTransitionIter::empty();
        }

        let effective_start_ns = start_time_ns.max(connected_since_ns);
        if end_time_ns <= effective_start_ns {
            return SignalTransitionIter::empty();
        }

        self.ne555
            .transitions_between(effective_start_ns, end_time_ns)
    }

    fn read_hall_level(&self) -> bool {
        true
    }

    fn read_p34_level(&self, latch: u8, all_latches: &[u8; 6], col4_low: Option<bool>) -> bool {
        let key_col4_low = self.key_mode == KeyMode::Keyboard
            && col4_low.unwrap_or_else(|| self.keys.col_low(3, all_latches));
        let drivers = [
            (
                "mcu.p3.4",
                if latch & (1 << 4) != 0 {
                    LineDrive::PullHigh
                } else {
                    LineDrive::DriveLow
                },
            ),
            (
                "key.col4",
                if key_col4_low {
                    LineDrive::DriveLow
                } else {
                    LineDrive::HighZ
                },
            ),
            (
                "ne555.net_sig",
                if self.jumper_installed(SignalId::NetSig, SignalId::SigOut) {
                    if self.frequency_level() {
                        LineDrive::DriveHigh
                    } else {
                        LineDrive::DriveLow
                    }
                } else {
                    LineDrive::HighZ
                },
            ),
        ];
        let resolution = resolve_line(&drivers.map(|(_, drive)| drive));
        self.update_p34_conflict(&drivers, resolution.conflict);
        resolution.level
    }

    fn update_p34_conflict(&self, drivers: &[(&str, LineDrive); 3], conflict: bool) {
        if conflict {
            if !self.p34_conflict_active.replace(true) {
                let low = drivers
                    .iter()
                    .filter_map(|(name, drive)| (*drive == LineDrive::DriveLow).then_some(*name))
                    .collect::<Vec<_>>()
                    .join(",");
                let high = drivers
                    .iter()
                    .filter_map(|(name, drive)| (*drive == LineDrive::DriveHigh).then_some(*name))
                    .collect::<Vec<_>>()
                    .join(",");
                warn!(
                    line = "P3.4/SIG_OUT",
                    low, high, "检测到线路驱动冲突, 按低电平处理"
                );
            }
        } else {
            self.p34_conflict_active.set(false);
        }
    }
}

impl Default for BoardModel {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_open_drain_bit(mut value: u8, bit: u8, device_drive_low: bool) -> u8 {
    let mcu_high = value & (1 << bit) != 0;
    let high = mcu_high && !device_drive_low;
    value = set_bit_level(value, bit, high);
    value
}

fn apply_push_pull_bit(mut value: u8, bit: u8, level: bool) -> u8 {
    value = set_bit_level(value, bit, level);
    value
}

fn set_bit_level(value: u8, bit: u8, high: bool) -> u8 {
    if high {
        value | (1 << bit)
    } else {
        value & !(1 << bit)
    }
}

fn apply_external_interrupt_flag(
    tcon: u8,
    trigger_select_mask: u8,
    request_flag_mask: u8,
    pin_mask: u8,
    prev_p3: u8,
    next_p3: u8,
) -> u8 {
    let prev_high = prev_p3 & pin_mask != 0;
    let next_high = next_p3 & pin_mask != 0;
    if prev_high == next_high {
        return tcon;
    }

    let edge_triggered = if tcon & trigger_select_mask != 0 {
        prev_high && !next_high
    } else {
        true
    };

    if edge_triggered {
        tcon | request_flag_mask
    } else {
        tcon
    }
}

fn parse_display_number(text: &str) -> Result<DisplayNumber> {
    let value = extract_unique_numeric_token(text, true)?;
    if value.contains('.') {
        return value
            .parse::<f64>()
            .map(DisplayNumber::Float)
            .map_err(|err| anyhow::anyhow!("解析显示浮点数失败: {err}"));
    }
    value
        .parse::<i64>()
        .map(DisplayNumber::Integer)
        .map_err(|err| anyhow::anyhow!("解析显示整数失败: {err}"))
}

fn extract_unique_numeric_token(text: &str, allow_decimal: bool) -> Result<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut numbers = Vec::new();
    let mut index = 0_usize;

    while index < chars.len() {
        let ch = chars[index];
        let starts_negative = ch == '-'
            && chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit());
        if !ch.is_ascii_digit() && !starts_negative {
            index += 1;
            continue;
        }

        let mut current = String::new();
        let mut has_dot = false;
        if starts_negative {
            current.push('-');
            index += 1;
        }

        while index < chars.len() {
            let ch = chars[index];
            if ch.is_ascii_digit() {
                current.push(ch);
                index += 1;
                continue;
            }
            if allow_decimal
                && ch == '.'
                && !has_dot
                && chars
                    .get(index + 1)
                    .is_some_and(|next| next.is_ascii_digit())
            {
                has_dot = true;
                current.push('.');
                index += 1;
                continue;
            }
            break;
        }

        if current != "-" {
            numbers.push(current);
        }
    }

    match numbers.as_slice() {
        [value] => Ok(value.clone()),
        [] => bail!("显示内容中没有可解析的数字: `{text}`"),
        _ => bail!("显示内容中包含多个数字, 无法唯一提取: `{text}`"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use i8051::{Cpu, Register};

    use crate::{
        event::track::EventTrack,
        ids::{KeyId, KeyMode, LedId, ResetMode, SignalId, VoltageChannel},
        peripherals::{I2cSlaveDevice, SignalEdge},
        wave::WaveCaptureOptions,
    };

    use super::{DisplayNumber, LedWatchStats, NS_PER_SECOND, Simulator};

    fn sample_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn default_port_latches() -> [u8; 6] {
        let mut latches = [0xFF_u8; 6];
        latches[5] = 0x3F;
        latches
    }

    fn p34_level(board: &super::BoardModel, all_latches: [u8; 6]) -> bool {
        board.read_port(3, all_latches[3], &all_latches) & (1 << 4) != 0
    }

    #[test]
    fn led_flicker_counts_expected_toggles_per_second() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/led_flicker/prj/Objects/led_flicker.hex"),
            false,
        )
        .expect("load led_flicker");

        sim.run_ms(20).expect("run to 20ms");
        let stats = sim
            .watch_led_stats(LedId::L1, 1000)
            .expect("watch L1 stats");
        assert!(
            (9..=11).contains(&stats.changes),
            "expected about 10 changes, got {}",
            stats.changes
        );
        let change_frequency_hz = stats
            .change_frequency_hz()
            .expect("measure change frequency")
            .expect("stable flicker should have valid change frequency");
        assert!(
            (9.5..=10.5).contains(&change_frequency_hz),
            "expected about 10Hz change frequency, got {change_frequency_hz}"
        );
    }

    #[test]
    fn change_frequency_ignores_stable_head_and_tail_gaps() {
        let stats = LedWatchStats {
            observed_time_ns: NS_PER_SECOND,
            changes: 4,
            change_intervals_ns: vec![100_000_000, 100_000_000, 100_000_000],
            ..LedWatchStats::default()
        };

        let change_frequency_hz = stats
            .change_frequency_hz()
            .expect("measure change frequency")
            .expect("uniform intervals should produce valid change frequency");
        assert!(
            (9.9..=10.1).contains(&change_frequency_hz),
            "expected about 10Hz after ignoring stable head and tail, got {change_frequency_hz}"
        );
    }

    #[test]
    fn change_frequency_is_zero_when_led_never_changes() {
        let stats = LedWatchStats {
            observed_time_ns: NS_PER_SECOND,
            ..LedWatchStats::default()
        };

        let change_frequency_hz = stats
            .change_frequency_hz()
            .expect("measure change frequency")
            .expect("steady LED should map to 0Hz");
        assert_eq!(
            change_frequency_hz, 0.0,
            "steady LED should report 0Hz instead of NaN"
        );
    }

    #[test]
    fn change_frequency_is_invalid_when_mid_window_intervals_diverge_too_much() {
        let stats = LedWatchStats {
            observed_time_ns: NS_PER_SECOND,
            changes: 4,
            change_intervals_ns: vec![100_000_000, 100_000_000, 400_000_000],
            ..LedWatchStats::default()
        };

        assert!(
            stats
                .change_frequency_hz()
                .expect("measure change frequency")
                .is_none(),
            "widely different mid-window intervals should not yield a numeric frequency"
        );
    }

    #[test]
    fn led_pwm_reports_expected_frequency_and_duty() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/led_pwm/prj/Objects/led_pwm.hex"),
            false,
        )
        .expect("load led_pwm");

        sim.run_ms(220).expect("run to stable display");
        assert_eq!(sim.display_text(), "000");

        let initial_stats = sim
            .watch_led_stats(LedId::L1, 40)
            .expect("watch initial stats");
        let initial_freq = initial_stats
            .pwm_frequency_hz()
            .expect("measure initial pwm frequency");
        assert!(
            (950.0..=1_050.0).contains(&initial_freq),
            "expected about 1kHz, got {initial_freq}"
        );
        let initial_duty = initial_stats.duty_percent().expect("measure initial duty");
        assert!(
            (8.0..=12.0).contains(&initial_duty),
            "expected about 10% duty, got {initial_duty}"
        );

        sim.tap_key_id(KeyId::S9, 80).expect("tap S9");
        sim.run_ms(120).expect("wait display refresh");
        assert_eq!(sim.display_text(), "001");

        let increased_stats = sim
            .watch_led_stats(LedId::L1, 40)
            .expect("watch increased stats");
        let increased_freq = increased_stats
            .pwm_frequency_hz()
            .expect("measure increased pwm frequency");
        assert!(
            (950.0..=1_050.0).contains(&increased_freq),
            "expected about 1kHz after S9, got {increased_freq}"
        );
        let increased_duty = increased_stats
            .duty_percent()
            .expect("measure increased duty");
        assert!(
            (18.0..=22.0).contains(&increased_duty),
            "expected about 20% duty after S9, got {increased_duty}"
        );
    }

    #[test]
    fn display_text_window_extends_wait_after_mid_window_change() {
        let code = vec![
            0x75, 0x80, 0x01, // MOV P0, #0x01
            0x75, 0xA0, 0xC0, // MOV P2, #0xC0
            0x75, 0x80, 0xA4, // MOV P0, #0xA4
            0x75, 0xA0, 0xE0, // MOV P2, #0xE0
            0x80, 0xFE, // SJMP $
        ];
        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        assert_eq!(sim.display_text(), "");

        let start_ns = sim.sim_time_ns();
        let text = sim
            .observe_display_text(30)
            .expect("wait for display after mid-window change");
        assert_eq!(text, "2");
        assert_eq!(sim.display_text(), "2");
        assert!(sim.sim_time_ns().saturating_sub(start_ns) > 30_000_000);
    }

    #[test]
    fn run_to_ns_advances_to_absolute_timestamp() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        let elapsed_ns = sim.run_to_ns(1_000_000).expect("run to 1ms");
        assert!(elapsed_ns >= 1_000_000);
        assert_eq!(sim.sim_time_ns(), elapsed_ns);
    }

    #[test]
    fn run_to_target_detects_ne555_flip() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        sim.set_frequency_hz(2_000.0);
        let elapsed_ns = sim
            .run_to_target(super::RunToTarget::Ne555SigOut, super::RunToEdge::Flip)
            .expect("wait ne555 flip");
        assert!(elapsed_ns > 0);
        assert!(sim.sim_time_ns() >= elapsed_ns);
    }

    #[test]
    fn net_sig_requires_explicit_jumper_to_reach_p34() {
        let mut board = super::BoardModel::default();
        let latches = default_port_latches();
        board.ne555.set_frequency_hz_at(0, 2_200.0);

        let saw_low_without_bridge = (0..20_000_u64).any(|index| {
            board.sim_time_ns = index * 100;
            !p34_level(&board, latches)
        });
        assert!(!saw_low_without_bridge);

        board
            .jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        let saw_low_with_bridge = (0..20_000_u64).any(|index| {
            board.sim_time_ns = index * 100;
            !p34_level(&board, latches)
        });
        assert!(saw_low_with_bridge);
    }

    #[test]
    fn t0_transitions_require_sig_out_jumper() {
        let mut board = super::BoardModel::default();
        let latches = default_port_latches();
        board.ne555.set_frequency_hz_at(0, 2_200.0);
        board.sim_time_ns = 1_000_000;

        assert!(
            board
                .t0_transitions(&latches, 0, 1_000_000)
                .next()
                .is_none()
        );

        board
            .jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        assert!(
            board
                .t0_transitions(&latches, 0, 1_000_000)
                .next()
                .is_none()
        );

        board.sim_time_ns = 2_000_000;
        let transitions: Vec<_> = board
            .t0_transitions(&latches, 1_000_000, 2_000_000)
            .collect();
        assert!(
            transitions
                .iter()
                .any(|transition| transition.edge == SignalEdge::Falling)
        );
        assert!(
            transitions
                .iter()
                .any(|transition| transition.edge == SignalEdge::Rising)
        );
    }

    #[test]
    fn timer0_counter_mode_counts_only_while_sig_out_jumper_is_installed() {
        let mut sim = Simulator::nop(false);
        sim.set_frequency_hz(1_000.0);
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TMOD,
            super::TMOD_C_T0 | 0x01,
        ));
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TCON,
            super::TCON_TR0,
        ));

        sim.tick_devices(12_000)
            .expect("advance 1ms without SIG_OUT jumper");
        let snapshot = sim.ctx.ports.timers.snapshot(&sim.ctx.ports.generic);
        assert_eq!(snapshot.th0, 0);
        assert_eq!(snapshot.tl0, 0);

        sim.jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        sim.tick_devices(12_000)
            .expect("advance 1ms with SIG_OUT jumper");
        let snapshot = sim.ctx.ports.timers.snapshot(&sim.ctx.ports.generic);
        assert_eq!(snapshot.th0, 0);
        assert_eq!(snapshot.tl0, 1);

        sim.jumper_off(SignalId::NetSig, SignalId::SigOut)
            .expect("remove NET_SIG to SIG_OUT jumper");
        sim.tick_devices(12_000)
            .expect("advance 1ms after removing SIG_OUT jumper");
        let snapshot = sim.ctx.ports.timers.snapshot(&sim.ctx.ports.generic);
        assert_eq!(snapshot.th0, 0);
        assert_eq!(snapshot.tl0, 1);
    }

    #[test]
    fn power_reset_keeps_sig_out_jumper_without_replaying_old_t0_edges() {
        let mut sim = Simulator::nop(false);
        sim.set_frequency_hz(1_000.0);
        sim.jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TMOD,
            super::TMOD_C_T0 | 0x01,
        ));
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TCON,
            super::TCON_TR0,
        ));

        sim.tick_devices(12_000)
            .expect("advance 1ms before power reset");
        let snapshot = sim.ctx.ports.timers.snapshot(&sim.ctx.ports.generic);
        assert_eq!(snapshot.tl0, 1);

        sim.reset_with_mode(ResetMode::Power)
            .expect("power reset simulator");
        assert!(
            sim.ctx
                .board
                .jumper_installed(SignalId::NetSig, SignalId::SigOut)
        );
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TMOD,
            super::TMOD_C_T0 | 0x01,
        ));
        assert!(sim.ctx.ports.timers.write(
            &mut sim.ctx.ports.generic,
            super::SFR_TCON,
            super::TCON_TR0,
        ));

        sim.tick_devices(12_000)
            .expect("advance 1ms after power reset");
        let snapshot = sim.ctx.ports.timers.snapshot(&sim.ctx.ports.generic);
        assert_eq!(snapshot.th0, 0);
        assert_eq!(snapshot.tl0, 1);
    }

    #[test]
    fn p34_conflict_prefers_low_when_key_column_and_ne555_disagree() {
        let mut board = super::BoardModel::default();
        let mut latches = default_port_latches();
        board.ne555.set_frequency_hz_at(0, 2_200.0);
        board
            .jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        board.keys.set_key_id(KeyId::S16, true);
        latches[3] &= !(1 << 3);
        board.sim_time_ns = 0;

        assert!(!p34_level(&board, latches));
    }

    #[test]
    fn i2c_lines_follow_wired_and_levels() {
        let mut board = super::BoardModel::default();

        assert_eq!(board.read_i2c_lines(0xFF), (true, true));

        board.pcf8591.force_lines_for_test(false, true);
        assert_eq!(board.read_i2c_lines(0xFF), (true, false));

        board.pcf8591.force_lines_for_test(true, true);
        assert_eq!(board.read_i2c_lines(0xFF), (false, false));

        board.pcf8591.force_lines_for_test(false, false);
        assert_eq!(board.read_i2c_lines(0xFC), (false, false));
    }

    #[test]
    fn p2_bit_read_uses_current_pin_level() {
        let code = vec![
            0xA2, 0xA1, // MOV C, P2.1
            0x92, 0x90, // MOV P1.0, C
            0x80, 0xFE, // SJMP $
        ];
        let mut cpu = Cpu::new();
        let mut ctx = super::MachineContext::new(code);
        ctx.board.pcf8591.force_lines_for_test(false, true);
        ctx.ports.refresh_inputs(&ctx.board);

        let _ = cpu.step(&mut ctx);
        let _ = cpu.step(&mut ctx);

        assert_eq!(ctx.ports.port_latch[1] & 0x01, 0x00);
    }

    #[test]
    fn interrupt_entry_consumes_cycles_before_vector_opcode_runs() {
        let mut code = vec![0x00; 0x20];
        code[0x1B] = 0xC2;
        code[0x1C] = 0x90;
        code[0x1D] = 0x32;

        let event_gate =
            super::EventGate::shared(crate::wave::WaveCaptureWindow::from_enabled(true));
        let mut sim = super::Simulator {
            cpu: Cpu::new(),
            ctx: super::MachineContext::new_with_event_gate(
                code.clone(),
                std::sync::Arc::clone(&event_gate),
            ),
            code_image: code,
            trace_cpu: false,
            interrupt_poll_blocked_instructions: 0,
            active_interrupts: Vec::new(),
            seg_decoder: super::SegmentDecoder::default(),
            wave: crate::wave::WaveRecorder::new(crate::wave::WaveCaptureOptions::default()),
            event_gate,
            i2c_event_decoder: super::I2cEventDecoder::default(),
            seg_event_detector: super::SegEventDetector::default(),
            script_event_tracker: super::EventTracker::default(),
        };
        sim.cpu
            .register_set(Register::IE, u16::from(super::IE_EA | super::IE_ET1));
        sim.ctx
            .ports
            .timers
            .write(&mut sim.ctx.ports.generic, super::SFR_TCON, super::TCON_TF1);

        sim.step_once().expect("enter timer1 interrupt");

        assert_eq!(sim.cpu.pc, 0x001B);
        assert_eq!(sim.ctx.ports.port_latch[1] & 0x01, 0x01);
        assert_eq!(
            sim.ctx
                .ports
                .timers
                .read(&sim.ctx.ports.generic, super::SFR_TCON),
            Some(0x00)
        );
        assert_eq!(
            sim.ctx.board.cpu_cycles,
            u64::from(super::INTERRUPT_ENTRY_TICKS)
        );

        sim.step_once().expect("run timer1 vector instruction");

        assert_eq!(sim.ctx.ports.port_latch[1] & 0x01, 0x00);
    }

    #[test]
    fn ext0_interrupt_uses_ie0_and_falling_edge_mode() {
        let mut sim = Simulator::nop(false);
        sim.key_mode(KeyMode::Button);
        sim.cpu
            .register_set(Register::IE, u16::from(super::IE_EA | super::IE_EX0));
        sim.cpu
            .sfr_set(super::SFR_TCON, super::TCON_IT0, &mut sim.ctx);

        sim.set_key("S5", true).expect("press S5 to pull INT0 low");
        assert_eq!(
            sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE0,
            super::TCON_IE0
        );

        sim.step_once().expect("enter ext0 interrupt");

        assert_eq!(sim.cpu.pc, 0x0003);
        assert_eq!(sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE0, 0);

        sim.set_key("S5", false)
            .expect("release S5 should raise INT0");
        assert_eq!(
            sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE0,
            0,
            "IT0=1 should ignore the rising edge"
        );
    }

    #[test]
    fn ext1_interrupt_uses_ie1_and_both_edge_mode_when_it1_is_clear() {
        let mut code = vec![0x00; 0x14];
        code[0x13] = 0x32;
        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        sim.key_mode(KeyMode::Button);
        sim.cpu
            .register_set(Register::IE, u16::from(super::IE_EA | super::IE_EX1));

        sim.set_key("S4", true).expect("press S4 to pull INT1 low");
        assert_eq!(
            sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE1,
            super::TCON_IE1
        );

        sim.step_once().expect("enter ext1 interrupt");
        assert_eq!(sim.cpu.pc, 0x0013);
        assert_eq!(sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE1, 0);

        sim.step_once().expect("execute ext1 RETI");
        sim.set_key("S4", false)
            .expect("release S4 should raise INT1");
        assert_eq!(
            sim.cpu.sfr(super::SFR_TCON, &sim.ctx) & super::TCON_IE1,
            super::TCON_IE1,
            "IT1=0 should accept the rising edge too"
        );
    }

    #[test]
    fn serial_interrupt_does_not_reassert_before_ti_can_clear() {
        let mut code = vec![0x00; 0x24];
        code[0x00] = 0x75;
        code[0x01] = 0xA8;
        code[0x02] = super::IE_EA | super::IE_ES;
        code[0x03] = 0xD2;
        code[0x04] = 0x99;
        code[0x05] = 0x00;
        code[0x06] = 0xC2;
        code[0x07] = 0x99;
        code[0x08] = 0x80;
        code[0x09] = 0xFE;
        code[0x23] = 0x32;

        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        sim.step_once().expect("enable serial interrupt");
        sim.step_once().expect("set TI pending");
        sim.step_once().expect("enter serial interrupt");

        assert_eq!(sim.cpu.pc, 0x0023);

        sim.step_once().expect("execute RETI");
        assert_eq!(sim.cpu.pc, 0x0005);
        assert_eq!(sim.interrupt_poll_blocked_instructions, 1);

        sim.step_once()
            .expect("run first main instruction before serial reentry");

        assert_eq!(sim.cpu.pc, 0x0006);
        assert_eq!(sim.interrupt_poll_blocked_instructions, 0);

        sim.step_once()
            .expect("clear TI before serial interrupt can reassert");

        assert_eq!(sim.cpu.pc, 0x0008);
        assert_eq!(
            sim.cpu.sfr(super::SFR_SCON, &sim.ctx) & super::SCON_TI,
            0,
            "TI should still be clear before serial interrupt reentry"
        );

        sim.step_once()
            .expect("same TI level should stay quiet after it clears");
        assert_eq!(sim.cpu.pc, 0x0008);
    }

    #[test]
    fn serial_interrupt_reasserts_if_ti_stays_high_for_two_post_reti_instructions() {
        let mut code = vec![0x00; 0x26];
        code[0x00] = 0x75;
        code[0x01] = 0xA8;
        code[0x02] = super::IE_EA | super::IE_ES;
        code[0x03] = 0xD2;
        code[0x04] = 0x99;
        code[0x05] = 0x00;
        code[0x06] = 0x00;
        code[0x07] = 0xC2;
        code[0x08] = 0x99;
        code[0x09] = 0x80;
        code[0x0A] = 0xFE;
        code[0x23] = 0x05;
        code[0x24] = 0x30;
        code[0x25] = 0x32;

        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        sim.step_once().expect("enable serial interrupt");
        sim.step_once().expect("set TI pending");
        sim.step_once().expect("enter serial interrupt");
        sim.step_once().expect("increment ISR counter");
        sim.step_once().expect("execute RETI");

        assert_eq!(sim.cpu.pc, 0x0005);
        assert_eq!(sim.cpu.internal_ram(0x30), 1);

        sim.step_once()
            .expect("run first post-RETI instruction with TI still high");
        assert_eq!(sim.cpu.pc, 0x0006);

        sim.step_once()
            .expect("run second post-RETI instruction before reassert");
        assert_eq!(sim.cpu.pc, 0x0007);

        sim.step_once()
            .expect("reenter serial interrupt before TI clears");
        assert_eq!(sim.cpu.pc, 0x0023);

        sim.step_once().expect("increment ISR counter again");
        assert_eq!(sim.cpu.internal_ram(0x30), 2);
    }

    #[test]
    fn uart2_interrupt_uses_vector_8_and_reasserts_high_ri_after_delay() {
        let mut code = vec![0x00; 0x44];
        code[0x00] = 0x75;
        code[0x01] = 0xA8;
        code[0x02] = super::IE_EA;
        code[0x03] = 0x75;
        code[0x04] = 0xAF;
        code[0x05] = super::IE2_ES2;
        code[0x06] = 0x75;
        code[0x07] = 0x9A;
        code[0x08] = super::S2CON_RI;
        code[0x09] = 0x00;
        code[0x0A] = 0x00;
        code[0x0B] = 0x80;
        code[0x0C] = 0xFE;
        code[0x43] = 0x32;

        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        sim.step_once().expect("enable global interrupts");
        sim.step_once().expect("enable uart2 interrupt");
        sim.step_once().expect("set uart2 RI pending");
        sim.step_once().expect("enter uart2 interrupt");

        assert_eq!(sim.cpu.pc, 0x0043);

        sim.step_once().expect("execute uart2 RETI");
        assert_eq!(sim.cpu.pc, 0x0009);
        assert_eq!(sim.interrupt_poll_blocked_instructions, 1);

        sim.step_once()
            .expect("run one main instruction before uart2 reentry");

        assert_eq!(sim.cpu.pc, 0x000A);
        assert_eq!(sim.interrupt_poll_blocked_instructions, 0);
        assert_ne!(
            sim.cpu.pc, 0x0043,
            "uart2 interrupt should not reenter before one main instruction executes"
        );

        sim.step_once()
            .expect("same uart2 RI level should wait for one more instruction");
        assert_eq!(sim.cpu.pc, 0x000B);

        sim.step_once()
            .expect("same uart2 RI level should reenter after delayed reassert");
        assert_eq!(sim.cpu.pc, 0x0043);
    }

    #[test]
    fn uart1_default_text_io_echoes_bytes() {
        let code = vec![
            0x75, 0x98, 0x10, 0xE5, 0x98, 0x54, 0x01, 0x60, 0xFA, 0xE5, 0x99, 0x53, 0x98, 0xFE,
            0xF5, 0x99, 0x80, 0xF1,
        ];
        let mut sim = Simulator::from_code_with_options(
            code,
            false,
            crate::wave::WaveCaptureOptions::default(),
        );

        sim.uart_write(b"42").expect("inject uart1 bytes");
        sim.run_ms(20).expect("wait for uart1 echo");

        assert_eq!(sim.uart_take_string().expect("take uart1 text"), "42");
    }

    #[test]
    fn approximate_instruction_ticks_matches_stc15_common_gpio_ops() {
        assert_eq!(super::approximate_instruction_ticks(0xA2), 1);
        assert_eq!(super::approximate_instruction_ticks(0xC2), 1);
        assert_eq!(super::approximate_instruction_ticks(0xD2), 1);
        assert_eq!(super::approximate_instruction_ticks(0xE5), 1);
        assert_eq!(super::approximate_instruction_ticks(0xF5), 1);
        assert_eq!(super::approximate_instruction_ticks(0x25), 1);
        assert_eq!(super::approximate_instruction_ticks(0x35), 1);
        assert_eq!(super::approximate_instruction_ticks(0x95), 1);
        assert_eq!(super::approximate_instruction_ticks(0x92), 2);
        assert_eq!(super::approximate_instruction_ticks(0x12), 4);
        assert_eq!(super::approximate_instruction_ticks(0x22), 4);
        assert_eq!(super::approximate_instruction_ticks(0x30), 5);
        assert_eq!(super::approximate_instruction_ticks(0x40), 3);
        assert_eq!(super::approximate_instruction_ticks(0x50), 3);
        assert_eq!(super::approximate_instruction_ticks(0x60), 4);
        assert_eq!(super::approximate_instruction_ticks(0x70), 4);
        assert_eq!(super::approximate_instruction_ticks(0x80), 3);
        assert_eq!(super::approximate_instruction_ticks(0x53), 2);
        assert_eq!(super::approximate_instruction_ticks(0x88), 2);
        assert_eq!(super::approximate_instruction_ticks(0xA8), 2);
        assert_eq!(super::approximate_instruction_ticks(0x90), 2);
        assert_eq!(super::approximate_instruction_ticks(0xD5), 5);
        assert_eq!(super::approximate_instruction_ticks(0xD8), 4);
        assert_eq!(super::approximate_instruction_ticks(0x73), 5);
    }

    #[test]
    fn delay_sample_delay5ms_matches_run_to_timing() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("samples/delay/prj/Objects/delay.hex"), false)
                .expect("load delay");

        let mut startup_off_ns = None;
        while sim.sim_time_ns() <= 2 * super::NS_PER_MILLISECOND {
            let all_off = (1..=8).all(|led| !sim.led_on(led).expect("read led"));
            if all_off {
                startup_off_ns = Some(sim.sim_time_ns());
                break;
            }
            sim.step_once().expect("advance delay sample");
        }

        let startup_off_ns = startup_off_ns.expect("delay sample should clear boot leds");
        assert!(
            startup_off_ns <= 200_000,
            "expected boot leds to clear within 200us, got {startup_off_ns}ns"
        );

        let dt0 = sim
            .run_to_target(super::RunToTarget::Led(LedId::L1), super::RunToEdge::Up)
            .expect("wait L1 rise");
        assert!(
            (4_500_000..=5_500_000).contains(&dt0),
            "expected L1 step near 5ms, got {dt0}ns"
        );

        let dt1 = sim
            .run_to_target(super::RunToTarget::Led(LedId::L2), super::RunToEdge::Up)
            .expect("wait L2 rise");
        assert!(
            (4_500_000..=5_500_000).contains(&dt1),
            "expected L2 step near 5ms, got {dt1}ns"
        );
    }

    #[test]
    fn power_reset_clears_peripheral_volatile_state_but_keeps_persistent_data() {
        let mut sim = Simulator::nop(false);
        let analog = sim.ctx.board.analog.clone();

        assert!(sim.ctx.board.pcf8591.on_addressed_write(100, &analog));
        assert!(sim.ctx.board.pcf8591.on_write_byte(100, 0x03, &analog));
        assert!(sim.ctx.board.pcf8591.on_write_byte(100, 0xA5, &analog));

        assert!(sim.ctx.board.at24c02.on_addressed_write(200, &()));
        assert!(sim.ctx.board.at24c02.on_write_byte(200, 0x10, &()));
        assert!(sim.ctx.board.at24c02.on_write_byte(200, 0xAB, &()));
        sim.ctx.board.at24c02.on_i2c_stop(200, &());

        sim.run_us(100).expect("advance sim time");
        assert!(!sim.ctx.board.at24c02.on_addressed_write(201, &()));
        assert_eq!(sim.ctx.board.pcf8591.selected_channel(), 3);
        assert_eq!(sim.ctx.board.pcf8591.dac_value(), 0xA5);
        assert_eq!(sim.ctx.board.at24c02.byte(0x10), 0xAB);
        assert!(sim.sim_time_ns() > 0);
        let reset_time_ns = sim.sim_time_ns();

        sim.reset().expect("power reset");

        assert_eq!(sim.sim_time_ns(), reset_time_ns);
        assert_eq!(sim.ctx.board.pcf8591.selected_channel(), 0);
        assert_eq!(sim.ctx.board.pcf8591.dac_value(), 0x00);
        assert_eq!(sim.ctx.board.at24c02.byte(0x10), 0xAB);
        assert!(sim.ctx.board.at24c02.on_addressed_write(201, &()));
    }

    #[test]
    fn cpu_reset_preserves_peripheral_volatile_state_and_board_latches() {
        let mut sim = Simulator::nop(false);
        let analog = sim.ctx.board.analog.clone();

        assert!(sim.ctx.board.pcf8591.on_addressed_write(100, &analog));
        assert!(sim.ctx.board.pcf8591.on_write_byte(100, 0x02, &analog));
        assert!(sim.ctx.board.pcf8591.on_write_byte(100, 0x5C, &analog));

        assert!(sim.ctx.board.at24c02.on_addressed_write(200, &()));
        assert!(sim.ctx.board.at24c02.on_write_byte(200, 0x20, &()));
        assert!(sim.ctx.board.at24c02.on_write_byte(200, 0xCD, &()));
        sim.ctx.board.at24c02.on_i2c_stop(200, &());

        sim.ctx.ports.board_latches = [0xFE, 0x10, 0x00, 0x00];
        sim.ctx.ports.board_latch_versions = [1, 1, 0, 0];
        sim.ctx.ports.latch_used = true;
        sim.ctx.board.outputs.sample_from_latches(
            &sim.ctx.ports.board_latches,
            &sim.ctx.ports.board_latch_versions,
            sim.sim_time_ns(),
        );

        sim.run_us(100).expect("advance sim time");
        let before_reset_ns = sim.sim_time_ns();

        sim.reset_with_mode(ResetMode::Cpu).expect("cpu reset");

        assert_eq!(sim.sim_time_ns(), before_reset_ns);
        assert_eq!(sim.ctx.board.pcf8591.selected_channel(), 2);
        assert_eq!(sim.ctx.board.pcf8591.dac_value(), 0x5C);
        assert_eq!(sim.ctx.board.at24c02.byte(0x20), 0xCD);
        assert!(!sim.ctx.board.at24c02.on_addressed_write(201, &()));
        assert!(sim.ctx.ports.latch_used);
        assert_eq!(sim.ctx.ports.board_latches[0], 0xFE);
        assert!(sim.led_on(1).expect("read L1 after cpu reset"));
        assert!(sim.relay_on(), "relay latch should survive cpu reset");
    }

    #[test]
    fn power_reset_preserves_sim_time_ns() {
        let mut sim = Simulator::nop(false);
        sim.run_us(100).expect("advance sim time");
        let before_reset_ns = sim.sim_time_ns();

        sim.reset_with_mode(ResetMode::Power).expect("power reset");

        assert_eq!(sim.sim_time_ns(), before_reset_ns);
    }

    #[test]
    fn wave_disabled_does_not_buffer_uart_events() {
        let mut uart = super::Uart::new(
            super::UART1_SFR_SCON,
            super::UART1_SFR_SBUF,
            super::EventTrack::Uart1,
            super::EventGate::shared(super::WaveCaptureWindow::from_enabled(false)),
        );

        uart.write(super::UART1_SFR_SBUF, 0x55);
        uart.tick_ns(0, 1);

        assert!(uart.take_event_notes().is_empty());
    }

    #[test]
    fn uart_event_label_includes_printable_ascii_hint() {
        assert_eq!(super::uart_event_label("RX", b'4'.into()), "RX 0x34 '4'");
    }

    #[test]
    fn uart_event_label_escapes_control_ascii_hint() {
        assert_eq!(super::uart_event_label("TX", b'\n'.into()), r"TX 0x0A '\n'");
    }

    #[test]
    fn seg_change_event_tracks_ignore_refresh_without_effective_change() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        sim.run_ms(150).expect("run key_seg to idle");
        assert_eq!(sim.display_text(), "       0");
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegChange), 0);
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegDigit8), 0);

        sim.set_key("S4", true).expect("press S4 to increment D8");
        let event = sim
            .run_to_event_with_timeout(EventTrack::SegDigit8, Some(200_000_000))
            .expect("wait D8 effective segment change");
        assert_eq!(event.track_id, EventTrack::SegDigit8.track_id());
        assert_eq!(event.label, "D8 change");
        assert!(event.elapsed_ns <= 200_000_000);
        assert_eq!(sim.display_text(), "       1");
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegDigit8), 1);

        let err = sim
            .run_to_event_with_timeout(EventTrack::SegDigit8, Some(20_000_000))
            .expect_err("stable refresh should not produce another D8 change");
        assert!(
            err.to_string().contains("run_to_event 等待超时"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn seg_change_events_flow_to_wave_without_touching_script_tracker() {
        let mut sim = Simulator::from_hex_path_with_options(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
            WaveCaptureOptions {
                json_path: Some(std::env::temp_dir().join("seg-change-test.json")),
                ..WaveCaptureOptions::default()
            },
        )
        .expect("load key_seg");

        sim.run_ms(150).expect("run key_seg to idle");
        let before_seg_change = sim
            .wave
            .event_records()
            .into_iter()
            .filter(|(track_id, _, _, _)| *track_id == EventTrack::SegChange.track_id())
            .count();
        let before_d8_change = sim
            .wave
            .event_records()
            .into_iter()
            .filter(|(track_id, _, _, _)| *track_id == EventTrack::SegDigit8.track_id())
            .count();
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegChange), 0);
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegDigit8), 0);

        sim.set_key("S4", true)
            .expect("press S4 to request a new D8 value");
        sim.run_ms(150)
            .expect("run until the display finishes changing");

        let after_events = sim.wave.event_records();
        let after_seg_change = after_events
            .iter()
            .filter(|(track_id, _, _, _)| *track_id == EventTrack::SegChange.track_id())
            .count();
        let after_d8_change = after_events
            .iter()
            .filter(|(track_id, _, _, _)| *track_id == EventTrack::SegDigit8.track_id())
            .count();
        assert!(after_seg_change > before_seg_change);
        assert!(after_d8_change > before_d8_change);
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegChange), 0);
        assert_eq!(sim.script_event_tracker.count(EventTrack::SegDigit8), 0);
    }

    #[test]
    fn key_seg_detects_s4_and_toggles_l1() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        sim.run_ms(150).expect("run key_seg to idle");
        assert_eq!(sim.display_text(), "       0");

        sim.set_key("S4", true).expect("press S4");
        sim.run_ms(150).expect("run with S4 pressed");
        assert_eq!(sim.display_text(), "       1");
        assert!(sim.led_on(1).expect("read L1"));
    }

    #[test]
    fn key_seg_clears_high_digits_after_release() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        sim.run_ms(220).expect("run key_seg to idle");
        sim.set_key("S12", true).expect("press S12");
        sim.run_ms(220).expect("run with S12 pressed");
        assert_eq!(sim.display_text(), "     256");

        sim.set_key("S12", false).expect("release S12");
        sim.run_ms(220).expect("run with S12 released");
        assert_eq!(sim.display_text(), "       0");
    }

    #[test]
    fn key_seg_btn_reads_independent_keys_without_matrix_scan() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg_btn/prj/Objects/key_seg_btn.hex"),
            false,
        )
        .expect("load key_seg_btn");

        sim.key_mode(KeyMode::Button);
        sim.run_ms(220).expect("run key_seg_btn to idle");
        assert_eq!(sim.display_text(), "       0");

        sim.set_key("S4", true).expect("press S4");
        sim.run_ms(220).expect("run with S4 pressed");
        assert_eq!(sim.display_text(), "       1");
        assert!(sim.led_on(1).expect("read L1"));

        sim.set_key("S4", false).expect("release S4");
        sim.set_key("S7", true).expect("press S7");
        sim.run_ms(220).expect("run with S7 pressed");
        assert_eq!(sim.display_text(), "       8");
        assert!(sim.led_on(4).expect("read L4"));
    }

    #[test]
    fn us_sample_tracks_distance_and_speed_setting() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("samples/us/prj/Objects/us.hex"), false)
                .expect("load us");

        sim.run_ms(220).expect("run us to idle");
        assert_eq!(sim.seg_pattern(1).expect("read L pattern"), 0x38);
        assert_eq!(
            sim.observe_display_number_in_range(2, 8, 30)
                .expect("read initial distance"),
            DisplayNumber::Integer(0)
        );

        sim.set_distance_cm(20.0);
        sim.run_ms(220).expect("run us with 20cm obstacle");
        let default_distance = sim
            .observe_display_number_in_range(4, 8, 30)
            .expect("read default distance");
        assert!(
            matches!(default_distance, DisplayNumber::Integer(value) if (18..=20).contains(&value))
        );

        sim.tap_key("S4", 80).expect("switch to speed page");
        sim.run_ms(220).expect("run us after switching menu");
        assert_eq!(sim.seg_pattern(1).expect("read P pattern"), 0x73);
        assert_eq!(
            sim.observe_display_number_in_range(6, 8, 30)
                .expect("read default speed"),
            DisplayNumber::Integer(340)
        );

        sim.tap_key("S9", 80).expect("increase speed");
        sim.run_ms(220).expect("run us after increasing speed");
        assert_eq!(
            sim.observe_display_number_in_range(6, 8, 30)
                .expect("read increased speed"),
            DisplayNumber::Integer(345)
        );

        sim.tap_key("S4", 80).expect("switch back to distance page");
        sim.run_ms(220)
            .expect("run us after returning to distance page");
        let adjusted_distance = sim
            .observe_display_number_in_range(4, 8, 30)
            .expect("read adjusted distance");
        assert!(
            matches!(adjusted_distance, DisplayNumber::Integer(value) if (19..=21).contains(&value))
        );
    }

    #[test]
    fn display_number_parses_integer_and_float_tokens() {
        assert_eq!(
            super::parse_display_number("25.937").expect("read float"),
            DisplayNumber::Float(25.937)
        );
        assert_eq!(
            super::parse_display_number("0007").expect("read leading zero int"),
            DisplayNumber::Integer(7)
        );
        assert!(super::parse_display_number("23-59-50").is_err());
    }

    #[test]
    fn display_number_range_uses_physical_digit_positions() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("samples/ds18b20/prj/Objects/ds18b20.hex"),
            false,
        )
        .expect("load ds18b20");

        sim.set_temperature_c(25.9375);
        sim.run_ms(1100).expect("run ds18b20 to stable display");

        assert_eq!(
            sim.display_number_in_range(1, 6)
                .expect("read temperature range"),
            DisplayNumber::Float(25.5)
        );
        assert_eq!(
            sim.display_number_in_range(8, 8).expect("read level digit"),
            DisplayNumber::Integer(0)
        );
    }

    #[test]
    fn na16_shows_boot_time_after_display_refresh() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("samples/na16/prj/Objects/na16.hex"), false)
                .expect("load na16");

        sim.run_ms(200).expect("run na16 to stable boot display");
        assert_eq!(sim.display_text(), "23-59-50");
    }

    #[test]
    fn na16_uart_float_param_round_trips_after_setting_h1() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("samples/na16/prj/Objects/na16.hex"), false)
                .expect("load na16");

        sim.set_rtc(23, 59, 50).expect("set rtc");
        sim.run_ms(320).expect("wait boot settle");

        sim.uart_write(b"(H1,1.8)").expect("write H1");
        let mut set_reply = String::new();
        for _ in 0..200 {
            sim.run_ms(1).expect("wait H1 apply tick");
            set_reply += &sim.uart_take_string().expect("take H1 reply tick");
            if set_reply.contains("OK") {
                break;
            }
        }
        assert!(set_reply.contains("OK"), "设置 H1 未返回 OK: {set_reply}");

        sim.uart_write(b"(H1,?)").expect("query H1");
        let mut query_reply = String::new();
        for _ in 0..200 {
            sim.run_ms(1).expect("wait H1 query tick");
            query_reply.push_str(&sim.uart_take_string().expect("take H1 query reply tick"));
            if query_reply.contains("(H1,1.8)") {
                break;
            }
        }
        assert!(
            query_reply.contains("(H1,1.8)"),
            "查询 H1 未返回期望值: {query_reply}"
        );
    }

    #[test]
    fn simulator_starts_with_relay_motor_and_buzzer_enabled() {
        let sim = Simulator::from_hex_path(
            &sample_path("samples/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        assert!(sim.relay_on(), "relay should be on at boot");
        assert!(sim.motor_on(), "motor should be on at boot");
        assert!(sim.buzzer_on(), "buzzer should be on at boot");
    }

    #[test]
    fn snapshot_matches_public_state_getters() {
        let mut sim = Simulator::nop(false);
        sim.set_key_id(KeyId::S4, true);
        sim.set_voltage_channel(VoltageChannel::Rd1, 1.25);
        sim.set_frequency_hz(1234.0);

        let snapshot = sim.snapshot();
        assert_eq!(snapshot.sim_time_ns, sim.sim_time_ns());
        assert_eq!(snapshot.display_text, sim.display_text());
        assert_eq!(snapshot.led_states[0], sim.led_on_id(LedId::L1));
        assert_eq!(snapshot.relay_on, sim.relay_on());
        assert_eq!(snapshot.motor_on, sim.motor_on());
        assert_eq!(snapshot.buzzer_on, sim.buzzer_on());
        assert!(snapshot.key_states[0]);
        assert!((snapshot.analog_rd1_v - 1.25).abs() < f32::EPSILON);
        assert_eq!(snapshot.ne555_frequency_hz, 1234.0);
    }
}
