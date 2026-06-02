use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Write as _,
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use i8051::{
    Cpu, CpuContext, CpuView, Instruction, MemoryMapper, PortMapper, ReadOnlyMemoryMapper, Register,
};
use tracing::trace;

use crate::hex::load_ihex;

const CPU_TICKS_PER_US: u64 = 35;
const TICKS_PER_SECOND: u64 = 1_000_000 * CPU_TICKS_PER_US;
const UART1_SFR_SCON: u8 = 0x98;
const UART1_SFR_SBUF: u8 = 0x99;
const UART2_SFR_S2CON: u8 = 0x9A;
const UART2_SFR_S2BUF: u8 = 0x9B;

const SFR_P0: u8 = 0x80;
const SFR_P1: u8 = 0x90;
const SFR_P2: u8 = 0xA0;
const SFR_P3: u8 = 0xB0;
const SFR_P4: u8 = 0xC0;
const SFR_P5: u8 = 0xC8;
const SFR_CCON: u8 = 0xD8;
const SFR_CMOD: u8 = 0xD9;
const SFR_TCON: u8 = 0x88;
const SFR_TMOD: u8 = 0x89;
const SFR_TL0: u8 = 0x8A;
const SFR_TL1: u8 = 0x8B;
const SFR_TH0: u8 = 0x8C;
const SFR_TH1: u8 = 0x8D;
const SFR_AUXR: u8 = 0x8E;
const SFR_INT_CLKO: u8 = 0x8F;
const SFR_CLK_DIV: u8 = 0x97;
const SFR_AUXR1: u8 = 0xA2;
const SFR_IE2: u8 = 0xAF;
const SFR_P3M1: u8 = 0xB1;
const SFR_P3M0: u8 = 0xB2;
const SFR_P4M1: u8 = 0xB3;
const SFR_P4M0: u8 = 0xB4;
const SFR_P_SW2: u8 = 0xBA;
const SFR_ADC_CONTR: u8 = 0xBC;
const SFR_ADC_RES: u8 = 0xBD;
const SFR_ADC_RESL: u8 = 0xBE;
const SFR_P5M1: u8 = 0xC9;
const SFR_P5M0: u8 = 0xCA;
const SFR_T2H: u8 = 0xD6;
const SFR_T2L: u8 = 0xD7;
const SFR_CL: u8 = 0xE9;
const SFR_CH: u8 = 0xF9;

const P3_INT0: u8 = 1 << 2;
const P3_INT1: u8 = 1 << 3;
const P3_T0: u8 = 1 << 4;
const P3_T1: u8 = 1 << 5;

const TCON_TR0: u8 = 1 << 4;
const TCON_TF0: u8 = 1 << 5;
const TCON_TR1: u8 = 1 << 6;
const TCON_TF1: u8 = 1 << 7;

const TMOD_GATE0: u8 = 1 << 3;
const TMOD_C_T0: u8 = 1 << 2;
const TMOD_GATE1: u8 = 1 << 7;
const TMOD_C_T1: u8 = 1 << 6;

const SCON_REN: u8 = 1 << 4;
const SCON_TI: u8 = 1 << 1;
const SCON_RI: u8 = 1 << 0;

const S2CON_REN: u8 = 1 << 4;
const S2CON_TI: u8 = 1 << 1;
const S2CON_RI: u8 = 1 << 0;

const AUXR_EXTRAM: u8 = 1 << 1;
const CCON_CR: u8 = 1 << 6;
const CCON_CF: u8 = 1 << 7;

