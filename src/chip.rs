use std::{cell::Cell, collections::VecDeque, fmt::Write as _, fs, path::Path};

use anyhow::{Context, Result, bail};
use i8051::{Cpu, CpuContext, CpuView, MemoryMapper, PortMapper, ReadOnlyMemoryMapper, Register};
use tracing::{trace, warn};

pub(crate) const CPU_TICKS_PER_US: u64 = 35;
pub(crate) const TICKS_PER_SECOND: u64 = 1_000_000 * CPU_TICKS_PER_US;
const BOARD_POWER_ON_LATCHES: [u8; 4] = [0x00, 0x70, 0x00, 0x00];

use crate::{
    hex::load_ihex,
    ids::{KeyId, KeyMode, LedId, SignalId, VoltageChannel},
    jumper::{BoardJumpers, LineDrive, resolve_line},
    peripherals::{
        AnalogInputs, At24c02, Ds18b20, Ds1302, I2cBus, Key, Ne555, Outputs, Pcf8591,
        SegmentDecoder, UltrasonicDevice,
    },
};

mod registers;
mod timers;

use registers::*;
use timers::TimerBlock;

pub struct Simulator {
    cpu: Cpu,
    ctx: MachineContext,
    trace_cpu: bool,
    seg_decoder: SegmentDecoder,
}

impl Simulator {
    pub fn from_hex_path(path: &Path, trace_cpu: bool) -> Result<Self> {
        let hex = fs::read_to_string(path)
            .with_context(|| format!("读取 HEX 文件失败: {}", path.display()))?;
        let code = load_ihex(&hex)?;
        Ok(Self {
            cpu: Cpu::new(),
            ctx: MachineContext::new(code),
            trace_cpu,
            seg_decoder: SegmentDecoder::default(),
        })
    }

    pub fn run_ms(&mut self, ms: u64) -> Result<()> {
        self.run_us(ms.saturating_mul(1_000))
    }

    pub fn run_us(&mut self, us: u64) -> Result<()> {
        let target = self
            .ctx
            .board
            .ticks
            .saturating_add(us.saturating_mul(CPU_TICKS_PER_US));
        while self.ctx.board.ticks < target {
            self.step_once()?;
        }
        Ok(())
    }

