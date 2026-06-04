use std::{cell::Cell, collections::VecDeque, fmt::Write as _, fs, path::Path};

use anyhow::{Context, Result, bail};
use i8051::{
    Cpu, CpuContext, CpuView, Flag, Interrupt as CpuInterrupt, MemoryMapper, PortMapper,
    ReadOnlyMemoryMapper, Register,
};
use tracing::{trace, warn};

pub(crate) const SYSTEM_HZ: u64 = 12_000_000;
pub(crate) const CPU_EXEC_HZ: u64 = 12_000_000;
pub(crate) const NS_PER_SECOND: u64 = 1_000_000_000;
pub(crate) const NS_PER_MILLISECOND: u64 = 1_000_000;
pub(crate) const NS_PER_MICROSECOND: u64 = 1_000;
const BOARD_POWER_ON_LATCHES: [u8; 4] = [0x00, 0x70, 0x00, 0x00];
const INTERRUPT_ENTRY_TICKS: u32 = 3;

use crate::{
    hex::load_ihex,
    ids::{KeyId, KeyMode, LedId, SignalId, VoltageChannel},
    jumper::{BoardJumpers, LineDrive, resolve_line},
    peripherals::{
        AnalogInputs, At24c02, Ds18b20, Ds1302, I2cBus, Key, Ne555, Outputs, Pcf8591,
        SegmentDecoder, UltrasonicDevice,
    },
    persistent_state::PersistentState,
    wave::{TRACK_EVENT_CPU, WaveCaptureOptions, WaveEventNote, WaveRecorder, WaveSnapshot},
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
    seg_decoder: SegmentDecoder,
    wave: WaveRecorder,
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
            InterruptSource::Serial => CpuInterrupt::Serial,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LedWatchStats {
    pub(crate) on_time_ns: u64,
    pub(crate) observed_time_ns: u64,
    pub(crate) changes: u64,
    pub(crate) rising_edges: u64,
}

impl LedWatchStats {
    pub(crate) fn change_frequency_hz(self) -> Result<f64> {
        if self.observed_time_ns == 0 {
            bail!("统计时长必须 > 0");
        }
        Ok(self.changes as f64 * NS_PER_SECOND as f64 / self.observed_time_ns as f64)
    }

    pub(crate) fn pwm_frequency_hz(self) -> Result<f64> {
        if self.observed_time_ns == 0 {
            bail!("统计时长必须 > 0");
        }
        Ok(self.rising_edges as f64 * NS_PER_SECOND as f64 / self.observed_time_ns as f64)
    }

    pub(crate) fn duty_percent(self) -> Result<f64> {
        if self.observed_time_ns == 0 {
            bail!("统计时长必须 > 0");
        }
        Ok(self.on_time_ns as f64 * 100.0 / self.observed_time_ns as f64)
    }
}

impl Simulator {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_hex_path(path: &Path, trace_cpu: bool) -> Result<Self> {
        Self::from_hex_path_with_options(path, trace_cpu, WaveCaptureOptions::default())
    }

    pub fn from_hex_path_with_options(
        path: &Path,
        trace_cpu: bool,
        wave_options: WaveCaptureOptions,
    ) -> Result<Self> {
        let hex = fs::read_to_string(path)
            .with_context(|| format!("读取 HEX 文件失败: {}", path.display()))?;
        let code = load_ihex(&hex)?;
        let wave_enabled = wave_options.enabled();
        let ctx = MachineContext::new_with_wave_enabled(code.clone(), wave_enabled);
        let mut sim = Self {
            cpu: Cpu::new(),
            ctx,
            code_image: code,
            trace_cpu,
            seg_decoder: SegmentDecoder::default(),
            wave: WaveRecorder::new(wave_options),
        };
        sim.ctx.ports.refresh_inputs(&sim.ctx.board);
        sim.capture_wave_snapshot();
        Ok(sim)
    }

    pub fn export_persistent_state(&self) -> String {
        self.ctx.board.persistent_state().encode()
    }

    pub fn load_persistent_state(&mut self, text: &str) -> Result<()> {
        let state = PersistentState::decode(text)?;
        self.ctx.board.load_persistent_state(&state);
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        let retained = self.ctx.board.retained_state();
        self.cpu = Cpu::new();
        self.ctx =
            MachineContext::new_with_wave_enabled(self.code_image.clone(), self.wave.enabled());
        self.ctx.board.load_retained_state(&retained);
        self.ctx.ports.refresh_inputs(&self.ctx.board);
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
        self.ctx.board.ds1302.set_hms(hour, minute, second)?;
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
        self.ctx.board.ne555.set_frequency_hz(value);
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

    pub fn uart_write(&mut self, bytes: &[u8]) {
        self.ctx.ports.uart1.feed_rx(bytes);
        self.capture_control_snapshot();
    }

    pub fn uart_take_string(&mut self) -> String {
        self.ctx.ports.uart1.take_tx_string()
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

    pub(crate) fn watch_led_stats(
        &mut self,
        led: LedId,
        duration_ms: u64,
    ) -> Result<LedWatchStats> {
        let start = self.ctx.board.sim_time_ns;
        let target = start.saturating_add(duration_ms.saturating_mul(NS_PER_MILLISECOND));
        let mut stats = LedWatchStats::default();

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
                stats.changes = stats.changes.saturating_add(1);
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
        let initial = self.display_text();
        if duration_ms == 0 {
            return Ok(initial);
        }
        let initial_digits = self.ctx.board.outputs.digits;

        let target = self
            .ctx
            .board
            .sim_time_ns
            .saturating_add(duration_ms.saturating_mul(NS_PER_MILLISECOND));
        while self.ctx.board.sim_time_ns < target {
            self.step_once()?;
            if self.ctx.board.outputs.digits != initial_digits {
                let current = self.display_text();
                bail!("display_text 在观察窗口内发生变化: 初始 `{initial}`, 后续 `{current}`");
            }
        }
        Ok(initial)
    }

    pub fn display_number(&self) -> Result<i64> {
        parse_display_number(&self.display_text())
    }

    pub fn observe_display_number(&mut self, duration_ms: u64) -> Result<i64> {
        let text = self.observe_display_text(duration_ms)?;
        parse_display_number(&text)
    }

    pub fn display_number_in_range(&self, start: usize, end: usize) -> Result<i64> {
        parse_display_number_in_range(&self.display_text(), start, end)
    }

    pub fn observe_display_number_in_range(
        &mut self,
        start: usize,
        end: usize,
        duration_ms: u64,
    ) -> Result<i64> {
        let text = self.observe_display_text(duration_ms)?;
        parse_display_number_in_range(&text, start, end)
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

    fn step_once(&mut self) -> Result<()> {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        if self.try_enter_pending_interrupt()? {
            return Ok(());
        }
        let ticks = self.current_instruction_ticks();
        if self.trace_cpu {
            let instruction = self.cpu.decode_pc(&self.ctx);
            trace!("{instruction:#}");
        }
        let _ = self.cpu.step(&mut self.ctx);
        self.tick_devices(ticks)
    }

    fn try_enter_pending_interrupt(&mut self) -> Result<bool> {
        let Some(pending) = self.pending_interrupt() else {
            return Ok(false);
        };
        if !self.cpu.interrupt(pending.cpu_interrupt()) {
            return Ok(false);
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
        let tcon = self.cpu.sfr(SFR_TCON, &self.ctx);
        let scon = self.cpu.sfr(SFR_SCON, &self.ctx);
        let p3 = self.cpu.sfr(SFR_P3, &self.ctx);

        for high_priority in [true, false] {
            for candidate in [
                PendingInterrupt {
                    source: InterruptSource::External0,
                    tcon_clear_mask: 0,
                },
                PendingInterrupt {
                    source: InterruptSource::Timer0,
                    tcon_clear_mask: TCON_TF0,
                },
                PendingInterrupt {
                    source: InterruptSource::External1,
                    tcon_clear_mask: 0,
                },
                PendingInterrupt {
                    source: InterruptSource::Timer1,
                    tcon_clear_mask: TCON_TF1,
                },
                PendingInterrupt {
                    source: InterruptSource::Serial,
                    tcon_clear_mask: 0,
                },
            ] {
                let (enable_mask, pending, priority_high) = match candidate.source {
                    InterruptSource::External0 => (IE_EX0, p3 & P3_INT0 == 0, ip & IE_EX0 != 0),
                    InterruptSource::Timer0 => (IE_ET0, tcon & TCON_TF0 != 0, ip & IE_ET0 != 0),
                    InterruptSource::External1 => (IE_EX1, p3 & P3_INT1 == 0, ip & IE_EX1 != 0),
                    InterruptSource::Timer1 => (IE_ET1, tcon & TCON_TF1 != 0, ip & IE_ET1 != 0),
                    InterruptSource::Serial => {
                        (IE_ES, scon & (SCON_RI | SCON_TI) != 0, ip & IE_ES != 0)
                    }
                };
                if ie & enable_mask == 0 || !pending || priority_high != high_priority {
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
        let t0_falling_edges = self
            .ctx
            .board
            .t0_falling_edges(&self.ctx.ports.port_latch, start_time_ns);
        self.ctx
            .ports
            .tick_timers01_t2(&self.ctx.board, system_cycles, t0_falling_edges)?;
        self.ctx
            .ports
            .tick_ultrasonic(&mut self.ctx.board, elapsed_ns);
        self.ctx.ports.tick_pca(system_cycles)?;
        self.ctx.ports.uart1.tick_ns(start_time_ns, elapsed_ns);
        let responses = self.ctx.ports.uart2.tick_ns(start_time_ns, elapsed_ns);
        for response in responses {
            self.ctx.board.ultrasonic.push_response(response);
        }
        if let Some(response) = self.ctx.board.ultrasonic.pop_response() {
            self.ctx.ports.uart2.feed_rx(&[response]);
        }
        let board_latches = self.ctx.effective_board_latches();
        let board_latch_versions = self.ctx.effective_board_latch_versions();
        self.ctx
            .board
            .tick_protocols(&self.ctx.ports, &board_latches, &board_latch_versions);
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        self.drain_wave_events();
        Ok(())
    }

    fn current_instruction_ticks(&self) -> u32 {
        let op = self
            .ctx
            .code
            .code
            .get(usize::from(self.cpu.pc))
            .copied()
            .unwrap_or(0);
        approximate_instruction_ticks(op)
    }

    fn capture_control_snapshot(&mut self) {
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        self.capture_wave_snapshot();
        self.drain_wave_events();
    }

    fn capture_wave_snapshot(&mut self) {
        if !self.wave.enabled() {
            return;
        }

        let effective_board_latches = self.ctx.effective_board_latches();
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

        let mut seg_chars = [' '; 8];
        let mut seg_text = String::with_capacity(8);
        let mut seg_raw = [0_u8; 8];
        for (index, digit) in self.ctx.board.outputs.digits.iter().copied().enumerate() {
            let ch = self.seg_decoder.decode_char(digit);
            seg_chars[index] = ch;
            seg_text.push(ch);
            seg_raw[index] = digit.segments;
        }

        let key_order = [
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
        let mut key_states = [false; 16];
        for (index, key) in key_order.into_iter().enumerate() {
            key_states[index] = self.ctx.board.keys.pressed(key);
        }

        let snapshot = WaveSnapshot {
            time_ns: self.ctx.board.sim_time_ns,
            port_latch: self.ctx.ports.port_latch,
            port_input: self.ctx.ports.port_input,
            board_latches_effective: effective_board_latches,
            board_latches_port: self.ctx.ports.board_latches,
            board_latches_xdata: self.ctx.xdata.board_latches,
            signal_sig_out: self.ctx.board.frequency_level(),
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
            uart2_tx_high: self.ctx.ports.uart2.tx_line_high(),
            uart2_rx_high: self.ctx.ports.uart2.rx_line_high(),
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
            adc_channel: self.ctx.board.pcf8591.selected_channel(),
            adc_channel_voltage_v: self
                .ctx
                .board
                .analog
                .channel_voltage(self.ctx.board.pcf8591.selected_channel()),
            dac_code: self.ctx.board.pcf8591.dac_value(),
            dac_voltage_v: self.ctx.board.pcf8591.dac_voltage_v(),
            ne555_level: self.ctx.board.frequency_level(),
            ne555_frequency_hz: self.ctx.board.ne555.frequency_hz(),
        };
        self.wave.observe_snapshot(&snapshot);
    }

    fn drain_wave_events(&mut self) {
        if !self.wave.enabled() {
            return;
        }
        for note in self.ctx.board.ds18b20.take_wave_events() {
            self.wave.record_event_note(note);
        }
        for note in self.ctx.board.pcf8591.take_wave_events() {
            self.wave.record_event_note(note);
        }
        for note in self.ctx.board.ds1302.take_wave_events() {
            self.wave.record_event_note(note);
        }
        for note in self.ctx.ports.uart1.take_wave_events() {
            self.wave.record_event_note(note);
        }
        for note in self.ctx.ports.uart2.take_wave_events() {
            self.wave.record_event_note(note);
        }
    }

    fn note_interrupt_event(&mut self, pending: PendingInterrupt) {
        if !self.wave.enabled() {
            return;
        }
        let label = match pending.source {
            InterruptSource::External0 => "INT0 enter",
            InterruptSource::Timer0 => "T0 enter",
            InterruptSource::External1 => "INT1 enter",
            InterruptSource::Timer1 => "T1 enter",
            InterruptSource::Serial => "UART enter",
        };
        let note = WaveEventNote::with_detail(
            self.ctx.board.sim_time_ns,
            TRACK_EVENT_CPU,
            label,
            format!("pc=0x{:04X}", self.cpu.pc_ext(&self.ctx)),
        );
        self.wave.record_event_note(note);
    }
}

fn approximate_instruction_ticks(op: u8) -> u32 {
    match op {
        0x00 => 1,
        0x01 | 0x11 | 0x21 | 0x31 | 0x41 | 0x51 | 0x61 | 0x71 | 0x81 | 0x91 | 0xA1 | 0xB1
        | 0xC1 | 0xD1 | 0xE1 | 0xF1 => 2,
        0x02 | 0x12 | 0x22 | 0x32 => 2,
        0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 | 0x80 => 2,
        0x76 | 0x77 | 0x86 | 0x87 | 0x88..=0x8F | 0x90 | 0xA6..=0xAF => 2,
        0x05 | 0x15 | 0x42 | 0x45 | 0x52 | 0x55 | 0x62 | 0x65 | 0xA2 | 0xA3 | 0xB2 | 0xC2
        | 0xD2 | 0xE5 | 0xF5 => 1,
        0x43 | 0x53 | 0x63 | 0x75 | 0x85 | 0x92 | 0xB4..=0xBF => 2,
        0xC0 | 0xD0 | 0xD5 | 0xD8..=0xDF => 2,
        0x73 | 0x83 | 0x93 => 2,
        0xA4 | 0x84 => 4,
        0xE0 | 0xE2 | 0xE3 | 0xF0 | 0xF2 | 0xF3 => 2,
        _ => 1,
    }
}

fn uart_frame_ns(baud_rate: u32) -> u64 {
    ((NS_PER_SECOND as f64 * 10.0) / f64::from(baud_rate))
        .round()
        .clamp(1.0, u64::MAX as f64) as u64
}

struct MachineContext {
    ports: MachinePorts,
    xdata: BoardXdata,
    code: CodeMemory,
    board: BoardModel,
}

impl MachineContext {
    #[cfg_attr(not(test), allow(dead_code))]
    fn new(code: Vec<u8>) -> Self {
        Self::new_with_wave_enabled(code, true)
    }

    fn new_with_wave_enabled(code: Vec<u8>, wave_enabled: bool) -> Self {
        let mut board = BoardModel::new_with_wave_enabled(wave_enabled);
        board
            .outputs
            .sample_from_latches(&BOARD_POWER_ON_LATCHES, &[0; 4]);
        Self {
            ports: MachinePorts::new_with_wave_enabled(wave_enabled),
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
                uart_frame_ns(9_600),
                wave_enabled,
            ),
            uart2: Uart::new(
                UART2_SFR_S2CON,
                UART2_SFR_S2BUF,
                uart_frame_ns(9_600),
                wave_enabled,
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

    fn refresh_inputs(&mut self, board: &BoardModel) {
        for index in 0..self.port_input.len() {
            self.port_input[index] =
                board.read_port(index, self.port_latch[index], &self.port_latch);
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
        t0_falling_edges: u32,
    ) -> Result<()> {
        let p3 = self.sample_port_p3(board);
        let auxr = self.generic_get(SFR_AUXR);
        self.timers
            .tick_timers01_t2(p3, auxr, ticks, t0_falling_edges, &mut self.generic)
    }

    fn tick_pca(&mut self, ticks: u32) -> Result<()> {
        self.timers.tick_pca(ticks, &mut self.generic)
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

#[derive(Debug)]
struct UartFrame {
    byte: u8,
    bit_index: u8,
    bit_remaining_ns: u64,
}

impl UartFrame {
    fn new(byte: u8, bit_ns: u64) -> Self {
        Self {
            byte,
            bit_index: 0,
            bit_remaining_ns: bit_ns,
        }
    }

    fn current_level(&self) -> bool {
        match self.bit_index {
            0 => false,
            1..=8 => self.byte & (1 << (self.bit_index - 1)) != 0,
            9 => true,
            _ => true,
        }
    }

    fn advance(&mut self, elapsed_ns: u64, bit_ns: u64) -> bool {
        let mut remaining = elapsed_ns;
        while remaining >= self.bit_remaining_ns {
            remaining -= self.bit_remaining_ns;
            self.bit_index = self.bit_index.saturating_add(1);
            if self.bit_index >= 10 {
                return true;
            }
            self.bit_remaining_ns = bit_ns;
        }
        self.bit_remaining_ns -= remaining;
        false
    }
}

#[derive(Debug)]
struct Uart {
    scon_addr: u8,
    sbuf_addr: u8,
    control: u8,
    rx_sbuf: u8,
    tx_queue: VecDeque<u8>,
    tx_pending: VecDeque<u8>,
    rx_queue: VecDeque<u8>,
    tx_frame: Option<UartFrame>,
    rx_frame: Option<UartFrame>,
    bit_ns: u64,
    tx_line_high: bool,
    rx_line_high: bool,
    wave_events: Option<Vec<WaveEventNote>>,
}

impl Uart {
    fn new(scon_addr: u8, sbuf_addr: u8, frame_ns: u64, wave_enabled: bool) -> Self {
        Self {
            scon_addr,
            sbuf_addr,
            control: 0,
            rx_sbuf: 0,
            tx_queue: VecDeque::new(),
            tx_pending: VecDeque::new(),
            rx_queue: VecDeque::new(),
            tx_frame: None,
            rx_frame: None,
            bit_ns: (frame_ns / 10).max(1),
            tx_line_high: true,
            rx_line_high: true,
            wave_events: wave_enabled.then(Vec::new),
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
            self.control = value;
        } else if addr == self.sbuf_addr {
            self.tx_pending.push_back(value);
        }
    }

    fn tick_ns(&mut self, start_time_ns: u64, elapsed_ns: u64) -> Vec<u8> {
        let mut sent = Vec::new();

        if self.tx_frame.is_none()
            && let Some(byte) = self.tx_pending.pop_front()
        {
            self.tx_frame = Some(UartFrame::new(byte, self.bit_ns));
            self.tx_line_high = false;
            let track_id = if self.scon_addr == UART2_SFR_S2CON {
                crate::wave::TRACK_EVENT_UART2
            } else {
                crate::wave::TRACK_EVENT_UART1
            };
            self.push_wave_event(|| {
                WaveEventNote::new(start_time_ns, track_id, format!("TX 0x{byte:02X}"))
            });
        }

        if let Some(frame) = self.tx_frame.as_mut() {
            if frame.advance(elapsed_ns, self.bit_ns) {
                let byte = frame.byte;
                self.tx_frame = None;
                self.tx_line_high = true;
                self.control |= if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_TI
                } else {
                    SCON_TI
                };
                self.tx_queue.push_back(byte);
                sent.push(byte);
            } else {
                self.tx_line_high = frame.current_level();
            }
        }

        if self.rx_frame.is_none()
            && let Some(byte) = self.rx_queue.pop_front()
        {
            self.rx_frame = Some(UartFrame::new(byte, self.bit_ns));
            self.rx_line_high = false;
            let track_id = if self.scon_addr == UART2_SFR_S2CON {
                crate::wave::TRACK_EVENT_UART2
            } else {
                crate::wave::TRACK_EVENT_UART1
            };
            self.push_wave_event(|| {
                WaveEventNote::new(start_time_ns, track_id, format!("RX 0x{byte:02X}"))
            });
        }

        if let Some(frame) = self.rx_frame.as_mut() {
            if frame.advance(elapsed_ns, self.bit_ns) {
                let byte = frame.byte;
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
                    self.rx_sbuf = byte;
                    self.control |= ri_flag;
                }
            } else {
                self.rx_line_high = frame.current_level();
            }
        }

        sent
    }

    fn feed_rx(&mut self, bytes: &[u8]) {
        self.rx_queue.extend(bytes.iter().copied());
    }

    fn take_wave_events(&mut self) -> Vec<WaveEventNote> {
        match self.wave_events.as_mut() {
            Some(events) => std::mem::take(events),
            None => Vec::new(),
        }
    }

    fn tx_line_high(&self) -> bool {
        self.tx_line_high
    }

    fn rx_line_high(&self) -> bool {
        self.rx_line_high
    }

    fn take_tx_string(&mut self) -> String {
        let bytes = self.tx_queue.drain(..).collect::<Vec<_>>();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn tx_text(&self) -> String {
        String::from_utf8_lossy(self.tx_queue.as_slices().0).into_owned()
    }

    fn push_wave_event<F>(&mut self, build: F)
    where
        F: FnOnce() -> WaveEventNote,
    {
        if let Some(events) = self.wave_events.as_mut() {
            events.push(build());
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
    p34_conflict_active: Cell<bool>,
}

impl BoardModel {
    #[cfg_attr(not(test), allow(dead_code))]
    fn new() -> Self {
        Self::new_with_wave_enabled(true)
    }

    fn new_with_wave_enabled(wave_enabled: bool) -> Self {
        Self {
            cpu_cycles: 0,
            sim_time_ns: 0,
            sim_time_ns_remainder: 0,
            system_cycle_remainder: 0,
            outputs: Outputs::default(),
            ds18b20: Ds18b20::new(wave_enabled),
            ds1302: Ds1302::new(wave_enabled),
            i2c: I2cBus,
            pcf8591: Pcf8591::new(wave_enabled),
            at24c02: At24c02::default(),
            ne555: Ne555::default(),
            ultrasonic: UltrasonicDevice::default(),
            keys: Key::default(),
            key_mode: KeyMode::default(),
            analog: AnalogInputs::default(),
            jumpers: BoardJumpers::default(),
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
        self.ds18b20.temperature_c = state.ds18b20_temperature_c;
        self.ds18b20
            .set_parasite_power(state.ds18b20_parasite_power);
        self.ultrasonic.distance_cm = state.ultrasonic_distance_cm.max(0.0);
        self.ne555.set_frequency_hz(state.ne555_frequency_hz);
    }

    fn advance_cycles(&mut self, cycles: u64) -> (u64, u32) {
        self.cpu_cycles = self.cpu_cycles.saturating_add(cycles);
        let total_ns = u128::from(self.sim_time_ns_remainder)
            .saturating_add(u128::from(cycles).saturating_mul(u128::from(NS_PER_SECOND)));
        let elapsed_ns = (total_ns / u128::from(CPU_EXEC_HZ)).min(u128::from(u64::MAX)) as u64;
        self.sim_time_ns_remainder = (total_ns % u128::from(CPU_EXEC_HZ)) as u64;
        self.sim_time_ns = self.sim_time_ns.saturating_add(elapsed_ns);
        self.ds1302.tick_ns(elapsed_ns);
        let total_system_cycles = u128::from(self.system_cycle_remainder)
            .saturating_add(u128::from(cycles).saturating_mul(u128::from(SYSTEM_HZ)));
        let system_cycles =
            (total_system_cycles / u128::from(CPU_EXEC_HZ)).min(u128::from(u32::MAX)) as u32;
        self.system_cycle_remainder = (total_system_cycles % u128::from(CPU_EXEC_HZ)) as u64;
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
            .sample_from_latches(board_latches, board_latch_versions);
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

    fn read_port(&self, index: usize, latch: u8, all_latches: &[u8; 6]) -> u8 {
        let mut value = latch;
        match index {
            1 => {
                value = apply_open_drain_bit(value, 4, self.ds18b20.drive_low);
                value = apply_push_pull_bit(value, 0, true);
                value = apply_push_pull_bit(value, 1, self.ultrasonic.rx_level());
            }
            2 => {
                value = self.apply_i2c_lines(value);
                value = apply_push_pull_bit(value, 3, self.ds1302.io_level);
                value = apply_push_pull_bit(value, 4, self.read_hall_level());
            }
            3 => {
                value = set_bit_level(value, 4, self.read_p34_level(latch, all_latches));
                let rows = [(0_u8, 0_u8), (1, 1), (2, 2), (3, 3)];
                for (bit, row) in rows {
                    let low = match self.key_mode {
                        KeyMode::Keyboard => self.keys.row_low(row, all_latches),
                        KeyMode::Button => self.keys.button_row_low(row),
                    };
                    value = set_bit_level(value, bit, !low);
                }
                value = set_bit_level(value, 5, true);
            }
            4 if self.key_mode == KeyMode::Keyboard => {
                value = apply_open_drain_bit(value, 2, self.keys.col_low(1, all_latches));
                value = apply_open_drain_bit(value, 4, self.keys.col_low(0, all_latches));
            }
            4 => {}
            _ => {}
        }
        value
    }

    fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.keys.set_key(name, pressed)
    }

    fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.analog.set_voltage(name, value)
    }

    fn jumper_on(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.jumpers.install(left, right)
    }

    fn jumper_off(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.jumpers.remove(left, right)
    }

    fn jumper_installed(&self, left: SignalId, right: SignalId) -> bool {
        self.jumpers.is_installed(left, right)
    }

    fn frequency_level(&self) -> bool {
        self.ne555.level(self.sim_time_ns)
    }

    fn t0_falling_edges(&self, all_latches: &[u8; 6], start_time_ns: u64) -> u32 {
        let end_time_ns = self.sim_time_ns;
        if end_time_ns <= start_time_ns {
            return 0;
        }
        if !self.jumper_installed(SignalId::NetSig, SignalId::SigOut) {
            return 0;
        }
        if all_latches[3] & (1 << 4) == 0 {
            return 0;
        }
        if self.key_mode == KeyMode::Keyboard && self.keys.col_low(3, all_latches) {
            return 0;
        }

        self.ne555.falling_edges_between(start_time_ns, end_time_ns)
    }

    fn read_hall_level(&self) -> bool {
        true
    }

    fn read_p34_level(&self, latch: u8, all_latches: &[u8; 6]) -> bool {
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
                if self.key_mode == KeyMode::Keyboard && self.keys.col_low(3, all_latches) {
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

fn slice_text_range(text: &str, start: usize, end: usize) -> Result<String> {
    if start == 0 || end == 0 {
        bail!("字符串切片范围必须从 1 开始: start={start}, end={end}");
    }
    if start > end {
        bail!("字符串切片范围必须满足 start <= end: start={start}, end={end}");
    }
    let chars = text.chars().collect::<Vec<_>>();
    if end > chars.len() {
        bail!(
            "字符串切片范围越界: 文本长度为 {}, 请求范围 {}..={}",
            chars.len(),
            start,
            end
        );
    }
    Ok(chars[start - 1..end].iter().collect::<String>())
}

fn parse_display_number(text: &str) -> Result<i64> {
    parse_display_integer_slice(text)
}

fn parse_display_number_in_range(text: &str, start: usize, end: usize) -> Result<i64> {
    parse_display_integer_slice(&slice_text_range(text, start, end)?)
}

fn parse_display_integer_slice(text: &str) -> Result<i64> {
    let value = extract_unique_numeric_token(text, false)?;
    value
        .parse::<i64>()
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

    use crate::ids::{KeyId, KeyMode, LedId, SignalId};

    use super::Simulator;

    fn sample_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
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
            &sample_path("sample/led_flicker/prj/Objects/led_flicker.hex"),
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
    }

    #[test]
    fn led_pwm_reports_expected_frequency_and_duty() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("sample/led_pwm/prj/Objects/led_pwm.hex"),
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
    fn net_sig_requires_explicit_jumper_to_reach_p34() {
        let mut board = super::BoardModel::default();
        let latches = default_port_latches();
        board.ne555.set_frequency_hz(2_200.0);

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
    fn p34_conflict_prefers_low_when_key_column_and_ne555_disagree() {
        let mut board = super::BoardModel::default();
        let mut latches = default_port_latches();
        board.ne555.set_frequency_hz(2_200.0);
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

        let mut sim = super::Simulator {
            cpu: Cpu::new(),
            ctx: super::MachineContext::new(code.clone()),
            code_image: code,
            trace_cpu: false,
            seg_decoder: super::SegmentDecoder::default(),
            wave: crate::wave::WaveRecorder::new(crate::wave::WaveCaptureOptions::default()),
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
        assert_eq!(super::approximate_instruction_ticks(0x30), 2);
        assert_eq!(super::approximate_instruction_ticks(0x53), 2);
        assert_eq!(super::approximate_instruction_ticks(0x88), 2);
        assert_eq!(super::approximate_instruction_ticks(0xA8), 2);
        assert_eq!(super::approximate_instruction_ticks(0x90), 2);
    }

    #[test]
    fn wave_disabled_does_not_buffer_uart_events() {
        let mut uart = super::Uart::new(
            super::UART1_SFR_SCON,
            super::UART1_SFR_SBUF,
            super::uart_frame_ns(9_600),
            false,
        );

        uart.write(super::UART1_SFR_SBUF, 0x55);
        let _ = uart.tick_ns(0, 1);

        assert!(uart.take_wave_events().is_empty());
    }

    #[test]
    fn key_seg_detects_s4_and_toggles_l1() {
        let mut sim = Simulator::from_hex_path(
            &sample_path("sample/key_seg/prj/Objects/key_seg.hex"),
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
            &sample_path("sample/key_seg/prj/Objects/key_seg.hex"),
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
            &sample_path("sample/key_seg_btn/prj/Objects/key_seg_btn.hex"),
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
        let mut sim = Simulator::from_hex_path(&sample_path("sample/us/prj/Objects/us.hex"), false)
            .expect("load us");

        sim.run_ms(220).expect("run us to idle");
        assert_eq!(sim.seg_pattern(1).expect("read L pattern"), 0x38);
        assert_eq!(
            sim.observe_display_number_in_range(2, 8, 30)
                .expect("read initial distance"),
            0
        );

        sim.set_distance_cm(20.0);
        sim.run_ms(220).expect("run us with 20cm obstacle");
        let default_distance = sim
            .observe_display_number_in_range(4, 8, 30)
            .expect("read default distance");
        assert!((18..=20).contains(&default_distance));

        sim.tap_key("S4", 80).expect("switch to speed page");
        sim.run_ms(220).expect("run us after switching menu");
        assert_eq!(sim.seg_pattern(1).expect("read P pattern"), 0x73);
        assert_eq!(
            sim.observe_display_number_in_range(6, 8, 30)
                .expect("read default speed"),
            340
        );

        sim.tap_key("S9", 80).expect("increase speed");
        sim.run_ms(220).expect("run us after increasing speed");
        assert_eq!(
            sim.observe_display_number_in_range(6, 8, 30)
                .expect("read increased speed"),
            345
        );

        sim.tap_key("S4", 80).expect("switch back to distance page");
        sim.run_ms(220)
            .expect("run us after returning to distance page");
        let adjusted_distance = sim
            .observe_display_number_in_range(4, 8, 30)
            .expect("read adjusted distance");
        assert!((19..=21).contains(&adjusted_distance));
    }

    #[test]
    fn display_number_range_extracts_requested_digits() {
        assert_eq!(
            super::parse_display_number_in_range("23-59-50", 1, 2).expect("read hour"),
            23
        );
        assert_eq!(
            super::parse_display_number_in_range("23-59-50", 4, 5).expect("read minute"),
            59
        );
        assert_eq!(
            super::parse_display_number_in_range("0007", 1, 4).expect("read leading zero int"),
            7
        );
        assert!(super::parse_display_number("23-59-50").is_err());
    }

    #[test]
    fn slice_text_range_uses_display_style_positions() {
        assert_eq!(
            super::slice_text_range("23-59-50", 4, 5).expect("slice text"),
            "59"
        );
    }

    #[test]
    fn na16_shows_boot_time_after_display_refresh() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("sample/na16/prj/Objects/na16.hex"), false)
                .expect("load na16");

        sim.run_ms(200).expect("run na16 to stable boot display");
        assert_eq!(sim.display_text(), "23-59-50");
    }

    #[test]
    fn simulator_starts_with_relay_motor_and_buzzer_enabled() {
        let sim = Simulator::from_hex_path(
            &sample_path("sample/key_seg/prj/Objects/key_seg.hex"),
            false,
        )
        .expect("load key_seg");

        assert!(sim.relay_on(), "relay should be on at boot");
        assert!(sim.motor_on(), "motor should be on at boot");
        assert!(sim.buzzer_on(), "buzzer should be on at boot");
    }
}