pub struct Simulator {
    cpu: Cpu,
    ctx: MachineContext,
    trace_cpu: bool,
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
            self.step_once();
        }
        Ok(())
    }

    pub fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.ctx.board.set_key(name, pressed)
    }

    pub fn tap_key(&mut self, name: &str, hold_ms: u64) -> Result<()> {
        self.set_key(name, true)?;
        self.run_ms(hold_ms)?;
        self.set_key(name, false)?;
        self.run_ms(30)?;
        Ok(())
    }

    pub fn set_rtc(&mut self, hour: u8, minute: u8, second: u8) -> Result<()> {
        self.ctx.board.rtc.set_hms(hour, minute, second)
    }

    pub fn set_temperature_c(&mut self, value: f32) {
        self.ctx.board.ds18b20.temperature_c = value;
    }

    pub fn set_distance_cm(&mut self, value: f32) {
        self.ctx.board.ultrasonic.distance_cm = value.max(0.0);
    }

    pub fn set_frequency_hz(&mut self, value: f32) {
        self.ctx.board.frequency_hz = value.max(0.0);
    }

    pub fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.ctx.board.set_voltage(name, value)
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

    pub fn led_on(&self, index: usize) -> Result<bool> {
        if !(1..=8).contains(&index) {
            bail!("LED 编号必须在 1..=8");
        }
        Ok(self.ctx.board.outputs.leds[index - 1])
    }

    pub fn count_line_changes(&mut self, name: &str, duration_ms: u64) -> Result<u64> {
        let line = ObservedLine::parse(name)?;
        let sample_us = 100;
        let mut remaining_us = duration_ms.saturating_mul(1_000);
        let mut previous = self.read_observed_line(line);
        let mut changes = 0_u64;

        while remaining_us > 0 {
            let step_us = remaining_us.min(sample_us);
            self.run_us(step_us)?;
            remaining_us -= step_us;

            let current = self.read_observed_line(line);
            if current != previous {
                changes += 1;
                previous = current;
            }
        }

        Ok(changes)
    }

    pub fn line_change_frequency_hz(&mut self, name: &str, duration_ms: u64) -> Result<f64> {
        if duration_ms == 0 {
            bail!("统计时长必须 > 0");
        }
        let changes = self.count_line_changes(name, duration_ms)?;
        Ok(changes as f64 * 1_000.0 / duration_ms as f64)
    }

    pub fn display_text(&self) -> String {
        self.ctx.board.outputs.display_text()
    }

    pub fn snapshot_text(&self) -> String {
        let mut out = String::new();
        let board_latches = self.ctx.effective_board_latches();
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
            self.ctx.ports.timer.tcon,
            self.ctx.ports.timer.tmod,
            self.ctx.ports.timer.tl0,
            self.ctx.ports.timer.th0,
            self.ctx.ports.timer.tl1,
            self.ctx.ports.timer.th1,
            self.ctx.ports.generic_get(SFR_AUXR)
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
            "rtc: {:02}:{:02}:{:02}",
            self.ctx.board.rtc.hour, self.ctx.board.rtc.minute, self.ctx.board.rtc.second
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

    fn step_once(&mut self) {
        let ticks = self.current_instruction_ticks();
        self.ctx.ports.refresh_inputs(&self.ctx.board);
        if self.trace_cpu {
            let instruction = self.cpu.decode_pc(&self.ctx);
            trace!("{instruction:#}");
        }
        let _ = self.cpu.step(&mut self.ctx);
        self.tick_devices(ticks);
    }

    fn tick_devices(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.ctx.board.tick_rtc();
            let p3 = self.ctx.ports.sample_port_p3(&self.ctx.board);
            let auxr = self.ctx.ports.generic_get(SFR_AUXR);
            self.ctx.ports.timer.tick(p3, auxr);
            self.ctx.ports.tick_ultrasonic(&mut self.ctx.board);
            self.ctx.ports.uart1.tick();
            let responses = self.ctx.ports.uart2.tick();
            for response in responses {
                self.ctx.board.ultrasonic.push_response(response);
            }
            if let Some(response) = self.ctx.board.ultrasonic.pop_response() {
                self.ctx.ports.uart2.feed_rx(&[response]);
            }
            let board_latches = self.ctx.effective_board_latches();
            self.ctx
                .board
                .tick_protocols(&self.ctx.ports, &board_latches);
            self.ctx.ports.refresh_inputs(&self.ctx.board);
        }
    }

    fn current_instruction_ticks(&self) -> u32 {
        let instruction = self.cpu.decode_pc(&self.ctx);
        approximate_instruction_ticks(&instruction)
    }

    fn read_observed_line(&self, line: ObservedLine) -> bool {
        match line {
            ObservedLine::Led(index) => self.ctx.board.outputs.leds[index - 1],
            ObservedLine::Relay => self.ctx.board.outputs.relay_on,
        }
    }
}

#[derive(Clone, Copy)]
enum ObservedLine {
    Led(usize),
    Relay,
}

impl ObservedLine {
    fn parse(name: &str) -> Result<Self> {
        let upper = name.trim().to_ascii_uppercase();
        if upper == "RELAY" {
            return Ok(Self::Relay);
        }

        if let Some(suffix) = upper.strip_prefix('L') {
            let index = suffix
                .parse::<usize>()
                .with_context(|| format!("未知线路: {name}"))?;
            if (1..=8).contains(&index) {
                return Ok(Self::Led(index));
            }
        }

        bail!("未知线路: {name}")
    }
}