    pub fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.ctx.board.set_key(name, pressed)
    }

    pub fn set_key_id(&mut self, key: KeyId, pressed: bool) {
        self.ctx.board.keys.set_key_id(key, pressed);
    }

    pub fn key_mode(&mut self, mode: KeyMode) {
        self.ctx.board.key_mode = mode;
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
        self.ctx.board.ds1302.set_hms(hour, minute, second)
    }

    pub fn set_temperature_c(&mut self, value: f32) {
        self.ctx.board.ds18b20.temperature_c = value;
    }

    pub fn set_distance_cm(&mut self, value: f32) {
        self.ctx.board.ultrasonic.distance_cm = value.max(0.0);
    }

    pub fn set_frequency_hz(&mut self, value: f32) {
        self.ctx.board.ne555.set_frequency_hz(value);
    }

    pub fn jumper_on(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.ctx.board.jumper_on(left, right)
    }

    pub fn jumper_on_named(&mut self, left: &str, right: &str) -> Result<()> {
        self.jumper_on(SignalId::parse(left)?, SignalId::parse(right)?)
    }

    pub fn jumper_off(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        self.ctx.board.jumper_off(left, right)
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
        self.ctx.board.set_voltage(name, value)
    }

    pub fn set_voltage_channel(&mut self, channel: VoltageChannel, value: f32) {
        self.ctx.board.analog.set_voltage_channel(channel, value);
    }

    pub fn uart_write(&mut self, bytes: &[u8]) {
        self.ctx.ports.uart1.feed_rx(bytes);
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

    pub fn watch_led_changes(&mut self, led: LedId, duration_ms: u64) -> Result<u64> {
        let target = self.ctx.board.ticks.saturating_add(
            duration_ms
                .saturating_mul(1_000)
                .saturating_mul(CPU_TICKS_PER_US),
        );
        let mut previous = self.led_on_id(led);
        let mut changes = 0_u64;

        while self.ctx.board.ticks < target {
            self.step_once()?;
            let current = self.led_on_id(led);
            if current != previous {
                changes += 1;
                previous = current;
            }
        }

        Ok(changes)
    }

    pub fn watch_led_changes_named(&mut self, name: &str, duration_ms: u64) -> Result<u64> {
        self.watch_led_changes(LedId::parse(name)?, duration_ms)
    }

    pub fn watch_led_frequency_hz(&mut self, led: LedId, duration_ms: u64) -> Result<f64> {
        if duration_ms == 0 {
            bail!("统计时长必须 > 0");
        }
        let changes = self.watch_led_changes(led, duration_ms)?;
        Ok(changes as f64 * 1_000.0 / duration_ms as f64)
    }

    pub fn watch_led_frequency_hz_named(&mut self, name: &str, duration_ms: u64) -> Result<f64> {
        self.watch_led_frequency_hz(LedId::parse(name)?, duration_ms)
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

        let target = self.ctx.board.ticks.saturating_add(
            duration_ms
                .saturating_mul(1_000)
                .saturating_mul(CPU_TICKS_PER_US),
        );
        while self.ctx.board.ticks < target {
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
        let _ = writeln!(out, "ticks: {}", self.ctx.board.ticks);
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
        let ticks = self.current_instruction_ticks();
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        if self.trace_cpu {
            let instruction = self.cpu.decode_pc(&self.ctx);
            trace!("{instruction:#}");
        }
        let _ = self.cpu.step(&mut self.ctx);
        self.tick_devices(ticks)
    }

    fn tick_devices(&mut self, ticks: u32) -> Result<()> {
        if ticks == 0 {
            return Ok(());
        }

        let start_ticks = self.ctx.board.ticks;
        self.ctx.board.advance_ticks(u64::from(ticks));
        let t0_falling_edges = self
            .ctx
            .board
            .t0_falling_edges(&self.ctx.ports.port_latch, start_ticks);
        self.ctx
            .ports
            .tick_timers01_t2(&self.ctx.board, ticks, t0_falling_edges)?;
        self.ctx.ports.tick_ultrasonic(&mut self.ctx.board, ticks);
        self.ctx.ports.tick_pca(ticks)?;
        self.ctx.ports.uart1.tick_ticks(ticks);
        let responses = self.ctx.ports.uart2.tick_ticks(ticks);
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
}

fn approximate_instruction_ticks(op: u8) -> u32 {
    match op {
        0x00 => 1,
        0x02 | 0x12 | 0x22 | 0x32 => 2,
        0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 | 0x80 => 2,
        0x73 | 0x83 | 0x93 => 2,
        0xA4 | 0x84 => 4,
        0xE0 | 0xE2 | 0xE3 | 0xF0 | 0xF2 | 0xF3 => 2,
        _ => 1,
    }
}

fn uart_frame_ticks(baud_rate: u32) -> u32 {
    ((TICKS_PER_SECOND as f64 * 10.0) / f64::from(baud_rate))
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

struct MachineContext {
    ports: MachinePorts,
    xdata: BoardXdata,
    code: CodeMemory,
    board: BoardModel,
}

impl MachineContext {
    fn new(code: Vec<u8>) -> Self {
        let mut board = BoardModel::default();
        board
            .outputs
            .sample_from_latches(&BOARD_POWER_ON_LATCHES, &[0; 4]);
        Self {
            ports: MachinePorts::new(),
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
    fn new() -> Self {
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
            uart1: Uart::new(UART1_SFR_SCON, UART1_SFR_SBUF, uart_frame_ticks(9_600)),
            uart2: Uart::new(UART2_SFR_S2CON, UART2_SFR_S2BUF, uart_frame_ticks(9_600)),
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

    fn sample_port_p3(&self, board: &BoardModel) -> u8 {
        board.read_port(3, self.port_latch[3], &self.port_latch)
    }

    fn refresh_inputs(&mut self, board: &BoardModel) {
        for index in 0..self.port_input.len() {
            self.port_input[index] =
                board.read_port(index, self.port_latch[index], &self.port_latch);
        }
    }

    fn tick_ultrasonic(&mut self, board: &mut BoardModel, ticks: u32) {
        let tx_high = self.port_latch[1] & (1 << 0) != 0;
        board.ultrasonic.sample_trigger(tx_high);
        board.ultrasonic.tick_ticks(ticks);
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
                if addr == SFR_P0 {
                    self.strobe_board_latch(self.port_latch[2], byte);
                } else if addr == SFR_P2 {
                    self.strobe_board_latch(byte, self.port_latch[0]);
                }
            }
            _ => self.generic_set(addr, byte),
        }
    }
}

#[derive(Debug)]
struct Uart {
    scon_addr: u8,
    sbuf_addr: u8,
    control: u8,
    rx_sbuf: u8,
    tx_queue: VecDeque<u8>,
    rx_queue: VecDeque<u8>,
    tx_countdown: Option<(u32, u8)>,
    rx_countdown: Option<(u32, u8)>,
    frame_ticks: u32,
}

impl Uart {
    fn new(scon_addr: u8, sbuf_addr: u8, frame_ticks: u32) -> Self {
        Self {
            scon_addr,
            sbuf_addr,
            control: 0,
            rx_sbuf: 0,
            tx_queue: VecDeque::new(),
            rx_queue: VecDeque::new(),
            tx_countdown: None,
            rx_countdown: None,
            frame_ticks,
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
            self.tx_countdown = Some((self.frame_ticks, value));
        }
    }

    fn tick_ticks(&mut self, elapsed_ticks: u32) -> Vec<u8> {
        let mut sent = Vec::new();

        if let Some((remaining, byte)) = self.tx_countdown.take() {
            if elapsed_ticks >= remaining {
                self.control |= if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_TI
                } else {
                    SCON_TI
                };
                self.tx_queue.push_back(byte);
                sent.push(byte);
            } else {
                self.tx_countdown = Some((remaining - elapsed_ticks, byte));
            }
        }

        if self.rx_countdown.is_none()
            && let Some(byte) = self.rx_queue.pop_front()
        {
            self.rx_countdown = Some((self.frame_ticks, byte));
        }

        if let Some((remaining, byte)) = self.rx_countdown.take() {
            if elapsed_ticks >= remaining {
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
                self.rx_countdown = Some((remaining - elapsed_ticks, byte));
            }
        }

        sent
    }

    fn feed_rx(&mut self, bytes: &[u8]) {
        self.rx_queue.extend(bytes.iter().copied());
    }

    fn take_tx_string(&mut self) -> String {
        let bytes = self.tx_queue.drain(..).collect::<Vec<_>>();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn tx_text(&self) -> String {
        String::from_utf8_lossy(self.tx_queue.as_slices().0).into_owned()
    }
}

#[derive(Debug, Default)]
struct BoardModel {
    ticks: u64,
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
    fn advance_ticks(&mut self, ticks: u64) {
        self.ticks = self.ticks.saturating_add(ticks);
        self.ds1302.tick_ticks(ticks);
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
            (p1 & (1 << 3)) != 0,
            (p1 & (1 << 7)) != 0,
            (p2 & (1 << 3)) != 0,
        );
        self.ds18b20.sample(self.ticks, (p1 & (1 << 4)) != 0);
        self.i2c.sample(
            (p2 & (1 << 0)) != 0,
            (p2 & (1 << 1)) != 0,
            &self.analog,
            &mut self.pcf8591,
            &mut self.at24c02,
        );
        self.outputs
            .sample_from_latches(board_latches, board_latch_versions);
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
                value = apply_open_drain_bit(value, 0, self.i2c.scl_drive_low);
                value = apply_open_drain_bit(value, 1, self.i2c.sda_drive_low);
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
        self.ne555.level(self.ticks)
    }

    fn t0_falling_edges(&self, all_latches: &[u8; 6], start_ticks: u64) -> u32 {
        let end_ticks = self.ticks;
        if end_ticks <= start_ticks {
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

        self.ne555.falling_edges_between(start_ticks, end_ticks)
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
        assert_eq!(
            sim.watch_led_changes(LedId::L1, 1000)
                .expect("watch L1 changes"),
            29
        );
    }

    #[test]
    fn net_sig_requires_explicit_jumper_to_reach_p34() {
        let mut board = super::BoardModel::default();
        let latches = default_port_latches();
        board.ne555.set_frequency_hz(2_200.0);

        let saw_low_without_bridge = (0..20_000_u64).any(|tick| {
            board.ticks = tick;
            !p34_level(&board, latches)
        });
        assert!(!saw_low_without_bridge);

        board
            .jumper_on(SignalId::NetSig, SignalId::SigOut)
            .expect("install NET_SIG to SIG_OUT jumper");
        let saw_low_with_bridge = (0..20_000_u64).any(|tick| {
            board.ticks = tick;
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
        board.ticks = 0;

        assert!(!p34_level(&board, latches));
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
    fn na16_shows_boot_time_at_50ms() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("sample/na16/prj/Objects/na16.hex"), false)
                .expect("load na16");

        sim.run_ms(50).expect("run na16 to 50ms");
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
