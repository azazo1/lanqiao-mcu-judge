use std::{
    collections::VecDeque,
    fmt::Write as _,
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use i8051::{
    Cpu, CpuContext, CpuView, Instruction, MemoryMapper, PortMapper, ReadOnlyMemoryMapper, Register,
};
use tracing::trace;

use crate::{
    hex::load_ihex,
    ids::{KeyId, LedId, VoltageChannel},
    peripherals::{
        AnalogInputs, Ds1302, Ds18b20, I2cBus, KeyMatrix, Outputs, Rtc, SegmentDecoder,
        UltrasonicDevice,
    },
    registers::*,
    timing::{CPU_TICKS_PER_US, TICKS_PER_SECOND},
};

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
            self.step_once();
        }
        Ok(())
    }

    pub fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.ctx.board.set_key(name, pressed)
    }

    pub fn set_key_id(&mut self, key: KeyId, pressed: bool) {
        self.ctx.board.key_matrix.set_key_id(key, pressed);
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
        let target = self
            .ctx
            .board
            .ticks
            .saturating_add(duration_ms.saturating_mul(1_000).saturating_mul(CPU_TICKS_PER_US));
        let mut previous = self.led_on_id(led);
        let mut changes = 0_u64;

        while self.ctx.board.ticks < target {
            self.step_once();
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

        let target = self
            .ctx
            .board
            .ticks
            .saturating_add(duration_ms.saturating_mul(1_000).saturating_mul(CPU_TICKS_PER_US));
        while self.ctx.board.ticks < target {
            self.step_once();
            let current = self.display_text();
            if current != initial {
                bail!(
                    "display_text 在观察窗口内发生变化: 初始 `{initial}`, 后续 `{current}`"
                );
            }
        }
        Ok(initial)
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
        let _ = writeln!(out, "buzzer: {}", self.buzzer_on());
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
            let board_latch_versions = self.ctx.effective_board_latch_versions();
            self.ctx
                .board
                .tick_protocols(&self.ctx.ports, &board_latches, &board_latch_versions);
            self.ctx.ports.refresh_inputs(&self.ctx.board);
        }
    }

    fn current_instruction_ticks(&self) -> u32 {
        let instruction = self.cpu.decode_pc(&self.ctx);
        approximate_instruction_ticks(&instruction)
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

#[derive(Default)]
struct BoardXdata {
    ram: Vec<u8>,
    board_latches: [u8; 4],
    board_latch_versions: [u64; 4],
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
            board_latch_versions: [0; 4],
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
            &mut self.rtc,
        );
        self.ds18b20.sample(self.ticks, (p1 & (1 << 4)) != 0);
        self.i2c.sample(
            (p2 & (1 << 0)) != 0,
            (p2 & (1 << 1)) != 0,
            &self.analog,
        );
        self.outputs
            .sample_from_latches(board_latches, board_latch_versions);
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
                value = apply_open_drain_bit(value, 2, self.key_matrix.col_low(1, all_latches));
                value = apply_open_drain_bit(value, 4, self.key_matrix.col_low(0, all_latches));
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::ids::LedId;

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
            sim.watch_led_changes(LedId::L1, 1000)
                .expect("watch L1 changes"),
            29
        );
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
    fn na16_shows_boot_time_at_50ms() {
        let mut sim =
            Simulator::from_hex_path(&sample_path("sample/na16/prj/Objects/na16.hex"), false)
                .expect("load na16");

        sim.run_ms(50).expect("run na16 to 50ms");
        assert_eq!(sim.display_text(), "23-59-50");
    }
}