fn approximate_instruction_ticks(instruction: &Instruction) -> u32 {
    let op = instruction.bytes()[0];
    match op {
        0x00 => 1,
        0x02 | 0x12 | 0x22 | 0x32 => 2,
        0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 | 0x80 => 2,
        0x73 | 0x83 | 0x93 => 2,
        0xA4 | 0x84 => 4,
        0xE0 | 0xE2 | 0xE3 | 0xF0 | 0xF2 | 0xF3 => 2,
        _ => match instruction.len() {
            0 | 1 => 1,
            2 => 1,
            _ => 1,
        },
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
        Self {
            ports: MachinePorts::new(),
            xdata: BoardXdata::default(),
            code: CodeMemory { code },
            board: BoardModel::default(),
        }
    }

    fn effective_board_latches(&self) -> [u8; 4] {
        if self.ports.latch_used {
            self.ports.board_latches
        } else if self.xdata.latch_used {
            self.xdata.board_latches
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

#[derive(Default)]
struct BoardXdata {
    ram: Vec<u8>,
    board_latches: [u8; 4],
    latch_used: bool,
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
                self.latch_used = true;
            }
            0xA000 => {
                self.board_latches[1] = byte;
                self.latch_used = true;
            }
            0xC000 => {
                self.board_latches[2] = byte;
                self.latch_used = true;
            }
            0xE000 => {
                self.board_latches[3] = byte;
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
    latch_used: bool,
    timer: Timer01,
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
            board_latches: [0; 4],
            latch_used: false,
            timer: Timer01::default(),
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
        board.read_port(3, self.port_latch[3], self.port_latch)
    }

    fn refresh_inputs(&mut self, board: &BoardModel) {
        for index in 0..self.port_input.len() {
            self.port_input[index] =
                board.read_port(index, self.port_latch[index], self.port_latch);
        }
    }

    fn tick_ultrasonic(&mut self, board: &mut BoardModel) {
        let tx_high = self.port_latch[1] & (1 << 0) != 0;
        board.ultrasonic.sample_trigger(tx_high);

        let running = self.generic_get(SFR_CCON) & CCON_CR != 0;
        if !running {
            board.ultrasonic.stop_measurement();
            return;
        }

        let counter = u16::from_be_bytes([self.generic_get(SFR_CH), self.generic_get(SFR_CL)])
            .wrapping_add(1);
        let [ch, cl] = counter.to_be_bytes();
        self.generic_set(SFR_CH, ch);
        self.generic_set(SFR_CL, cl);
        if counter == 0 {
            self.generic_set(SFR_CCON, self.generic_get(SFR_CCON) | CCON_CF);
        }

        let timeout = self.generic_get(SFR_CMOD) & 0x80 != 0;
        board.ultrasonic.sample_counter(counter, timeout);
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
            SFR_TCON | SFR_TMOD | SFR_TL0 | SFR_TL1 | SFR_TH0 | SFR_TH1 => self.timer.read(addr),
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
        self.generic_get(addr)
    }

    fn prepare_write<C: CpuView>(&self, _cpu: &C, addr: u8, value: u8) -> Self::WriteValue {
        (addr, value)
    }

    fn write(&mut self, value: Self::WriteValue) {
        let (addr, byte) = value;
        match addr {
            SFR_TCON | SFR_TMOD | SFR_TL0 | SFR_TL1 | SFR_TH0 | SFR_TH1 => {
                self.timer.write(addr, byte)
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

#[derive(Debug, Default)]
struct Timer01 {
    tcon: u8,
    tmod: u8,
    tl0: u8,
    tl1: u8,
    th0: u8,
    th1: u8,
    rl_tl0: u8,
    rl_tl1: u8,
    rl_th0: u8,
    rl_th1: u8,
    div0: u8,
    div1: u8,
    prev_p3: u8,
}

impl Timer01 {
    fn read(&self, addr: u8) -> u8 {
        match addr {
            SFR_TCON => self.tcon,
            SFR_TMOD => self.tmod,
            SFR_TL0 => self.tl0,
            SFR_TL1 => self.tl1,
            SFR_TH0 => self.th0,
            SFR_TH1 => self.th1,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u8, value: u8) {
        match addr {
            SFR_TCON => self.tcon = value,
            SFR_TMOD => self.tmod = value,
            SFR_TL0 => {
                self.tl0 = value;
                self.rl_tl0 = value;
            }
            SFR_TL1 => {
                self.tl1 = value;
                self.rl_tl1 = value;
            }
            SFR_TH0 => {
                self.th0 = value;
                self.rl_th0 = value;
            }
            SFR_TH1 => {
                self.th1 = value;
                self.rl_th1 = value;
            }
            _ => {}
        }
    }

    fn tick(&mut self, p3: u8, auxr: u8) {
        self.tick_timer0(p3, auxr);
        self.tick_timer1(p3, auxr);
        self.prev_p3 = p3;
    }

    fn tick_timer0(&mut self, p3: u8, auxr: u8) {
        let tr0 = (self.tcon & TCON_TR0) != 0;
        if !tr0 {
            self.div0 = 0;
            return;
        }
        let gate0 = (self.tmod & TMOD_GATE0) != 0;
        let counter0 = (self.tmod & TMOD_C_T0) != 0;
        if gate0 && (p3 & P3_INT0 == 0) {
            self.div0 = 0;
            return;
        }
        let should_tick = if counter0 {
            self.prev_p3 & P3_T0 != 0 && p3 & P3_T0 == 0
        } else {
            Self::timer_tick_ready(auxr & 0x80 != 0, &mut self.div0)
        };
        if !should_tick {
            return;
        }
        match self.tmod & 0x03 {
            0x00 => {
                let next = u16::from_be_bytes([self.th0, self.tl0]).wrapping_add(1);
                if next == 0 {
                    let [th0, tl0] = [self.rl_th0, self.rl_tl0];
                    self.th0 = th0;
                    self.tl0 = tl0;
                    self.tcon |= TCON_TF0;
                } else {
                    let [th0, tl0] = next.to_be_bytes();
                    self.th0 = th0;
                    self.tl0 = tl0;
                }
            }
            0x01 => {
                let next = u16::from_be_bytes([self.th0, self.tl0]).wrapping_add(1);
                let [th0, tl0] = next.to_be_bytes();
                self.th0 = th0;
                self.tl0 = tl0;
                if next == 0 {
                    self.tcon |= TCON_TF0;
                }
            }
            0x02 => {
                self.tl0 = self.tl0.wrapping_add(1);
                if self.tl0 == 0 {
                    self.tl0 = self.th0;
                    self.tcon |= TCON_TF0;
                }
            }
            _ => {}
        }
    }

    fn tick_timer1(&mut self, p3: u8, auxr: u8) {
        let tr1 = (self.tcon & TCON_TR1) != 0;
        if !tr1 {
            self.div1 = 0;
            return;
        }
        let gate1 = (self.tmod & TMOD_GATE1) != 0;
        let counter1 = (self.tmod & TMOD_C_T1) != 0;
        if gate1 && (p3 & P3_INT1 == 0) {
            self.div1 = 0;
            return;
        }
        let should_tick = if counter1 {
            self.prev_p3 & P3_T1 != 0 && p3 & P3_T1 == 0
        } else {
            Self::timer_tick_ready(auxr & 0x40 != 0, &mut self.div1)
        };
        if !should_tick {
            return;
        }
        match (self.tmod >> 4) & 0x03 {
            0x00 => {
                let next = u16::from_be_bytes([self.th1, self.tl1]).wrapping_add(1);
                if next == 0 {
                    let [th1, tl1] = [self.rl_th1, self.rl_tl1];
                    self.th1 = th1;
                    self.tl1 = tl1;
                    self.tcon |= TCON_TF1;
                } else {
                    let [th1, tl1] = next.to_be_bytes();
                    self.th1 = th1;
                    self.tl1 = tl1;
                }
            }
            0x01 => {
                let next = u16::from_be_bytes([self.th1, self.tl1]).wrapping_add(1);
                let [th1, tl1] = next.to_be_bytes();
                self.th1 = th1;
                self.tl1 = tl1;
                if next == 0 {
                    self.tcon |= TCON_TF1;
                }
            }
            0x02 => {
                self.tl1 = self.tl1.wrapping_add(1);
                if self.tl1 == 0 {
                    self.tl1 = self.th1;
                    self.tcon |= TCON_TF1;
                }
            }
            _ => {}
        }
    }

    fn timer_tick_ready(one_t: bool, divider: &mut u8) -> bool {
        if one_t {
            *divider = 0;
            return true;
        }
        *divider = divider.saturating_add(1);
        if *divider >= 12 {
            *divider = 0;
            true
        } else {
            false
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

    fn tick(&mut self) -> Vec<u8> {
        let mut sent = Vec::new();

        if let Some((ticks, byte)) = self.tx_countdown.take() {
            if ticks <= 1 {
                self.control |= if self.scon_addr == UART2_SFR_S2CON {
                    S2CON_TI
                } else {
                    SCON_TI
                };
                self.tx_queue.push_back(byte);
                sent.push(byte);
            } else {
                self.tx_countdown = Some((ticks - 1, byte));
            }
        }

        if self.rx_countdown.is_none()
            && let Some(byte) = self.rx_queue.pop_front()
        {
            self.rx_countdown = Some((self.frame_ticks, byte));
        }

        if let Some((ticks, byte)) = self.rx_countdown.take() {
            if ticks <= 1 {
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
                self.rx_countdown = Some((ticks - 1, byte));
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
    rtc: Rtc,
    outputs: Outputs,
    ds18b20: Ds18b20,
    ds1302: Ds1302,
    i2c: I2cBus,
    ultrasonic: UltrasonicDevice,
    key_matrix: KeyMatrix,
    analog: AnalogInputs,
    frequency_hz: f32,
}

impl BoardModel {
    fn tick_rtc(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        self.rtc.tick();
    }

    fn tick_protocols(&mut self, ports: &MachinePorts, board_latches: &[u8; 4]) {
        let p1 = ports.port_latch[1];
        let p2 = ports.port_latch[2];
        self.ds1302.sample(
            self.ticks,
            (p1 & (1 << 3)) != 0,
            (p1 & (1 << 7)) != 0,
            (p2 & (1 << 3)) != 0,
            &mut self.rtc,
        );
        self.ds18b20.sample(self.ticks, (p1 & (1 << 4)) != 0);
        self.i2c.sample(
            self.ticks,
            (p2 & (1 << 0)) != 0,
            (p2 & (1 << 1)) != 0,
            &self.analog,
        );
        self.outputs.sample_from_latches(board_latches);
    }

    fn read_port(&self, index: usize, latch: u8, all_latches: [u8; 6]) -> u8 {
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
                value = apply_push_pull_bit(value, 4, self.frequency_level());
                let rows = [(0_u8, 0_u8), (1, 1), (2, 2), (3, 3)];
                for (bit, row) in rows {
                    let low = self.key_matrix.row_low(row, all_latches);
                    value = set_bit_level(value, bit, !low);
                }
                value = set_bit_level(value, 5, true);
            }
            4 => {
                value = set_bit_level(value, 2, !self.key_matrix.col_low(1, all_latches));
                value = set_bit_level(value, 4, !self.key_matrix.col_low(0, all_latches));
            }
            _ => {}
        }
        value
    }

    fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.key_matrix.set_key(name, pressed)
    }

    fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.analog.set_voltage(name, value)
    }

    fn frequency_level(&self) -> bool {
        if self.frequency_hz <= 0.0 {
            return true;
        }
        let effective_hz = self.frequency_hz * (CPU_TICKS_PER_US as f32 / 12.0);
        let period_ticks = (TICKS_PER_SECOND as f32 / effective_hz).max(1.0);
        let half = (period_ticks / 2.0).max(1.0) as u64;
        (self.ticks / half).is_multiple_of(2)
    }

    fn read_hall_level(&self) -> bool {
        true
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

#[derive(Debug, Default)]
struct Rtc {
    hour: u8,
    minute: u8,
    second: u8,
    sub_us: u64,
}

impl Rtc {
    fn set_hms(&mut self, hour: u8, minute: u8, second: u8) -> Result<()> {
        if hour > 23 || minute > 59 || second > 59 {
            bail!("RTC 时间越界");
        }
        self.hour = hour;
        self.minute = minute;
        self.second = second;
        self.sub_us = 0;
        Ok(())
    }

    fn tick(&mut self) {
        self.sub_us += 1;
        if self.sub_us < TICKS_PER_SECOND {
            return;
        }
        self.sub_us = 0;
        self.second += 1;
        if self.second < 60 {
            return;
        }
        self.second = 0;
        self.minute += 1;
        if self.minute < 60 {
            return;
        }
        self.minute = 0;
        self.hour = (self.hour + 1) % 24;
    }
}

#[derive(Debug, Default)]
struct Outputs {
    leds: [bool; 8],
    relay_on: bool,
    buzzer_on: bool,
    digits: [DigitSample; 8],
    segment_latch: u8,
    com_latch: u8,
}

impl Outputs {
    fn sample_from_latches(&mut self, latches: &[u8; 4]) {
        let led = latches[0];
        for bit in 0..8 {
            self.leds[bit] = led & (1 << bit) == 0;
        }

        let ctrl = latches[1];
        self.relay_on = ctrl & (1 << 4) != 0;
        self.buzzer_on = ctrl & (1 << 6) != 0;

        self.com_latch = latches[2];
        self.segment_latch = latches[3];

        for digit in 0..8 {
            if self.com_latch & (1 << digit) != 0 {
                if self.segment_latch == 0xFF && self.digits[digit].seen {
                    continue;
                }
                self.digits[digit].segments = self.segment_latch;
                self.digits[digit].seen = true;
            }
        }
    }

    fn display_text(&self) -> String {
        self.digits
            .iter()
            .map(|digit| digit.decode_char())
            .collect::<String>()
            .trim_end()
            .to_string()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DigitSample {
    segments: u8,
    seen: bool,
}

impl DigitSample {
    fn decode_char(&self) -> char {
        if !self.seen {
            return ' ';
        }
        let pattern = !self.segments;
        if pattern & 0x7F == 0 && pattern & 0x80 == 0 {
            return ' ';
        }
        match pattern & 0x7F {
            0x3F => '0',
            0x06 => '1',
            0x5B => '2',
            0x4F => '3',
            0x66 => '4',
            0x6D => '5',
            0x7D => '6',
            0x07 => '7',
            0x7F => '8',
            0x6F => '9',
            0x40 => '-',
            0x73 => 'P',
            0x79 => 'E',
            0x38 => 'L',
            0x71 => 'F',
            0x76 => 'H',
            0x39 => 'C',
            _ => {
                if pattern & 0x80 != 0 {
                    '.'
                } else {
                    '?'
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct KeyMatrix {
    pressed: [[bool; 4]; 4],
}

impl KeyMatrix {
    fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        let (row, col) = match name {
            "S4" => (3, 0),
            "S5" => (2, 0),
            "S6" => (1, 0),
            "S7" => (0, 0),
            "S8" => (3, 1),
            "S9" => (2, 1),
            "S10" => (1, 1),
            "S11" => (0, 1),
            "S12" => (3, 2),
            "S13" => (2, 2),
            "S14" => (1, 2),
            "S15" => (0, 2),
            "S16" => (3, 3),
            "S17" => (2, 3),
            "S18" => (1, 3),
            "S19" => (0, 3),
            _ => bail!("未知按键: {name}"),
        };
        self.pressed[row][col] = pressed;
        Ok(())
    }

    fn row_low(&self, row: u8, latches: [u8; 6]) -> bool {
        for col in 0..4 {
            if self.pressed[row as usize][col] && self.column_driven_low(col, latches) {
                return true;
            }
        }
        false
    }

    fn col_low(&self, col: u8, latches: [u8; 6]) -> bool {
        for row in 0..4 {
            if self.pressed[row][col as usize] && self.row_driven_low(row, latches) {
                return true;
            }
        }
        false
    }

    fn row_driven_low(&self, row: usize, latches: [u8; 6]) -> bool {
        let p3 = latches[3];
        match row {
            0 => p3 & (1 << 0) == 0,
            1 => p3 & (1 << 1) == 0,
            2 => p3 & (1 << 2) == 0,
            3 => p3 & (1 << 3) == 0,
            _ => false,
        }
    }

    fn column_driven_low(&self, col: usize, latches: [u8; 6]) -> bool {
        match col {
            0 => latches[4] & (1 << 4) == 0,
            1 => latches[4] & (1 << 2) == 0,
            2 => latches[3] & (1 << 5) == 0,
            3 => latches[3] & (1 << 4) == 0,
            _ => false,
        }
    }
}

#[derive(Debug)]
struct AnalogInputs {
    voltages: BTreeMap<String, f32>,
    eeprom: [u8; 256],
}

impl Default for AnalogInputs {
    fn default() -> Self {
        Self {
            voltages: BTreeMap::new(),
            eeprom: [0; 256],
        }
    }
}

impl AnalogInputs {
    fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        let key = match name {
            "RB2" | "rb2" => "RB2",
            "RB3" | "rb3" => "RB3",
            "RB4" | "rb4" => "RB4",
            "RD1" | "rd1" => "RD1",
            _ => bail!("未知电压通道: {name}"),
        };
        self.voltages.insert(key.to_string(), value.clamp(0.0, 5.0));
        Ok(())
    }

    fn channel_voltage(&self, channel: u8) -> f32 {
        match channel {
            0 => *self.voltages.get("RB3").unwrap_or(&0.0),
            1 => *self.voltages.get("RD1").unwrap_or(&0.0),
            2 => *self.voltages.get("RB4").unwrap_or(&0.0),
            3 => *self.voltages.get("RB2").unwrap_or(&0.0),
            _ => 0.0,
        }
    }

    fn channel_value(&self, channel: u8) -> u8 {
        ((self.channel_voltage(channel) / 5.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    }
}

#[derive(Debug)]
struct UltrasonicDevice {
    distance_cm: f32,
    pending_rx: VecDeque<u8>,
    tx_prev_high: bool,
    waiting_for_measure: bool,
    rx_high: bool,
    target_counter: Option<u16>,
}

impl Default for UltrasonicDevice {
    fn default() -> Self {
        Self {
            distance_cm: 0.0,
            pending_rx: VecDeque::new(),
            tx_prev_high: false,
            waiting_for_measure: false,
            rx_high: true,
            target_counter: None,
        }
    }
}

impl UltrasonicDevice {
    fn push_response(&mut self, tx: u8) {
        if tx == 0x55 {
            let distance_mm = (self.distance_cm.max(0.0) * 10.0).round() as u16;
            self.pending_rx.push_back((distance_mm >> 8) as u8);
            self.pending_rx.push_back((distance_mm & 0xFF) as u8);
        } else if tx == 0x50 {
            self.pending_rx.push_back(25_u8);
        }
    }

    fn pop_response(&mut self) -> Option<u8> {
        self.pending_rx.pop_front()
    }

    fn sample_trigger(&mut self, tx_high: bool) {
        if tx_high && !self.tx_prev_high {
            self.waiting_for_measure = true;
            self.rx_high = true;
            self.target_counter = None;
        } else if !tx_high && self.tx_prev_high && self.waiting_for_measure {
            self.target_counter = Some(self.distance_counter());
        }
        self.tx_prev_high = tx_high;
    }

    fn sample_counter(&mut self, counter: u16, timeout: bool) {
        if timeout {
            self.rx_high = false;
            self.target_counter = None;
            self.waiting_for_measure = false;
            return;
        }

        if let Some(target) = self.target_counter
            && counter >= target
        {
            self.rx_high = false;
            self.target_counter = None;
            self.waiting_for_measure = false;
        }
    }

    fn stop_measurement(&mut self) {
        self.rx_high = true;
        self.target_counter = None;
        self.waiting_for_measure = false;
    }

    fn distance_counter(&self) -> u16 {
        ((self.distance_cm.max(0.0) / 0.017) * CPU_TICKS_PER_US as f32)
            .round()
            .clamp(0.0, u16::MAX as f32) as u16
    }

    fn rx_level(&self) -> bool {
        self.rx_high
    }
}

#[derive(Debug, Default)]
struct Ds1302 {
    ce_prev: bool,
    clk_prev: bool,
    bit_count: u8,
    shift_in: u8,
    shift_out: u8,
    read_byte: u8,
    reading: bool,
    io_level: bool,
    current_reg: u8,
    data_phase: bool,
    burst_index: u8,
    write_protect: bool,
    last_write_reg: u8,
    last_write_value: u8,
    last_clock_write_reg: u8,
    last_clock_write_value: u8,
    last_read_reg: u8,
    last_read_value: u8,
}

impl Ds1302 {
    fn sample(&mut self, _ticks: u64, ce: bool, clk: bool, io: bool, rtc: &mut Rtc) {
        if !ce {
            self.ce_prev = ce;
            self.clk_prev = clk;
            self.bit_count = 0;
            self.shift_in = 0;
            self.shift_out = 0;
            self.read_byte = 0;
            self.reading = false;
            self.data_phase = false;
            self.burst_index = 0;
            self.io_level = true;
            return;
        }

        if !self.ce_prev && ce {
            self.bit_count = 0;
            self.shift_in = 0;
            self.shift_out = 0;
            self.read_byte = 0;
            self.reading = false;
            self.data_phase = false;
            self.burst_index = 0;
        }

        if !self.clk_prev && clk {
            if !self.data_phase {
                if io {
                    self.shift_in |= 1 << self.bit_count;
                }
                self.bit_count += 1;
                if self.bit_count == 8 {
                    self.current_reg = self.shift_in;
                    self.shift_in = 0;
                    self.bit_count = 0;
                    self.data_phase = true;
                    self.reading = self.current_reg & 0x01 != 0;
                    if self.reading {
                        self.read_byte = self.read_register(rtc);
                        self.last_read_reg = self.effective_reg();
                        self.last_read_value = self.read_byte;
                        self.shift_out = self.read_byte;
                    }
                }
            } else if !self.reading {
                if io {
                    self.shift_in |= 1 << self.bit_count;
                }
                self.bit_count += 1;
                if self.bit_count == 8 {
                    let value = self.shift_in;
                    self.shift_in = 0;
                    self.bit_count = 0;
                    self.write_register(value, rtc);
                    if self.is_clock_burst() {
                        self.burst_index = self.burst_index.saturating_add(1);
                    }
                }
            }
        }

        if self.clk_prev && !clk && self.data_phase && self.reading {
            self.io_level = self.shift_out & 1 != 0;
            self.shift_out >>= 1;
            self.bit_count += 1;
            if self.bit_count == 8 {
                self.bit_count = 0;
                if self.is_clock_burst() {
                    self.burst_index = self.burst_index.saturating_add(1);
                    self.read_byte = self.read_register(rtc);
                    self.last_read_reg = self.effective_reg();
                    self.last_read_value = self.read_byte;
                }
                self.shift_out = self.read_byte;
            }
        }

        self.ce_prev = ce;
        self.clk_prev = clk;
    }

    fn is_clock_burst(&self) -> bool {
        self.current_reg & 0xFE == 0xBE
    }

    fn effective_reg(&self) -> u8 {
        if self.is_clock_burst() {
            match self.burst_index {
                0 => 0x80,
                1 => 0x82,
                2 => 0x84,
                3 => 0x86,
                4 => 0x88,
                5 => 0x8A,
                6 => 0x8C,
                7 => 0x8E,
                _ => 0x8E,
            }
        } else {
            self.current_reg & 0xFE
        }
    }

    fn read_register(&self, rtc: &Rtc) -> u8 {
        match self.effective_reg() {
            0x80 => bcd(rtc.second),
            0x82 => bcd(rtc.minute),
            0x84 => encode_ds1302_hour(rtc.hour),
            0x86 => 1,
            0x88 => 1,
            0x8A => 1,
            0x8C => 0,
            0x8E => u8::from(self.write_protect) << 7,
            0x90 => 0,
            _ => 0,
        }
    }

    fn write_register(&mut self, value: u8, rtc: &mut Rtc) {
        let reg = self.effective_reg();
        if self.write_protect && reg != 0x8E {
            return;
        }
        match reg {
            0x80 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.second = decode_bcd(value & 0x7F).min(59);
                rtc.sub_us = 0;
            }
            0x82 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.minute = decode_bcd(value & 0x7F).min(59);
            }
            0x84 => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.last_clock_write_reg = reg;
                self.last_clock_write_value = value;
                rtc.hour = decode_ds1302_hour(value);
            }
            0x8E => {
                self.last_write_reg = reg;
                self.last_write_value = value;
                self.write_protect = value & 0x80 != 0;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
struct Ds18b20 {
    drive_low: bool,
    temperature_c: f32,
    line_prev: bool,
    low_since: Option<u64>,
    awaiting_command: bool,
    command_bits: u8,
    command_value: u8,
    output_bits: VecDeque<bool>,
    read_slot_until: Option<u64>,
}

impl Ds18b20 {
    fn sample(&mut self, ticks: u64, line_high: bool) {
        if !line_high && self.line_prev {
            self.low_since = Some(ticks);
        }

        if line_high
            && !self.line_prev
            && let Some(start) = self.low_since.take()
        {
            let width = ticks.saturating_sub(start);
            if width >= 400 * CPU_TICKS_PER_US {
                self.drive_low = false;
                self.awaiting_command = true;
                self.command_bits = 0;
                self.command_value = 0;
                self.read_slot_until = Some(ticks + 120 * CPU_TICKS_PER_US);
                self.drive_low = true;
            } else if self.awaiting_command {
                let bit = width < 15 * CPU_TICKS_PER_US;
                if bit {
                    self.command_value |= 1 << self.command_bits;
                }
                self.command_bits += 1;
                if self.command_bits == 8 {
                    self.handle_command();
                    self.command_bits = 0;
                    self.command_value = 0;
                }
            } else if let Some(bit) = self.output_bits.pop_front() {
                self.drive_low = !bit;
                self.read_slot_until = Some(ticks + 45 * CPU_TICKS_PER_US);
            }
        }

        if let Some(until) = self.read_slot_until
            && ticks >= until
        {
            self.drive_low = false;
            self.read_slot_until = None;
        }

        self.line_prev = line_high;
    }

    fn handle_command(&mut self) {
        match self.command_value {
            0xCC => {
                self.awaiting_command = true;
            }
            0x44 => {
                self.awaiting_command = false;
            }
            0xBE => {
                self.awaiting_command = false;
                let bytes = self.scratchpad();
                self.output_bits.clear();
                for byte in bytes {
                    for bit in 0..8 {
                        self.output_bits.push_back(byte & (1 << bit) != 0);
                    }
                }
            }
            _ => {
                self.awaiting_command = false;
            }
        }
    }

    fn scratchpad(&self) -> [u8; 9] {
        let raw = (self.temperature_c * 16.0).round() as i16;
        [
            raw as u8,
            (raw >> 8) as u8,
            0x4B,
            0x46,
            0x7F,
            0xFF,
            0x0C,
            0x10,
            0x00,
        ]
    }
}

#[derive(Debug, Default)]
struct I2cBus {
    scl_prev: bool,
    sda_prev: bool,
    bit_count: u8,
    shift: u8,
    sda_drive_low: bool,
    scl_drive_low: bool,
    active: Option<I2cDevice>,
    mode: I2cMode,
    pcf8591_control: u8,
    eeprom_addr: u8,
    read_buffer: VecDeque<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum I2cMode {
    #[default]
    Idle,
    Address,
    Write,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum I2cDevice {
    Pcf8591,
    Eeprom,
}

impl I2cBus {
    fn sample(&mut self, _ticks: u64, scl_high: bool, sda_high: bool, analog: &AnalogInputs) {
        if self.sda_prev && !sda_high && scl_high {
            self.mode = I2cMode::Address;
            self.bit_count = 0;
            self.shift = 0;
            self.read_buffer.clear();
            self.sda_drive_low = false;
        }
        if !self.sda_prev && sda_high && scl_high {
            self.mode = I2cMode::Idle;
            self.active = None;
            self.sda_drive_low = false;
        }

        if !self.scl_prev && scl_high {
            match self.mode {
                I2cMode::Address | I2cMode::Write => {
                    self.shift = (self.shift << 1) | u8::from(sda_high);
                    self.bit_count += 1;
                    if self.bit_count == 8 {
                        self.handle_received_byte(analog);
                        self.bit_count = 0;
                        self.shift = 0;
                        self.sda_drive_low = true;
                    }
                }
                I2cMode::Read => {
                    if self.bit_count == 0
                        && let Some(byte) = self.read_buffer.front().copied()
                    {
                        self.shift = byte;
                    }
                    self.bit_count += 1;
                    if self.bit_count > 8 {
                        self.bit_count = 0;
                        self.sda_drive_low = false;
                        self.read_buffer.pop_front();
                    } else {
                        let bit = self.shift & 0x80 != 0;
                        self.sda_drive_low = !bit;
                        self.shift <<= 1;
                    }
                }
                I2cMode::Idle => {}
            }
        }

        if self.scl_prev && !scl_high && self.sda_drive_low {
            self.sda_drive_low = false;
        }

        self.scl_prev = scl_high;
        self.sda_prev = sda_high;
    }

    fn handle_received_byte(&mut self, analog: &AnalogInputs) {
        match self.mode {
            I2cMode::Address => {
                let address = self.shift >> 1;
                let read = self.shift & 1 != 0;
                self.active = match address {
                    0x48 => Some(I2cDevice::Pcf8591),
                    0x50 => Some(I2cDevice::Eeprom),
                    _ => None,
                };
                match (self.active, read) {
                    (Some(I2cDevice::Pcf8591), true) => {
                        self.mode = I2cMode::Read;
                        let value = analog.channel_value(self.pcf8591_control & 0x03);
                        self.read_buffer.push_back(value);
                    }
                    (Some(I2cDevice::Pcf8591), false) => {
                        self.mode = I2cMode::Write;
                    }
                    (Some(I2cDevice::Eeprom), true) => {
                        self.mode = I2cMode::Read;
                        self.read_buffer
                            .push_back(analog.eeprom[self.eeprom_addr as usize]);
                    }
                    (Some(I2cDevice::Eeprom), false) => {
                        self.mode = I2cMode::Write;
                    }
                    (None, _) => {
                        self.mode = I2cMode::Idle;
                    }
                }
            }
            I2cMode::Write => match self.active {
                Some(I2cDevice::Pcf8591) => {
                    self.pcf8591_control = self.shift;
                }
                Some(I2cDevice::Eeprom) => {
                    self.eeprom_addr = self.shift;
                }
                None => {}
            },
            I2cMode::Idle | I2cMode::Read => {}
        }
    }
}

fn bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

fn decode_bcd(value: u8) -> u8 {
    ((value >> 4) * 10) + (value & 0x0F)
}

fn encode_ds1302_hour(hour: u8) -> u8 {
    bcd(hour % 24)
}

fn decode_ds1302_hour(value: u8) -> u8 {
    if value & 0x80 == 0 {
        return decode_bcd(value & 0x3F).min(23);
    }
    let mut hour = decode_bcd(value & 0x1F).clamp(1, 12);
    if value & 0x20 != 0 {
        hour = (hour % 12) + 12;
    } else if hour == 12 {
        hour = 0;
    }
    hour
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Simulator;

    fn sample_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
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
            sim.count_line_changes("L1", 1000)
                .expect("count L1 changes"),
            29
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
}
