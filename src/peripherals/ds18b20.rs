use std::collections::VecDeque;

use anyhow::{Result, bail};

use crate::chip::NS_PER_MICROSECOND;
use crate::event::{
    gate::{EventGate, SharedEventGate},
    track::EventTrack,
};
use crate::persistent_state::Ds18b20PersistentState;
use crate::wave::{WaveCaptureWindow, WaveEventNote};

const RESET_PULSE_MIN_NS: u64 = 400 * NS_PER_MICROSECOND;
const PRESENCE_PULSE_NS: u64 = 120 * NS_PER_MICROSECOND;
const READ_SLOT_DRIVE_NS: u64 = 45 * NS_PER_MICROSECOND;
const WRITE_ONE_MAX_NS: u64 = 15 * NS_PER_MICROSECOND;
const DEFAULT_TH: u8 = 0x4B;
const DEFAULT_TL: u8 = 0x46;
const DEFAULT_CONFIG: u8 = 0x7F;
const POWER_ON_TEMP_RAW: i16 = 0x0550;
const RECALL_E2_NS: u64 = 1_000 * NS_PER_MICROSECOND;
const CONVERT_9BIT_NS: u64 = 93_750 * NS_PER_MICROSECOND;
const CONVERT_10BIT_NS: u64 = 187_500 * NS_PER_MICROSECOND;
const CONVERT_11BIT_NS: u64 = 375_000 * NS_PER_MICROSECOND;
const CONVERT_12BIT_NS: u64 = 750_000 * NS_PER_MICROSECOND;

#[derive(Debug)]
pub(crate) struct Ds18b20 {
    pub(crate) drive_low: bool,
    pub(crate) temperature_c: f32,
    line_prev: bool,
    low_since: Option<u64>,
    bus_state: BusState,
    input_bits: u8,
    input_value: u8,
    tx_bits: VecDeque<bool>,
    read_slot_until: Option<u64>,
    status_response: StatusResponse,
    convert_busy_until: Option<u64>,
    recall_busy_until: Option<u64>,
    parasite_power: bool,
    rom: [u8; 8],
    temperature_raw: i16,
    alarm_flag: bool,
    scratchpad_th: u8,
    scratchpad_tl: u8,
    scratchpad_config: u8,
    eeprom_th: u8,
    eeprom_tl: u8,
    eeprom_config: u8,
    event_gate: SharedEventGate,
    event_notes: Vec<WaveEventNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BusState {
    #[default]
    IdleUntilReset,
    AwaitRomCommand,
    MatchRom {
        byte_index: u8,
        candidate: [u8; 8],
    },
    AwaitFunctionCommand,
    WriteScratchpad {
        byte_index: u8,
    },
    SearchRom {
        bit_index: u8,
        stage: SearchStage,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchStage {
    SendBit,
    SendComplement,
    ReceiveSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum StatusResponse {
    #[default]
    None,
    Convert,
    RecallE2,
    PowerSupply,
}

impl Default for Ds18b20 {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Ds18b20 {
    pub(crate) fn new(wave_enabled: bool) -> Self {
        Self::new_with_event_gate(EventGate::shared(WaveCaptureWindow::from_enabled(
            wave_enabled,
        )))
    }

    pub(crate) fn new_with_event_gate(event_gate: SharedEventGate) -> Self {
        Self {
            drive_low: false,
            temperature_c: 0.0,
            line_prev: false,
            low_since: None,
            bus_state: BusState::IdleUntilReset,
            input_bits: 0,
            input_value: 0,
            tx_bits: VecDeque::new(),
            read_slot_until: None,
            status_response: StatusResponse::None,
            convert_busy_until: None,
            recall_busy_until: None,
            parasite_power: false,
            rom: default_rom(),
            temperature_raw: POWER_ON_TEMP_RAW,
            alarm_flag: false,
            scratchpad_th: DEFAULT_TH,
            scratchpad_tl: DEFAULT_TL,
            scratchpad_config: DEFAULT_CONFIG,
            eeprom_th: DEFAULT_TH,
            eeprom_tl: DEFAULT_TL,
            eeprom_config: DEFAULT_CONFIG,
            event_gate,
            event_notes: Vec::new(),
        }
    }

    pub(crate) fn sample(&mut self, time_ns: u64, line_high: bool) {
        self.finish_background_jobs(time_ns);

        if !line_high && self.line_prev {
            self.low_since = Some(time_ns);
        }

        if line_high
            && !self.line_prev
            && let Some(start) = self.low_since.take()
        {
            let width = time_ns.saturating_sub(start);
            if width >= RESET_PULSE_MIN_NS {
                self.handle_reset(time_ns);
            } else if self.search_waiting_for_selection() {
                self.handle_search_selection(width < WRITE_ONE_MAX_NS);
            } else if self.expects_input_byte() {
                self.push_input_bit(width < WRITE_ONE_MAX_NS, time_ns);
            } else {
                self.handle_read_slot(time_ns);
            }
        }

        if let Some(until) = self.read_slot_until
            && time_ns >= until
        {
            self.drive_low = false;
            self.read_slot_until = None;
        }

        self.line_prev = line_high;
    }

    pub(crate) fn set_rom_hex(&mut self, text: &str) -> Result<()> {
        let cleaned = text
            .chars()
            .filter(|ch| !matches!(ch, ' ' | '\t' | '\n' | '\r' | '_' | '-'))
            .collect::<String>();
        let cleaned = cleaned.strip_prefix("0x").unwrap_or(&cleaned);
        let cleaned = cleaned.strip_prefix("0X").unwrap_or(cleaned);

        let bytes = match cleaned.len() {
            14 => {
                let mut rom = [0_u8; 8];
                for (index, chunk) in cleaned.as_bytes().chunks(2).enumerate() {
                    rom[index] = parse_hex_byte(chunk)?;
                }
                rom[7] = crc8_maxim(&rom[..7]);
                rom
            }
            16 => {
                let mut rom = [0_u8; 8];
                for (index, chunk) in cleaned.as_bytes().chunks(2).enumerate() {
                    rom[index] = parse_hex_byte(chunk)?;
                }
                rom
            }
            _ => {
                bail!("DS18B20 ROM 需要 14 或 16 个十六进制字符");
            }
        };
        self.rom = bytes;
        Ok(())
    }

    pub(crate) fn set_parasite_power(&mut self, enabled: bool) {
        self.parasite_power = enabled;
    }

    pub(crate) fn parasite_power(&self) -> bool {
        self.parasite_power
    }

    pub(crate) fn persistent_state(&self) -> Ds18b20PersistentState {
        Ds18b20PersistentState {
            rom: self.rom,
            eeprom_th: self.eeprom_th,
            eeprom_tl: self.eeprom_tl,
            eeprom_config: self.eeprom_config,
        }
    }

    pub(crate) fn load_persistent_state(&mut self, state: &Ds18b20PersistentState) {
        self.rom = state.rom;
        self.eeprom_th = state.eeprom_th;
        self.eeprom_tl = state.eeprom_tl;
        self.eeprom_config = state.eeprom_config;
        self.scratchpad_th = state.eeprom_th;
        self.scratchpad_tl = state.eeprom_tl;
        self.scratchpad_config = state.eeprom_config;
        self.update_alarm_flag();
    }

    pub(crate) fn take_event_notes(&mut self) -> Vec<WaveEventNote> {
        std::mem::take(&mut self.event_notes)
    }

    fn handle_reset(&mut self, time_ns: u64) {
        self.drive_low = true;
        self.read_slot_until = Some(time_ns.saturating_add(PRESENCE_PULSE_NS));
        self.bus_state = BusState::AwaitRomCommand;
        self.input_bits = 0;
        self.input_value = 0;
        self.tx_bits.clear();
        self.status_response = StatusResponse::None;
        let presence_until = time_ns.saturating_add(PRESENCE_PULSE_NS);
        self.push_event_note(time_ns, || {
            WaveEventNote::with_detail(
                time_ns,
                EventTrack::Onewire.track_id(),
                "RESET",
                format!("presence_until={presence_until}ns"),
            )
        });
    }

    fn expects_input_byte(&self) -> bool {
        matches!(
            self.bus_state,
            BusState::AwaitRomCommand
                | BusState::MatchRom { .. }
                | BusState::AwaitFunctionCommand
                | BusState::WriteScratchpad { .. }
        )
    }

    fn search_waiting_for_selection(&self) -> bool {
        matches!(
            self.bus_state,
            BusState::SearchRom {
                stage: SearchStage::ReceiveSelection,
                ..
            }
        )
    }

    fn push_input_bit(&mut self, bit: bool, time_ns: u64) {
        if bit {
            self.input_value |= 1 << self.input_bits;
        }
        self.input_bits += 1;
        if self.input_bits == 8 {
            let value = self.input_value;
            self.input_bits = 0;
            self.input_value = 0;
            self.handle_input_byte(value, time_ns);
        }
    }

    fn handle_input_byte(&mut self, value: u8, time_ns: u64) {
        self.push_event_note(time_ns, || {
            WaveEventNote::new(
                time_ns,
                EventTrack::Onewire.track_id(),
                format!("RX 0x{value:02X}"),
            )
        });
        match self.bus_state {
            BusState::AwaitRomCommand => self.handle_rom_command(value, time_ns),
            BusState::MatchRom {
                byte_index,
                mut candidate,
            } => {
                candidate[byte_index as usize] = value;
                if byte_index == 7 {
                    self.bus_state = if candidate == self.rom {
                        BusState::AwaitFunctionCommand
                    } else {
                        BusState::IdleUntilReset
                    };
                } else {
                    self.bus_state = BusState::MatchRom {
                        byte_index: byte_index + 1,
                        candidate,
                    };
                }
            }
            BusState::AwaitFunctionCommand => self.handle_function_command(value, time_ns),
            BusState::WriteScratchpad { byte_index } => {
                self.handle_write_scratchpad_byte(byte_index, value)
            }
            BusState::SearchRom { .. } | BusState::IdleUntilReset => {}
        }
    }

    fn handle_rom_command(&mut self, value: u8, time_ns: u64) {
        self.push_event_note(time_ns, || {
            WaveEventNote::new(
                time_ns,
                EventTrack::Onewire.track_id(),
                format!("ROM {}", rom_command_name(value)),
            )
        });
        match value {
            0x33 => {
                let rom = self.rom;
                self.load_tx_bytes(time_ns, &rom);
                self.bus_state = BusState::IdleUntilReset;
            }
            0x55 => {
                self.bus_state = BusState::MatchRom {
                    byte_index: 0,
                    candidate: [0_u8; 8],
                };
            }
            0xCC => {
                self.bus_state = BusState::AwaitFunctionCommand;
            }
            0xF0 => {
                self.bus_state = BusState::SearchRom {
                    bit_index: 0,
                    stage: SearchStage::SendBit,
                };
            }
            0xEC => {
                self.bus_state = if self.alarm_flag {
                    BusState::SearchRom {
                        bit_index: 0,
                        stage: SearchStage::SendBit,
                    }
                } else {
                    BusState::IdleUntilReset
                };
            }
            _ => {
                self.bus_state = BusState::IdleUntilReset;
            }
        }
    }

    fn handle_function_command(&mut self, value: u8, time_ns: u64) {
        self.push_event_note(time_ns, || {
            WaveEventNote::new(
                time_ns,
                EventTrack::Onewire.track_id(),
                format!("FUNC {}", function_command_name(value)),
            )
        });
        match value {
            0x44 => {
                self.start_conversion(time_ns);
                self.status_response = StatusResponse::Convert;
                self.bus_state = BusState::IdleUntilReset;
            }
            0x4E => {
                self.bus_state = BusState::WriteScratchpad { byte_index: 0 };
            }
            0xBE => {
                let bytes = self.scratchpad();
                self.load_tx_bytes(time_ns, &bytes);
                self.bus_state = BusState::IdleUntilReset;
            }
            0x48 => {
                self.eeprom_th = self.scratchpad_th;
                self.eeprom_tl = self.scratchpad_tl;
                self.eeprom_config = self.scratchpad_config;
                self.bus_state = BusState::IdleUntilReset;
            }
            0xB8 => {
                self.recall_busy_until = Some(time_ns.saturating_add(RECALL_E2_NS));
                self.status_response = StatusResponse::RecallE2;
                self.bus_state = BusState::IdleUntilReset;
            }
            0xB4 => {
                self.status_response = StatusResponse::PowerSupply;
                self.bus_state = BusState::IdleUntilReset;
            }
            _ => {
                self.bus_state = BusState::IdleUntilReset;
            }
        }
    }

    fn handle_write_scratchpad_byte(&mut self, byte_index: u8, value: u8) {
        match byte_index {
            0 => {
                self.scratchpad_th = value;
                self.bus_state = BusState::WriteScratchpad { byte_index: 1 };
            }
            1 => {
                self.scratchpad_tl = value;
                self.bus_state = BusState::WriteScratchpad { byte_index: 2 };
            }
            2 => {
                self.scratchpad_config = (value & 0x60) | 0x1F;
                self.bus_state = BusState::IdleUntilReset;
            }
            _ => {
                self.bus_state = BusState::IdleUntilReset;
            }
        }
    }

    fn handle_read_slot(&mut self, time_ns: u64) {
        if let Some(bit) = self.read_search_bit() {
            self.drive_low = !bit;
            self.read_slot_until = Some(time_ns.saturating_add(READ_SLOT_DRIVE_NS));
            return;
        }

        if let Some(bit) = self.tx_bits.pop_front() {
            self.drive_low = !bit;
            self.read_slot_until = Some(time_ns.saturating_add(READ_SLOT_DRIVE_NS));
            return;
        }

        if let Some(bit) = self.status_bit() {
            self.drive_low = !bit;
            self.read_slot_until = Some(time_ns.saturating_add(READ_SLOT_DRIVE_NS));
        }
    }

    fn read_search_bit(&mut self) -> Option<bool> {
        match self.bus_state {
            BusState::SearchRom {
                bit_index,
                stage: SearchStage::SendBit,
            } => {
                self.bus_state = BusState::SearchRom {
                    bit_index,
                    stage: SearchStage::SendComplement,
                };
                Some(self.rom_bit(bit_index))
            }
            BusState::SearchRom {
                bit_index,
                stage: SearchStage::SendComplement,
            } => {
                self.bus_state = BusState::SearchRom {
                    bit_index,
                    stage: SearchStage::ReceiveSelection,
                };
                Some(!self.rom_bit(bit_index))
            }
            BusState::SearchRom {
                stage: SearchStage::ReceiveSelection,
                ..
            }
            | BusState::AwaitRomCommand
            | BusState::MatchRom { .. }
            | BusState::AwaitFunctionCommand
            | BusState::WriteScratchpad { .. }
            | BusState::IdleUntilReset => None,
        }
    }

    fn handle_search_selection(&mut self, bit: bool) {
        if let BusState::SearchRom {
            bit_index,
            stage: SearchStage::ReceiveSelection,
        } = self.bus_state
        {
            if bit == self.rom_bit(bit_index) {
                self.bus_state = if bit_index == 63 {
                    BusState::IdleUntilReset
                } else {
                    BusState::SearchRom {
                        bit_index: bit_index + 1,
                        stage: SearchStage::SendBit,
                    }
                };
            } else {
                self.bus_state = BusState::IdleUntilReset;
            }
        }
    }

    fn load_tx_bytes(&mut self, time_ns: u64, bytes: &[u8]) {
        self.tx_bits.clear();
        if !bytes.is_empty() {
            self.push_event_note(time_ns, || {
                let detail = bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                WaveEventNote::with_detail(
                    time_ns,
                    EventTrack::Onewire.track_id(),
                    format!("TX {} bytes", bytes.len()),
                    detail,
                )
            });
        }
        for &byte in bytes {
            for bit in 0..8 {
                self.tx_bits.push_back(byte & (1 << bit) != 0);
            }
        }
    }

    fn status_bit(&self) -> Option<bool> {
        match self.status_response {
            StatusResponse::None => None,
            StatusResponse::Convert => Some(if self.parasite_power {
                true
            } else {
                self.convert_busy_until.is_none()
            }),
            StatusResponse::RecallE2 => Some(self.recall_busy_until.is_none()),
            StatusResponse::PowerSupply => Some(!self.parasite_power),
        }
    }

    fn start_conversion(&mut self, time_ns: u64) {
        if self.convert_busy_until.is_none() {
            let done_at = time_ns.saturating_add(self.conversion_time_ns());
            self.convert_busy_until = Some(done_at);
            self.push_event_note(time_ns, || {
                WaveEventNote::with_detail(
                    time_ns,
                    EventTrack::Onewire.track_id(),
                    "CONVERT T",
                    format!("done_at={done_at}ns"),
                )
            });
        }
    }

    fn push_event_note<F>(&mut self, time_ns: u64, build: F)
    where
        F: FnOnce() -> WaveEventNote,
    {
        if self
            .event_gate
            .need_direct_event(EventTrack::Onewire, time_ns)
        {
            self.event_notes.push(build());
        }
    }

    fn finish_background_jobs(&mut self, time_ns: u64) {
        if let Some(until) = self.convert_busy_until
            && time_ns >= until
        {
            self.convert_busy_until = None;
            self.temperature_raw = self.quantized_temperature_raw();
            self.update_alarm_flag();
        }

        if let Some(until) = self.recall_busy_until
            && time_ns >= until
        {
            self.recall_busy_until = None;
            self.scratchpad_th = self.eeprom_th;
            self.scratchpad_tl = self.eeprom_tl;
            self.scratchpad_config = self.eeprom_config;
        }
    }

    fn conversion_time_ns(&self) -> u64 {
        match self.scratchpad_config & 0x60 {
            0x00 => CONVERT_9BIT_NS,
            0x20 => CONVERT_10BIT_NS,
            0x40 => CONVERT_11BIT_NS,
            _ => CONVERT_12BIT_NS,
        }
    }

    fn quantized_temperature_raw(&self) -> i16 {
        let raw_12bit = (self.temperature_c * 16.0).round() as i16;
        match self.scratchpad_config & 0x60 {
            0x00 => raw_12bit & !0x7,
            0x20 => raw_12bit & !0x3,
            0x40 => raw_12bit & !0x1,
            _ => raw_12bit,
        }
    }

    fn update_alarm_flag(&mut self) {
        let temperature_whole = (self.temperature_raw >> 4) as i8;
        let th = self.scratchpad_th as i8;
        let tl = self.scratchpad_tl as i8;
        self.alarm_flag = temperature_whole <= tl || temperature_whole >= th;
    }

    fn rom_bit(&self, bit_index: u8) -> bool {
        let byte = self.rom[(bit_index / 8) as usize];
        byte & (1 << (bit_index % 8)) != 0
    }

    fn scratchpad(&self) -> [u8; 9] {
        let mut bytes = [
            self.temperature_raw as u8,
            (self.temperature_raw >> 8) as u8,
            self.scratchpad_th,
            self.scratchpad_tl,
            self.scratchpad_config,
            0xFF,
            0x0C,
            0x10,
            0x00,
        ];
        bytes[8] = crc8_maxim(&bytes[..8]);
        bytes
    }
}

fn rom_command_name(value: u8) -> &'static str {
    match value {
        0x33 => "0x33 Read ROM",
        0x55 => "0x55 Match ROM",
        0xCC => "0xCC Skip ROM",
        0xF0 => "0xF0 Search ROM",
        0xEC => "0xEC Alarm Search",
        _ => "unknown",
    }
}

fn function_command_name(value: u8) -> &'static str {
    match value {
        0x44 => "0x44 Convert T",
        0x4E => "0x4E Write Scratchpad",
        0xBE => "0xBE Read Scratchpad",
        0x48 => "0x48 Copy Scratchpad",
        0xB8 => "0xB8 Recall E2",
        0xB4 => "0xB4 Read Power Supply",
        _ => "unknown",
    }
}

fn default_rom() -> [u8; 8] {
    let mut rom = [0_u8; 8];
    rom[..7].copy_from_slice(&[0x28, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB]);
    rom[7] = crc8_maxim(&rom[..7]);
    rom
}

fn parse_hex_byte(text: &[u8]) -> Result<u8> {
    let text =
        std::str::from_utf8(text).map_err(|err| anyhow::anyhow!("ROM 不是合法 UTF-8: {err}"))?;
    u8::from_str_radix(text, 16)
        .map_err(|err| anyhow::anyhow!("ROM 十六进制字节解析失败 `{text}`: {err}"))
}

fn crc8_maxim(bytes: &[u8]) -> u8 {
    let mut crc = 0_u8;
    for &byte in bytes {
        let mut in_byte = byte;
        for _ in 0..8 {
            let mix = (crc ^ in_byte) & 0x01;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            in_byte >>= 1;
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        dev: Ds18b20,
        time_ns: u64,
        line_high: bool,
    }

    impl Harness {
        fn new() -> Self {
            let mut harness = Self {
                dev: Ds18b20::default(),
                time_ns: 0,
                line_high: true,
            };
            harness.dev.sample(0, true);
            harness
        }

        fn reset(&mut self) {
            self.set_line(true);
            self.advance_us(20);
            self.set_line(false);
            self.advance_us(500);
            self.set_line(true);
            self.advance_us(80);
            assert!(!self.bus_level(), "presence pulse");
            self.advance_us(200);
            assert!(self.bus_level(), "presence release");
        }

        fn advance_us(&mut self, us: u64) {
            self.time_ns = self
                .time_ns
                .saturating_add(us.saturating_mul(NS_PER_MICROSECOND));
            self.dev.sample(self.time_ns, self.line_high);
        }

        fn set_line(&mut self, high: bool) {
            self.line_high = high;
            self.dev.sample(self.time_ns, high);
        }

        fn bus_level(&self) -> bool {
            self.line_high && !self.dev.drive_low
        }

        fn write_bit(&mut self, bit: bool) {
            self.set_line(false);
            self.advance_us(if bit { 5 } else { 65 });
            self.set_line(true);
            self.advance_us(10);
        }

        fn read_bit(&mut self) -> bool {
            self.set_line(false);
            self.advance_us(2);
            self.set_line(true);
            self.advance_us(2);
            let bit = self.bus_level();
            self.advance_us(70);
            bit
        }

        fn write_byte(&mut self, value: u8) {
            for bit in 0..8 {
                self.write_bit(value & (1 << bit) != 0);
            }
        }

        fn read_byte(&mut self) -> u8 {
            let mut value = 0_u8;
            for bit in 0..8 {
                if self.read_bit() {
                    value |= 1 << bit;
                }
            }
            value
        }
    }

    #[test]
    fn ds18b20_read_rom_returns_configured_rom() {
        let mut harness = Harness::new();
        harness.dev.set_rom_hex("28010203040506").expect("set rom");
        let expected = harness.dev.rom;

        harness.reset();
        harness.write_byte(0x33);

        let mut actual = [0_u8; 8];
        for byte in &mut actual {
            *byte = harness.read_byte();
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn ds18b20_search_rom_returns_bit_and_complement() {
        let mut harness = Harness::new();
        harness.dev.set_rom_hex("28112233445566").expect("set rom");
        let expected = harness.dev.rom;
        let mut discovered = [0_u8; 8];

        harness.reset();
        harness.write_byte(0xF0);

        for bit_index in 0..64_u8 {
            let bit = harness.read_bit();
            let complement = harness.read_bit();
            let expected_bit = expected[(bit_index / 8) as usize] & (1 << (bit_index % 8)) != 0;
            assert_eq!(bit, expected_bit, "search rom bit");
            assert_eq!(complement, !expected_bit, "search rom complement");
            if bit {
                discovered[(bit_index / 8) as usize] |= 1 << (bit_index % 8);
            }
            harness.write_bit(expected_bit);
        }

        assert_eq!(discovered, expected);
    }

    #[test]
    fn wave_disabled_does_not_buffer_events() {
        let mut harness = Harness {
            dev: Ds18b20::new(false),
            time_ns: 0,
            line_high: true,
        };
        harness.dev.sample(0, true);

        harness.reset();
        harness.write_byte(0x33);

        assert!(harness.dev.take_event_notes().is_empty());
    }

    #[test]
    fn ds18b20_match_rom_gates_function_commands() {
        let mut harness = Harness::new();
        harness.dev.set_rom_hex("28AABBCCDDEE11").expect("set rom");
        let expected = harness.dev.rom;
        let mut wrong = expected;
        wrong[3] ^= 0x55;

        harness.reset();
        harness.write_byte(0x55);
        for byte in wrong {
            harness.write_byte(byte);
        }
        harness.write_byte(0xB4);
        assert!(harness.read_bit(), "wrong match should not respond");

        harness.reset();
        harness.write_byte(0x55);
        for byte in expected {
            harness.write_byte(byte);
        }
        harness.write_byte(0xB4);
        assert!(harness.read_bit(), "external power should read high");
    }

    #[test]
    fn ds18b20_alarm_search_requires_alarm_flag() {
        let mut harness = Harness::new();
        let expected_bit0 = harness.dev.rom[0] & 1 != 0;

        harness.dev.temperature_c = 20.0;
        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x4E);
        harness.write_byte(10);
        harness.write_byte(0);
        harness.write_byte(0x7F);
        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x44);
        harness.advance_us(800_000);

        harness.reset();
        harness.write_byte(0xEC);
        assert_eq!(harness.read_bit(), expected_bit0, "alarm search bit0");
        assert_eq!(
            harness.read_bit(),
            !expected_bit0,
            "alarm search bit0 complement"
        );

        harness.dev.temperature_c = 5.0;
        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x44);
        harness.advance_us(800_000);

        harness.reset();
        harness.write_byte(0xEC);
        assert!(harness.read_bit(), "no alarm response should stay high");
        assert!(harness.read_bit(), "no alarm complement should stay high");
    }

    #[test]
    fn ds18b20_function_commands_update_scratchpad_and_status() {
        let mut harness = Harness::new();

        harness.dev.temperature_c = 25.9375;
        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x44);
        assert!(!harness.read_bit(), "convert should report busy");
        harness.advance_us(800_000);
        assert!(harness.read_bit(), "convert should report done");

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0xBE);
        let lo = harness.read_byte();
        let hi = harness.read_byte();
        assert_eq!([lo, hi], [0x9F, 0x01]);

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x4E);
        harness.write_byte(0x11);
        harness.write_byte(0xEE);
        harness.write_byte(0x5F);

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x48);

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0x4E);
        harness.write_byte(0x22);
        harness.write_byte(0xDD);
        harness.write_byte(0x1F);

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0xB8);
        assert!(!harness.read_bit(), "recall should report busy");
        harness.advance_us(2_000);
        assert!(harness.read_bit(), "recall should report done");

        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0xBE);
        let _temp_lo = harness.read_byte();
        let _temp_hi = harness.read_byte();
        let th = harness.read_byte();
        let tl = harness.read_byte();
        let config = harness.read_byte();
        assert_eq!(th, 0x11);
        assert_eq!(tl, 0xEE);
        assert_eq!(config, 0x5F);

        harness.dev.set_parasite_power(true);
        harness.reset();
        harness.write_byte(0xCC);
        harness.write_byte(0xB4);
        assert!(!harness.read_bit(), "parasite power should pull low");
    }
}
