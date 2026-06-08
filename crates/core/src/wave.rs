use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use tracing::warn;

pub(crate) const TRACK_EVENT_CPU: &str = "event.cpu";
pub(crate) const TRACK_EVENT_I2C: &str = "event.i2c";
pub(crate) const TRACK_EVENT_ONEWIRE: &str = "event.onewire";
pub(crate) const TRACK_EVENT_UART1: &str = "event.uart1";
pub(crate) const TRACK_EVENT_UART2: &str = "event.uart2";
pub(crate) const TRACK_EVENT_ADC_DAC: &str = "event.adc_dac";
pub(crate) const TRACK_EVENT_DS1302: &str = "event.ds1302";
pub(crate) const TRACK_EVENT_SEG_CHANGE: &str = "event.seg.change";

pub(crate) fn seg_digit_change_track_id(digit: usize) -> &'static str {
    match digit {
        1 => "event.seg.d1.change",
        2 => "event.seg.d2.change",
        3 => "event.seg.d3.change",
        4 => "event.seg.d4.change",
        5 => "event.seg.d5.change",
        6 => "event.seg.d6.change",
        7 => "event.seg.d7.change",
        8 => "event.seg.d8.change",
        _ => "event.seg.change",
    }
}

const WAVE_VIEWER_TEMPLATE: &str = include_str!("../assets/wave_viewer.html");
const MSGPACK_BROWSER_LIB: &str = include_str!("../assets/msgpack.min.js");
const WAVE_MSGPACK_VERSION: u8 = 1;

const KEY_NAMES: [&str; 16] = [
    "S4", "S5", "S6", "S7", "S8", "S9", "S10", "S11", "S12", "S13", "S14", "S15", "S16", "S17",
    "S18", "S19",
];
const LED_NAMES: [&str; 8] = ["L1", "L2", "L3", "L4", "L5", "L6", "L7", "L8"];

#[derive(Debug, Clone, Default)]
pub struct WaveCaptureOptions {
    pub html_path: Option<PathBuf>,
    pub json_path: Option<PathBuf>,
    pub msgpack_path: Option<PathBuf>,
    pub start_ns: u64,
    pub end_ns: Option<u64>,
}

impl WaveCaptureOptions {
    pub(crate) fn enabled(&self) -> bool {
        self.html_path.is_some() || self.json_path.is_some() || self.msgpack_path.is_some()
    }

    pub(crate) fn window(&self) -> WaveCaptureWindow {
        WaveCaptureWindow {
            enabled: self.enabled(),
            start_ns: self.start_ns,
            end_ns: self.end_bound(),
        }
    }

    fn end_bound(&self) -> u64 {
        self.end_ns.unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WaveCaptureWindow {
    enabled: bool,
    start_ns: u64,
    end_ns: u64,
}

impl WaveCaptureWindow {
    pub(crate) fn from_enabled(enabled: bool) -> Self {
        Self {
            enabled,
            start_ns: 0,
            end_ns: u64::MAX,
        }
    }

    #[cfg(test)]
    pub(crate) fn bounded(start_ns: u64, end_ns: Option<u64>) -> Self {
        Self {
            enabled: true,
            start_ns,
            end_ns: end_ns.unwrap_or(u64::MAX),
        }
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }

    pub(crate) fn includes(self, time_ns: u64) -> bool {
        self.enabled && self.start_ns <= time_ns && time_ns <= self.end_ns
    }

    pub(crate) fn overlaps(self, start_ns: u64, end_ns: u64) -> bool {
        self.enabled && start_ns <= self.end_ns && self.start_ns <= end_ns
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WaveEventNote {
    pub(crate) time_ns: u64,
    pub(crate) track_id: &'static str,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
}

impl WaveEventNote {
    pub(crate) fn new(time_ns: u64, track_id: &'static str, label: impl Into<String>) -> Self {
        Self {
            time_ns,
            track_id,
            label: label.into(),
            detail: None,
        }
    }

    pub(crate) fn with_detail(
        time_ns: u64,
        track_id: &'static str,
        label: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            time_ns,
            track_id,
            label: label.into(),
            detail: Some(detail.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WaveMarkerNote {
    pub(crate) time_ns: u64,
    pub(crate) label: Option<String>,
}

impl WaveMarkerNote {
    pub(crate) fn anonymous(time_ns: u64) -> Self {
        Self {
            time_ns,
            label: None,
        }
    }

    pub(crate) fn named(time_ns: u64, label: impl Into<String>) -> Self {
        Self {
            time_ns,
            label: Some(label.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WaveSnapshot {
    pub(crate) time_ns: u64,
    pub(crate) port_latch: [u8; 6],
    pub(crate) port_input: [u8; 6],
    pub(crate) board_latches_effective: [u8; 4],
    pub(crate) board_latches_port: [u8; 4],
    pub(crate) board_latches_xdata: [u8; 4],
    pub(crate) signal_sig_out: bool,
    pub(crate) jumper_net_sig_to_sig_out: bool,
    pub(crate) i2c_master_scl: bool,
    pub(crate) i2c_master_sda: bool,
    pub(crate) i2c_bus_scl: bool,
    pub(crate) i2c_bus_sda: bool,
    pub(crate) i2c_slave_scl_low: bool,
    pub(crate) i2c_slave_sda_low: bool,
    pub(crate) onewire_master_high: bool,
    pub(crate) onewire_bus_high: bool,
    pub(crate) onewire_device_low: bool,
    pub(crate) ds1302_ce: bool,
    pub(crate) ds1302_clk: bool,
    pub(crate) ds1302_io: bool,
    pub(crate) uart1_tx_high: bool,
    pub(crate) uart1_rx_high: bool,
    pub(crate) uart1_ti: bool,
    pub(crate) uart1_ri: bool,
    pub(crate) uart2_tx_high: bool,
    pub(crate) uart2_rx_high: bool,
    pub(crate) uart2_ti: bool,
    pub(crate) uart2_ri: bool,
    pub(crate) key_states: [bool; 16],
    pub(crate) led_states: [bool; 8],
    pub(crate) relay_on: bool,
    pub(crate) motor_on: bool,
    pub(crate) buzzer_on: bool,
    pub(crate) seg_text: String,
    pub(crate) seg_chars: [char; 8],
    pub(crate) seg_raw: [u8; 8],
    pub(crate) analog_rd1_v: f32,
    pub(crate) analog_rb2_v: f32,
    pub(crate) adc_code: u8,
    pub(crate) adc_channel: u8,
    pub(crate) adc_channel_voltage_v: f32,
    pub(crate) dac_code: u8,
    pub(crate) dac_voltage_v: f32,
    pub(crate) ne555_level: bool,
    pub(crate) ne555_frequency_hz: f32,
}

#[derive(Debug, Clone, Copy)]
enum SignalKind {
    Digital,
    Integer,
    Analog,
    Text,
    Event,
}

impl SignalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Digital => "digital",
            Self::Integer => "integer",
            Self::Analog => "analog",
            Self::Text => "text",
            Self::Event => "event",
        }
    }
}

#[derive(Debug, Clone)]
struct SignalDef {
    id: String,
    label: String,
    category: String,
    group: String,
    aliases: Vec<String>,
    kind: SignalKind,
    format: &'static str,
    unit: Option<&'static str>,
    default_visible: bool,
}

#[derive(Debug, Clone)]
enum SignalValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Clone)]
struct SamplePoint {
    time_ns: u64,
    value: SignalValue,
}

#[derive(Debug, Clone)]
struct EventRecord {
    track_id: &'static str,
    time_ns: u64,
    label: String,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
struct MarkerRecord {
    time_ns: u64,
    label: Option<String>,
}

#[derive(Debug, Clone)]
struct SignalRecord {
    def: SignalDef,
    points: Vec<SamplePoint>,
}

#[derive(Debug, Serialize)]
struct WaveBinaryPayload<'a> {
    version: u8,
    start_ns: u64,
    end_ns: u64,
    signals: Vec<WaveBinarySignal<'a>>,
    samples: Vec<Vec<WaveBinarySample<'a>>>,
    events: Vec<WaveBinaryEvent<'a>>,
    markers: Vec<WaveBinaryMarker<'a>>,
}

#[derive(Debug, Serialize)]
struct WaveBinarySignal<'a>(
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a [String],
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
);

#[derive(Debug, Serialize)]
struct WaveBinarySample<'a>(u64, WaveBinaryValue<'a>);

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum WaveBinaryValue<'a> {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(&'a str),
}

#[derive(Debug, Serialize)]
struct WaveBinaryEvent<'a>(usize, u64, &'a str, Option<&'a str>);

#[derive(Debug, Serialize)]
struct WaveBinaryMarker<'a>(u64, Option<&'a str>);

impl<'a> From<&'a SignalValue> for WaveBinaryValue<'a> {
    fn from(value: &'a SignalValue) -> Self {
        match value {
            SignalValue::Bool(value) => Self::Bool(*value),
            SignalValue::Integer(value) => Self::Integer(*value),
            SignalValue::Float(value) => Self::Float(*value),
            SignalValue::Text(value) => Self::Text(value.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct WaveSignalSlots {
    pin_bytes: [usize; 6],
    latch_bytes: [usize; 6],
    pin_bits: [[usize; 8]; 6],
    latch_bits: [[usize; 8]; 6],
    board_effective: [usize; 4],
    board_port: [usize; 4],
    board_xdata: [usize; 4],
    signal_sig_out: usize,
    jumper_net_sig_to_sig_out: usize,
    i2c_master_scl: usize,
    i2c_master_sda: usize,
    i2c_bus_scl: usize,
    i2c_bus_sda: usize,
    i2c_slave_scl_low: usize,
    i2c_slave_sda_low: usize,
    onewire_master_high: usize,
    onewire_bus_high: usize,
    onewire_device_low: usize,
    ds1302_ce: usize,
    ds1302_clk: usize,
    ds1302_io: usize,
    uart1_tx: usize,
    uart1_rx: usize,
    uart1_ti: usize,
    uart1_ri: usize,
    uart2_tx: usize,
    uart2_rx: usize,
    uart2_ti: usize,
    uart2_ri: usize,
    key_states: [usize; 16],
    led_states: [usize; 8],
    relay_on: usize,
    motor_on: usize,
    buzzer_on: usize,
    seg_text: usize,
    seg_digit_text: [usize; 8],
    seg_digit_raw: [usize; 8],
    analog_rd1_v: usize,
    analog_rb2_v: usize,
    adc_code: usize,
    adc_channel: usize,
    adc_channel_voltage_v: usize,
    dac_code: usize,
    dac_voltage_v: usize,
    ne555_level: usize,
    ne555_frequency_hz: usize,
}

fn push_alias(aliases: &mut Vec<String>, alias: impl Into<String>) {
    let alias = alias.into();
    let alias = alias.trim();
    if alias.is_empty() {
        return;
    }
    if aliases.iter().any(|existing| existing == alias) {
        return;
    }
    aliases.push(alias.to_owned());
}

fn normalize_alias_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_space && !normalized.is_empty() {
                normalized.push(' ');
            }
            normalized.push(ch.to_ascii_lowercase());
            pending_space = false;
        } else if !normalized.is_empty() {
            pending_space = true;
        }
    }
    normalized
}

fn add_alias_forms(aliases: &mut Vec<String>, text: &str) {
    let lowercase = text.to_ascii_lowercase();
    push_alias(aliases, lowercase.as_str());

    let normalized = normalize_alias_text(&lowercase);
    push_alias(aliases, normalized.as_str());
    push_alias(aliases, normalized.replace(' ', ""));

    if lowercase.contains("i2c") {
        push_alias(aliases, lowercase.replace("i2c", "iic"));
    }
    if normalized.contains("i2c") {
        push_alias(aliases, normalized.replace("i2c", "iic"));
        push_alias(aliases, normalized.replace("i2c", "i 2 c"));
    }
    if lowercase.contains("iic") {
        push_alias(aliases, lowercase.replace("iic", "i2c"));
    }
    if normalized.contains("iic") {
        push_alias(aliases, normalized.replace("iic", "i2c"));
    }

    if lowercase.contains("onewire") {
        push_alias(aliases, lowercase.replace("onewire", "1wire"));
        push_alias(aliases, lowercase.replace("onewire", "1-wire"));
        push_alias(aliases, lowercase.replace("onewire", "one wire"));
    }
    if normalized.contains("onewire") {
        push_alias(aliases, normalized.replace("onewire", "1wire"));
        push_alias(aliases, normalized.replace("onewire", "1 wire"));
    }

    if normalized.contains("uart1") {
        push_alias(aliases, normalized.replace("uart1", "uart 1"));
        push_alias(aliases, normalized.replace("uart1", "serial1"));
        push_alias(aliases, normalized.replace("uart1", "serial 1"));
    }
    if normalized.contains("uart2") {
        push_alias(aliases, normalized.replace("uart2", "uart 2"));
        push_alias(aliases, normalized.replace("uart2", "serial2"));
        push_alias(aliases, normalized.replace("uart2", "serial 2"));
    }
    if normalized.contains("uart") {
        push_alias(aliases, normalized.replace("uart", "serial"));
    }
    if normalized.contains("clk") {
        push_alias(aliases, normalized.replace("clk", "clock"));
    }
    if normalized.contains("scl") {
        push_alias(aliases, normalized.replace("scl", "clock"));
    }
    if normalized.contains("sda") {
        push_alias(aliases, normalized.replace("sda", "data"));
    }
    if normalized.contains("seg") {
        push_alias(aliases, normalized.replace("seg", "7seg"));
        push_alias(aliases, normalized.replace("seg", "sevenseg"));
    }
    if normalized.contains("ds1302") {
        push_alias(aliases, normalized.replace("ds1302", "rtc"));
        push_alias(aliases, normalized.replace("ds1302", "clock chip"));
    }
    if normalized.contains("adc") {
        push_alias(aliases, normalized.replace("adc", "ad"));
    }
    if normalized.contains("dac") {
        push_alias(aliases, normalized.replace("dac", "da"));
    }
    if lowercase.contains("ain") {
        push_alias(aliases, lowercase.replace("ain", "adc"));
    }
    if normalized.contains("ain") {
        push_alias(aliases, normalized.replace("ain", "adc"));
        push_alias(aliases, normalized.replace("ain", "analog"));
    }
    if normalized.contains("ne555") {
        push_alias(aliases, normalized.replace("ne555", "555"));
        push_alias(aliases, normalized.replace("ne555", "timer555"));
        push_alias(aliases, normalized.replace("ne555", "timer 555"));
    }
}

fn signal_aliases(id: &str, label: &str, category: &str, group: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    for text in [id, label, category, group] {
        add_alias_forms(&mut aliases, text);
    }
    add_alias_forms(&mut aliases, &format!("{category} {group}"));
    add_alias_forms(&mut aliases, &format!("{group} {label}"));
    add_alias_forms(&mut aliases, &format!("{id} {label}"));
    aliases
}

pub(crate) struct WaveRecorder {
    options: WaveCaptureOptions,
    window: WaveCaptureWindow,
    signal_lookup: HashMap<String, usize>,
    signals: Vec<SignalRecord>,
    signal_slots: WaveSignalSlots,
    events: Vec<EventRecord>,
    markers: Vec<MarkerRecord>,
    last_snapshot: Option<WaveSnapshot>,
    observed_start_ns: Option<u64>,
    observed_end_ns: Option<u64>,
}

impl WaveRecorder {
    pub(crate) fn new(options: WaveCaptureOptions) -> Self {
        let window = options.window();
        let mut recorder = Self {
            options,
            window,
            signal_lookup: HashMap::new(),
            signals: Vec::new(),
            signal_slots: WaveSignalSlots::default(),
            events: Vec::new(),
            markers: Vec::new(),
            last_snapshot: None,
            observed_start_ns: None,
            observed_end_ns: None,
        };
        if recorder.enabled() {
            recorder.signal_slots = recorder.register_defaults();
        }
        recorder
    }

    pub(crate) fn enabled(&self) -> bool {
        self.window.enabled()
    }

    pub(crate) fn captures_time(&self, time_ns: u64) -> bool {
        self.window.includes(time_ns)
    }

    #[cfg(test)]
    pub(crate) fn new_with_window(window: WaveCaptureWindow) -> Self {
        let mut recorder = Self {
            options: WaveCaptureOptions::default(),
            window,
            signal_lookup: HashMap::new(),
            signals: Vec::new(),
            signal_slots: WaveSignalSlots::default(),
            events: Vec::new(),
            markers: Vec::new(),
            last_snapshot: None,
            observed_start_ns: None,
            observed_end_ns: None,
        };
        recorder.signal_slots = recorder.register_defaults();
        recorder
    }

    pub(crate) fn observe_snapshot(&mut self, snapshot: WaveSnapshot) {
        if !self.window.includes(snapshot.time_ns) {
            return;
        }
        let slots = self.signal_slots;
        let prev_snapshot = self.last_snapshot.take();
        let prev = prev_snapshot.as_ref();

        for port in 0..6 {
            let input_byte = snapshot.port_input[port];
            let input_changed = prev.is_none_or(|last| last.port_input[port] != input_byte);
            if input_changed {
                self.record_integer_index(
                    slots.pin_bytes[port],
                    snapshot.time_ns,
                    i64::from(input_byte),
                );
                let input_diff = prev.map_or(u8::MAX, |last| last.port_input[port] ^ input_byte);
                for bit in 0..8 {
                    if prev.is_none() || input_diff & (1 << bit) != 0 {
                        let pin_high = input_byte & (1 << bit) != 0;
                        self.record_bool_index(
                            slots.pin_bits[port][bit],
                            snapshot.time_ns,
                            pin_high,
                        );
                    }
                }
            }

            let latch_byte = snapshot.port_latch[port];
            let latch_changed = prev.is_none_or(|last| last.port_latch[port] != latch_byte);
            if latch_changed {
                self.record_integer_index(
                    slots.latch_bytes[port],
                    snapshot.time_ns,
                    i64::from(latch_byte),
                );
                let latch_diff = prev.map_or(u8::MAX, |last| last.port_latch[port] ^ latch_byte);
                for bit in 0..8 {
                    if prev.is_none() || latch_diff & (1 << bit) != 0 {
                        let latch_high = latch_byte & (1 << bit) != 0;
                        self.record_bool_index(
                            slots.latch_bits[port][bit],
                            snapshot.time_ns,
                            latch_high,
                        );
                    }
                }
            }
        }

        for slot in 0..4 {
            let effective_value = snapshot.board_latches_effective[slot];
            if prev.is_none_or(|last| last.board_latches_effective[slot] != effective_value) {
                self.record_integer_index(
                    slots.board_effective[slot],
                    snapshot.time_ns,
                    i64::from(effective_value),
                );
            }
            let port_value = snapshot.board_latches_port[slot];
            if prev.is_none_or(|last| last.board_latches_port[slot] != port_value) {
                self.record_integer_index(
                    slots.board_port[slot],
                    snapshot.time_ns,
                    i64::from(port_value),
                );
            }
            let xdata_value = snapshot.board_latches_xdata[slot];
            if prev.is_none_or(|last| last.board_latches_xdata[slot] != xdata_value) {
                self.record_integer_index(
                    slots.board_xdata[slot],
                    snapshot.time_ns,
                    i64::from(xdata_value),
                );
            }
        }

        if prev.is_none_or(|last| last.signal_sig_out != snapshot.signal_sig_out) {
            self.record_bool_index(
                slots.signal_sig_out,
                snapshot.time_ns,
                snapshot.signal_sig_out,
            );
        }
        if prev
            .is_none_or(|last| last.jumper_net_sig_to_sig_out != snapshot.jumper_net_sig_to_sig_out)
        {
            self.record_bool_index(
                slots.jumper_net_sig_to_sig_out,
                snapshot.time_ns,
                snapshot.jumper_net_sig_to_sig_out,
            );
        }

        if prev.is_none_or(|last| last.i2c_master_scl != snapshot.i2c_master_scl) {
            self.record_bool_index(
                slots.i2c_master_scl,
                snapshot.time_ns,
                snapshot.i2c_master_scl,
            );
        }
        if prev.is_none_or(|last| last.i2c_master_sda != snapshot.i2c_master_sda) {
            self.record_bool_index(
                slots.i2c_master_sda,
                snapshot.time_ns,
                snapshot.i2c_master_sda,
            );
        }
        if prev.is_none_or(|last| last.i2c_bus_scl != snapshot.i2c_bus_scl) {
            self.record_bool_index(slots.i2c_bus_scl, snapshot.time_ns, snapshot.i2c_bus_scl);
        }
        if prev.is_none_or(|last| last.i2c_bus_sda != snapshot.i2c_bus_sda) {
            self.record_bool_index(slots.i2c_bus_sda, snapshot.time_ns, snapshot.i2c_bus_sda);
        }
        if prev.is_none_or(|last| last.i2c_slave_scl_low != snapshot.i2c_slave_scl_low) {
            self.record_bool_index(
                slots.i2c_slave_scl_low,
                snapshot.time_ns,
                snapshot.i2c_slave_scl_low,
            );
        }
        if prev.is_none_or(|last| last.i2c_slave_sda_low != snapshot.i2c_slave_sda_low) {
            self.record_bool_index(
                slots.i2c_slave_sda_low,
                snapshot.time_ns,
                snapshot.i2c_slave_sda_low,
            );
        }

        if prev.is_none_or(|last| last.onewire_master_high != snapshot.onewire_master_high) {
            self.record_bool_index(
                slots.onewire_master_high,
                snapshot.time_ns,
                snapshot.onewire_master_high,
            );
        }
        if prev.is_none_or(|last| last.onewire_bus_high != snapshot.onewire_bus_high) {
            self.record_bool_index(
                slots.onewire_bus_high,
                snapshot.time_ns,
                snapshot.onewire_bus_high,
            );
        }
        if prev.is_none_or(|last| last.onewire_device_low != snapshot.onewire_device_low) {
            self.record_bool_index(
                slots.onewire_device_low,
                snapshot.time_ns,
                snapshot.onewire_device_low,
            );
        }

        if prev.is_none_or(|last| last.ds1302_ce != snapshot.ds1302_ce) {
            self.record_bool_index(slots.ds1302_ce, snapshot.time_ns, snapshot.ds1302_ce);
        }
        if prev.is_none_or(|last| last.ds1302_clk != snapshot.ds1302_clk) {
            self.record_bool_index(slots.ds1302_clk, snapshot.time_ns, snapshot.ds1302_clk);
        }
        if prev.is_none_or(|last| last.ds1302_io != snapshot.ds1302_io) {
            self.record_bool_index(slots.ds1302_io, snapshot.time_ns, snapshot.ds1302_io);
        }

        if prev.is_none_or(|last| last.uart1_tx_high != snapshot.uart1_tx_high) {
            self.record_bool_index(slots.uart1_tx, snapshot.time_ns, snapshot.uart1_tx_high);
        }
        if prev.is_none_or(|last| last.uart1_rx_high != snapshot.uart1_rx_high) {
            self.record_bool_index(slots.uart1_rx, snapshot.time_ns, snapshot.uart1_rx_high);
        }
        if prev.is_none_or(|last| last.uart1_ti != snapshot.uart1_ti) {
            self.record_bool_index(slots.uart1_ti, snapshot.time_ns, snapshot.uart1_ti);
        }
        if prev.is_none_or(|last| last.uart1_ri != snapshot.uart1_ri) {
            self.record_bool_index(slots.uart1_ri, snapshot.time_ns, snapshot.uart1_ri);
        }
        if prev.is_none_or(|last| last.uart2_tx_high != snapshot.uart2_tx_high) {
            self.record_bool_index(slots.uart2_tx, snapshot.time_ns, snapshot.uart2_tx_high);
        }
        if prev.is_none_or(|last| last.uart2_rx_high != snapshot.uart2_rx_high) {
            self.record_bool_index(slots.uart2_rx, snapshot.time_ns, snapshot.uart2_rx_high);
        }
        if prev.is_none_or(|last| last.uart2_ti != snapshot.uart2_ti) {
            self.record_bool_index(slots.uart2_ti, snapshot.time_ns, snapshot.uart2_ti);
        }
        if prev.is_none_or(|last| last.uart2_ri != snapshot.uart2_ri) {
            self.record_bool_index(slots.uart2_ri, snapshot.time_ns, snapshot.uart2_ri);
        }

        for index in 0..KEY_NAMES.len() {
            if prev.is_none_or(|last| last.key_states[index] != snapshot.key_states[index]) {
                self.record_bool_index(
                    slots.key_states[index],
                    snapshot.time_ns,
                    snapshot.key_states[index],
                );
            }
        }

        for index in 0..LED_NAMES.len() {
            if prev.is_none_or(|last| last.led_states[index] != snapshot.led_states[index]) {
                self.record_bool_index(
                    slots.led_states[index],
                    snapshot.time_ns,
                    snapshot.led_states[index],
                );
            }
        }

        if prev.is_none_or(|last| last.relay_on != snapshot.relay_on) {
            self.record_bool_index(slots.relay_on, snapshot.time_ns, snapshot.relay_on);
        }
        if prev.is_none_or(|last| last.motor_on != snapshot.motor_on) {
            self.record_bool_index(slots.motor_on, snapshot.time_ns, snapshot.motor_on);
        }
        if prev.is_none_or(|last| last.buzzer_on != snapshot.buzzer_on) {
            self.record_bool_index(slots.buzzer_on, snapshot.time_ns, snapshot.buzzer_on);
        }

        for digit in 0..8 {
            if prev.is_none_or(|last| last.seg_chars[digit] != snapshot.seg_chars[digit]) {
                self.record_char_text_index(
                    slots.seg_digit_text[digit],
                    snapshot.time_ns,
                    snapshot.seg_chars[digit],
                );
            }
            if prev.is_none_or(|last| last.seg_raw[digit] != snapshot.seg_raw[digit]) {
                self.record_integer_index(
                    slots.seg_digit_raw[digit],
                    snapshot.time_ns,
                    i64::from(snapshot.seg_raw[digit]),
                );
            }
        }

        if prev.is_none_or(|last| last.seg_text != snapshot.seg_text) {
            self.record_text_index(slots.seg_text, snapshot.time_ns, snapshot.seg_text.as_str());
        }

        if prev.is_none_or(|last| last.analog_rd1_v.to_bits() != snapshot.analog_rd1_v.to_bits()) {
            self.record_float_index(
                slots.analog_rd1_v,
                snapshot.time_ns,
                f64::from(snapshot.analog_rd1_v),
            );
        }
        if prev.is_none_or(|last| last.analog_rb2_v.to_bits() != snapshot.analog_rb2_v.to_bits()) {
            self.record_float_index(
                slots.analog_rb2_v,
                snapshot.time_ns,
                f64::from(snapshot.analog_rb2_v),
            );
        }
        if prev.is_none_or(|last| last.adc_code != snapshot.adc_code) {
            self.record_integer_index(
                slots.adc_code,
                snapshot.time_ns,
                i64::from(snapshot.adc_code),
            );
        }
        if prev.is_none_or(|last| last.adc_channel != snapshot.adc_channel) {
            self.record_integer_index(
                slots.adc_channel,
                snapshot.time_ns,
                i64::from(snapshot.adc_channel),
            );
        }
        if prev.is_none_or(|last| {
            last.adc_channel_voltage_v.to_bits() != snapshot.adc_channel_voltage_v.to_bits()
        }) {
            self.record_float_index(
                slots.adc_channel_voltage_v,
                snapshot.time_ns,
                f64::from(snapshot.adc_channel_voltage_v),
            );
        }
        if prev.is_none_or(|last| last.dac_code != snapshot.dac_code) {
            self.record_integer_index(
                slots.dac_code,
                snapshot.time_ns,
                i64::from(snapshot.dac_code),
            );
        }
        if prev.is_none_or(|last| last.dac_voltage_v.to_bits() != snapshot.dac_voltage_v.to_bits())
        {
            self.record_float_index(
                slots.dac_voltage_v,
                snapshot.time_ns,
                f64::from(snapshot.dac_voltage_v),
            );
        }
        if prev.is_none_or(|last| last.ne555_level != snapshot.ne555_level) {
            self.record_bool_index(slots.ne555_level, snapshot.time_ns, snapshot.ne555_level);
        }
        if prev.is_none_or(|last| {
            last.ne555_frequency_hz.to_bits() != snapshot.ne555_frequency_hz.to_bits()
        }) {
            self.record_float_index(
                slots.ne555_frequency_hz,
                snapshot.time_ns,
                f64::from(snapshot.ne555_frequency_hz),
            );
        }

        self.last_snapshot = Some(snapshot);
    }

    pub(crate) fn record_event_note(&mut self, note: WaveEventNote) {
        if !self.window.includes(note.time_ns) {
            return;
        }
        if !self.signal_lookup.contains_key(note.track_id) {
            return;
        }
        self.mark_observed_time(note.time_ns);
        self.events.push(EventRecord {
            track_id: note.track_id,
            time_ns: note.time_ns,
            label: note.label,
            detail: note.detail,
        });
    }

    pub(crate) fn record_marker_note(&mut self, note: WaveMarkerNote) {
        if !self.window.includes(note.time_ns) {
            return;
        }
        self.mark_observed_time(note.time_ns);
        self.markers.push(MarkerRecord {
            time_ns: note.time_ns,
            label: note.label.and_then(|label| {
                let trimmed = label.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_owned())
            }),
        });
    }

    #[cfg(test)]
    pub(crate) fn marker_records(&self) -> Vec<(u64, Option<String>)> {
        self.markers
            .iter()
            .map(|marker| (marker.time_ns, marker.label.clone()))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn event_records(&self) -> Vec<(&'static str, u64, String, Option<String>)> {
        self.events
            .iter()
            .map(|event| {
                (
                    event.track_id,
                    event.time_ns,
                    event.label.clone(),
                    event.detail.clone(),
                )
            })
            .collect()
    }

    fn register_defaults(&mut self) -> WaveSignalSlots {
        let mut slots = WaveSignalSlots::default();
        for port in 0..6 {
            slots.pin_bytes[port] = self.register_signal(
                format!("pin.p{port}"),
                format!("P{port} pin byte"),
                "pins",
                format!("P{port} pins"),
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            slots.latch_bytes[port] = self.register_signal(
                format!("latch.p{port}"),
                format!("P{port} latch byte"),
                "port_latches",
                format!("P{port} latches"),
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            for bit in 0..8 {
                slots.pin_bits[port][bit] = self.register_signal(
                    format!("pin.p{port}.{bit}"),
                    format!("P{port}.{bit} pin"),
                    "pins",
                    format!("P{port} pins"),
                    SignalKind::Digital,
                    "bool",
                    None,
                    false,
                );
                slots.latch_bits[port][bit] = self.register_signal(
                    format!("latch.p{port}.{bit}"),
                    format!("P{port}.{bit} latch"),
                    "port_latches",
                    format!("P{port} latches"),
                    SignalKind::Digital,
                    "bool",
                    None,
                    false,
                );
            }
        }

        for slot in 0..4 {
            slots.board_effective[slot] = self.register_signal(
                format!("board.effective.{slot}"),
                format!("effective latch {slot}"),
                "board_latches",
                "effective",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            slots.board_port[slot] = self.register_signal(
                format!("board.port.{slot}"),
                format!("port latch {slot}"),
                "board_latches",
                "port",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            slots.board_xdata[slot] = self.register_signal(
                format!("board.xdata.{slot}"),
                format!("xdata latch {slot}"),
                "board_latches",
                "xdata",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
        }

        for (id, label, visible) in [
            ("signal.sig_out", "SIG_OUT", true),
            ("jumper.net_sig_sig_out", "NET_SIG<->SIG_OUT", true),
        ] {
            let index = self.register_signal(
                id,
                label,
                "board_signals",
                "jumpers",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "signal.sig_out" => slots.signal_sig_out = index,
                "jumper.net_sig_sig_out" => slots.jumper_net_sig_to_sig_out = index,
                _ => {}
            }
        }

        self.register_signal(
            TRACK_EVENT_I2C,
            "I2C events",
            "protocol",
            "i2c",
            SignalKind::Event,
            "event",
            None,
            true,
        );
        for (id, label, visible) in [
            ("i2c.master_scl", "master SCL", false),
            ("i2c.master_sda", "master SDA", false),
            ("i2c.bus_scl", "bus SCL", true),
            ("i2c.bus_sda", "bus SDA", true),
            ("i2c.slave_scl_low", "slave SCL low", false),
            ("i2c.slave_sda_low", "slave SDA low", false),
        ] {
            let index = self.register_signal(
                id,
                label,
                "protocol",
                "i2c",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "i2c.master_scl" => slots.i2c_master_scl = index,
                "i2c.master_sda" => slots.i2c_master_sda = index,
                "i2c.bus_scl" => slots.i2c_bus_scl = index,
                "i2c.bus_sda" => slots.i2c_bus_sda = index,
                "i2c.slave_scl_low" => slots.i2c_slave_scl_low = index,
                "i2c.slave_sda_low" => slots.i2c_slave_sda_low = index,
                _ => {}
            }
        }

        self.register_signal(
            TRACK_EVENT_ONEWIRE,
            "1-Wire events",
            "protocol",
            "onewire",
            SignalKind::Event,
            "event",
            None,
            true,
        );
        for (id, label, visible) in [
            ("onewire.master_high", "master high", false),
            ("onewire.bus_high", "bus high", true),
            ("onewire.device_low", "device low", false),
        ] {
            let index = self.register_signal(
                id,
                label,
                "protocol",
                "onewire",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "onewire.master_high" => slots.onewire_master_high = index,
                "onewire.bus_high" => slots.onewire_bus_high = index,
                "onewire.device_low" => slots.onewire_device_low = index,
                _ => {}
            }
        }

        self.register_signal(
            TRACK_EVENT_UART1,
            "UART1 events",
            "protocol",
            "uart1",
            SignalKind::Event,
            "event",
            None,
            true,
        );
        self.register_signal(
            TRACK_EVENT_UART2,
            "UART2 events",
            "protocol",
            "uart2",
            SignalKind::Event,
            "event",
            None,
            false,
        );
        for (id, label, group, visible) in [
            ("uart1.tx", "TX", "uart1", true),
            ("uart1.rx", "RX", "uart1", true),
            ("uart1.ti", "TI", "uart1", true),
            ("uart1.ri", "RI", "uart1", true),
            ("uart2.tx", "TX", "uart2", false),
            ("uart2.rx", "RX", "uart2", false),
            ("uart2.ti", "TI", "uart2", false),
            ("uart2.ri", "RI", "uart2", false),
        ] {
            let index = self.register_signal(
                id,
                label,
                "protocol",
                group,
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "uart1.tx" => slots.uart1_tx = index,
                "uart1.rx" => slots.uart1_rx = index,
                "uart1.ti" => slots.uart1_ti = index,
                "uart1.ri" => slots.uart1_ri = index,
                "uart2.tx" => slots.uart2_tx = index,
                "uart2.rx" => slots.uart2_rx = index,
                "uart2.ti" => slots.uart2_ti = index,
                "uart2.ri" => slots.uart2_ri = index,
                _ => {}
            }
        }

        self.register_signal(
            TRACK_EVENT_DS1302,
            "DS1302 events",
            "protocol",
            "ds1302",
            SignalKind::Event,
            "event",
            None,
            false,
        );
        for (id, label, visible) in [
            ("ds1302.ce", "CE", false),
            ("ds1302.clk", "CLK", false),
            ("ds1302.io", "IO", false),
        ] {
            let index = self.register_signal(
                id,
                label,
                "protocol",
                "ds1302",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "ds1302.ce" => slots.ds1302_ce = index,
                "ds1302.clk" => slots.ds1302_clk = index,
                "ds1302.io" => slots.ds1302_io = index,
                _ => {}
            }
        }

        self.register_signal(
            TRACK_EVENT_CPU,
            "CPU events",
            "cpu",
            "interrupts",
            SignalKind::Event,
            "event",
            None,
            true,
        );

        for (index, name) in KEY_NAMES.iter().enumerate() {
            slots.key_states[index] = self.register_signal(
                format!("key.{}", name.to_ascii_lowercase()),
                (*name).to_string(),
                "keys",
                "matrix_keys",
                SignalKind::Digital,
                "bool",
                None,
                false,
            );
        }

        for (index, name) in LED_NAMES.iter().enumerate() {
            slots.led_states[index] = self.register_signal(
                format!("led.{}", name.to_ascii_lowercase()),
                *name,
                "outputs",
                "leds",
                SignalKind::Digital,
                "bool",
                None,
                true,
            );
        }
        for (id, label, visible) in [
            ("output.relay", "relay", false),
            ("output.motor", "motor", false),
            ("output.buzzer", "buzzer", false),
        ] {
            let index = self.register_signal(
                id,
                label,
                "outputs",
                "board_outputs",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
            match id {
                "output.relay" => slots.relay_on = index,
                "output.motor" => slots.motor_on = index,
                "output.buzzer" => slots.buzzer_on = index,
                _ => {}
            }
        }

        slots.seg_text = self.register_signal(
            "seg.text",
            "display text",
            "display",
            "seg",
            SignalKind::Text,
            "text",
            None,
            true,
        );
        for digit in 1..=8 {
            slots.seg_digit_text[digit - 1] = self.register_signal(
                format!("seg.d{digit}.text"),
                format!("D{digit} char"),
                "display",
                "seg_digits",
                SignalKind::Text,
                "text",
                None,
                false,
            );
            slots.seg_digit_raw[digit - 1] = self.register_signal(
                format!("seg.d{digit}.raw"),
                format!("D{digit} raw"),
                "display",
                "seg_raw",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
        }
        self.register_signal(
            TRACK_EVENT_SEG_CHANGE,
            "display change events",
            "display",
            "seg_events",
            SignalKind::Event,
            "event",
            None,
            true,
        );
        for digit in 1..=8 {
            self.register_signal(
                seg_digit_change_track_id(digit),
                format!("D{digit} change events"),
                "display",
                "seg_digit_events",
                SignalKind::Event,
                "event",
                None,
                false,
            );
        }

        self.register_signal(
            TRACK_EVENT_ADC_DAC,
            "ADC/DAC events",
            "analog",
            "pcf8591",
            SignalKind::Event,
            "event",
            None,
            true,
        );
        for (id, label, format, unit, visible, kind) in [
            (
                "analog.rd1_v",
                "RD1/AIN1",
                "float",
                Some("V"),
                true,
                SignalKind::Analog,
            ),
            (
                "analog.rb2_v",
                "RB2/AIN3",
                "float",
                Some("V"),
                true,
                SignalKind::Analog,
            ),
            (
                "pcf8591.adc_code",
                "ADC code",
                "dec",
                None,
                true,
                SignalKind::Integer,
            ),
            (
                "pcf8591.adc_channel",
                "ADC channel",
                "dec",
                None,
                true,
                SignalKind::Integer,
            ),
            (
                "pcf8591.adc_channel_v",
                "ADC source V",
                "float",
                Some("V"),
                true,
                SignalKind::Analog,
            ),
            (
                "pcf8591.dac_code",
                "DAC code",
                "dec",
                None,
                true,
                SignalKind::Integer,
            ),
            (
                "pcf8591.dac_v",
                "DAC V",
                "float",
                Some("V"),
                true,
                SignalKind::Analog,
            ),
            (
                "ne555.frequency_hz",
                "NE555 Hz",
                "float",
                Some("Hz"),
                true,
                SignalKind::Analog,
            ),
            (
                "ne555.level",
                "NE555 level",
                "bool",
                None,
                true,
                SignalKind::Digital,
            ),
        ] {
            let index = self.register_signal(
                id,
                label,
                "analog",
                "pcf8591_ne555",
                kind,
                format,
                unit,
                visible,
            );
            match id {
                "analog.rd1_v" => slots.analog_rd1_v = index,
                "analog.rb2_v" => slots.analog_rb2_v = index,
                "pcf8591.adc_code" => slots.adc_code = index,
                "pcf8591.adc_channel" => slots.adc_channel = index,
                "pcf8591.adc_channel_v" => slots.adc_channel_voltage_v = index,
                "pcf8591.dac_code" => slots.dac_code = index,
                "pcf8591.dac_v" => slots.dac_voltage_v = index,
                "ne555.frequency_hz" => slots.ne555_frequency_hz = index,
                "ne555.level" => slots.ne555_level = index,
                _ => {}
            }
        }

        slots
    }

    #[allow(clippy::too_many_arguments)]
    fn register_signal(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        category: impl Into<String>,
        group: impl Into<String>,
        kind: SignalKind,
        format: &'static str,
        unit: Option<&'static str>,
        default_visible: bool,
    ) -> usize {
        let id = id.into();
        let label = label.into();
        let category = category.into();
        let group = group.into();
        let aliases = signal_aliases(&id, &label, &category, &group);
        let index = self.signals.len();
        self.signal_lookup.insert(id.clone(), index);
        self.signals.push(SignalRecord {
            def: SignalDef {
                id,
                label,
                category,
                group,
                aliases,
                kind,
                format,
                unit,
                default_visible,
            },
            points: Vec::new(),
        });
        index
    }

    fn record_bool_index(&mut self, index: usize, time_ns: u64, value: bool) {
        let should_push = match self.signals[index].points.last() {
            Some(last) => {
                !matches!(last.value, SignalValue::Bool(last_value) if last_value == value)
            }
            None => true,
        };
        if should_push {
            self.push_sample_index(index, time_ns, SignalValue::Bool(value));
        }
    }

    fn record_integer_index(&mut self, index: usize, time_ns: u64, value: i64) {
        let should_push = match self.signals[index].points.last() {
            Some(last) => {
                !matches!(last.value, SignalValue::Integer(last_value) if last_value == value)
            }
            None => true,
        };
        if should_push {
            self.push_sample_index(index, time_ns, SignalValue::Integer(value));
        }
    }

    fn record_float_index(&mut self, index: usize, time_ns: u64, value: f64) {
        let should_push = match self.signals[index].points.last() {
            Some(last) => {
                !matches!(last.value, SignalValue::Float(last_value) if last_value.to_bits() == value.to_bits())
            }
            None => true,
        };
        if should_push {
            self.push_sample_index(index, time_ns, SignalValue::Float(value));
        }
    }

    fn record_text_index(&mut self, index: usize, time_ns: u64, value: &str) {
        let should_push = match self.signals[index].points.last() {
            Some(last) => {
                !matches!(&last.value, SignalValue::Text(last_text) if last_text == value)
            }
            None => true,
        };
        if should_push {
            self.push_sample_index(index, time_ns, SignalValue::Text(value.to_owned()));
        }
    }

    fn record_char_text_index(&mut self, index: usize, time_ns: u64, value: char) {
        let should_push = match self.signals[index].points.last() {
            Some(last) => match &last.value {
                SignalValue::Text(last_text) => {
                    let mut chars = last_text.chars();
                    chars.next() != Some(value) || chars.next().is_some()
                }
                _ => true,
            },
            None => true,
        };
        if should_push {
            let mut text = String::new();
            text.push(value);
            self.record_text_index(index, time_ns, &text);
        }
    }

    fn push_sample_index(&mut self, index: usize, time_ns: u64, value: SignalValue) {
        self.mark_observed_time(time_ns);
        self.signals[index]
            .points
            .push(SamplePoint { time_ns, value });
    }

    fn mark_observed_time(&mut self, time_ns: u64) {
        self.observed_start_ns = Some(match self.observed_start_ns {
            Some(current) => current.min(time_ns),
            None => time_ns,
        });
        self.observed_end_ns = Some(match self.observed_end_ns {
            Some(current) => current.max(time_ns),
            None => time_ns,
        });
    }

    fn capture_bounds(&self) -> (u64, u64) {
        let start_ns = self.observed_start_ns.unwrap_or(self.options.start_ns);
        let end_ns = self.observed_end_ns.unwrap_or(start_ns);
        (start_ns, end_ns)
    }

    fn build_binary_payload(&self) -> WaveBinaryPayload<'_> {
        let (start_ns, end_ns) = self.capture_bounds();
        let signals = self
            .signals
            .iter()
            .map(|signal| {
                WaveBinarySignal(
                    signal.def.id.as_str(),
                    signal.def.label.as_str(),
                    signal.def.category.as_str(),
                    signal.def.group.as_str(),
                    signal.def.aliases.as_slice(),
                    signal.def.kind.as_str(),
                    signal.def.format,
                    signal.def.unit,
                    signal.def.default_visible,
                )
            })
            .collect::<Vec<_>>();
        let samples = self
            .signals
            .iter()
            .map(|signal| {
                signal
                    .points
                    .iter()
                    .map(|point| {
                        WaveBinarySample(point.time_ns, WaveBinaryValue::from(&point.value))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let events = self
            .events
            .iter()
            .filter_map(|event| {
                self.signal_lookup
                    .get(event.track_id)
                    .copied()
                    .map(|track_index| {
                        WaveBinaryEvent(
                            track_index,
                            event.time_ns,
                            event.label.as_str(),
                            event.detail.as_deref(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let markers = self
            .markers
            .iter()
            .map(|marker| WaveBinaryMarker(marker.time_ns, marker.label.as_deref()))
            .collect::<Vec<_>>();
        WaveBinaryPayload {
            version: WAVE_MSGPACK_VERSION,
            start_ns,
            end_ns,
            signals,
            samples,
            events,
            markers,
        }
    }

    fn build_msgpack_payload(&self) -> std::io::Result<Vec<u8>> {
        rmp_serde::to_vec_named(&self.build_binary_payload())
            .map_err(|err| std::io::Error::other(err.to_string()))
    }

    fn flush_json_path(&self, path: &Path) -> std::io::Result<()> {
        let mut writer = BufWriter::new(File::create(path)?);
        self.write_json_payload(&mut writer)?;
        writer.flush()
    }

    fn flush_msgpack_path(&self, path: &Path) -> std::io::Result<()> {
        let payload = self.build_msgpack_payload()?;
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(&payload)?;
        writer.flush()
    }

    fn flush_html_path(&self, path: &Path) -> std::io::Result<()> {
        let payload = self.build_msgpack_payload()?;
        let meta = format!(
            r#"<script id="wave-meta" type="application/json">{{"codec":"msgpack","encoding":"base64","version":{WAVE_MSGPACK_VERSION}}}</script>"#
        );
        let data = format!(
            r#"<script id="wave-data" type="text/plain">{}</script>"#,
            BASE64_STANDARD.encode(payload)
        );
        let document = WAVE_VIEWER_TEMPLATE
            .replace("__WAVE_META_TAG__", &meta)
            .replace("__WAVE_DATA_TAG__", &data)
            .replace("__WAVE_MSGPACK_LIB_TAG__", MSGPACK_BROWSER_LIB);
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(document.as_bytes())?;
        writer.flush()
    }

    fn write_json_payload<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let (start_ns, end_ns) = self.capture_bounds();
        writer.write_all(b"{")?;
        write!(writer, "\"start_ns\":{},", start_ns)?;
        write!(writer, "\"end_ns\":{},", end_ns)?;
        writer.write_all(b"\"signals\":[")?;
        for (index, signal) in self.signals.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"{")?;
            write_json_field(writer, "id", &signal.def.id)?;
            writer.write_all(b",")?;
            write_json_field(writer, "label", &signal.def.label)?;
            writer.write_all(b",")?;
            write_json_field(writer, "category", &signal.def.category)?;
            writer.write_all(b",")?;
            write_json_field(writer, "group", &signal.def.group)?;
            writer.write_all(b",\"aliases\":[")?;
            for (alias_index, alias) in signal.def.aliases.iter().enumerate() {
                if alias_index != 0 {
                    writer.write_all(b",")?;
                }
                write_json_string(writer, alias)?;
            }
            writer.write_all(b"]")?;
            writer.write_all(b",")?;
            write_json_field(writer, "kind", signal.def.kind.as_str())?;
            writer.write_all(b",")?;
            write_json_field(writer, "format", signal.def.format)?;
            writer.write_all(b",\"default_visible\":")?;
            write!(writer, "{}", signal.def.default_visible)?;
            writer.write_all(b",\"unit\":")?;
            match signal.def.unit {
                Some(unit) => write_json_string(writer, unit)?,
                None => writer.write_all(b"null")?,
            }
            writer.write_all(b"}")?;
        }
        writer.write_all(b"],\"samples\":{")?;
        for (index, signal) in self.signals.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",")?;
            }
            write_json_string(writer, &signal.def.id)?;
            writer.write_all(b":[")?;
            for (point_index, point) in signal.points.iter().enumerate() {
                if point_index != 0 {
                    writer.write_all(b",")?;
                }
                writer.write_all(b"{\"t\":")?;
                write!(writer, "{}", point.time_ns)?;
                writer.write_all(b",\"v\":")?;
                write_signal_value(writer, &point.value)?;
                writer.write_all(b"}")?;
            }
            writer.write_all(b"]")?;
        }
        writer.write_all(b"},\"events\":[")?;
        for (index, event) in self.events.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"{")?;
            write_json_field(writer, "track_id", event.track_id)?;
            writer.write_all(b",\"t\":")?;
            write!(writer, "{}", event.time_ns)?;
            writer.write_all(b",")?;
            write_json_field(writer, "label", &event.label)?;
            writer.write_all(b",\"detail\":")?;
            match &event.detail {
                Some(detail) => write_json_string(writer, detail)?,
                None => writer.write_all(b"null")?,
            }
            writer.write_all(b"}")?;
        }
        writer.write_all(b"],\"markers\":[")?;
        for (index, marker) in self.markers.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"{\"t\":")?;
            write!(writer, "{}", marker.time_ns)?;
            writer.write_all(b",\"label\":")?;
            match &marker.label {
                Some(label) => write_json_string(writer, label)?,
                None => writer.write_all(b"null")?,
            }
            writer.write_all(b"}")?;
        }
        writer.write_all(b"]}")?;
        Ok(())
    }
}

impl Drop for WaveRecorder {
    fn drop(&mut self) {
        if !self.enabled() {
            return;
        }
        if let Some(path) = self.options.json_path.as_deref()
            && let Err(err) = self.flush_json_path(path)
        {
            warn!(path = %path.display(), "wave json export failed: {err}");
        }
        if let Some(path) = self.options.html_path.as_deref()
            && let Err(err) = self.flush_html_path(path)
        {
            warn!(path = %path.display(), "wave html export failed: {err}");
        }
        if let Some(path) = self.options.msgpack_path.as_deref()
            && let Err(err) = self.flush_msgpack_path(path)
        {
            warn!(path = %path.display(), "wave msgpack export failed: {err}");
        }
    }
}

fn write_json_field<W: Write>(writer: &mut W, key: &str, value: &str) -> std::io::Result<()> {
    write_json_string(writer, key)?;
    writer.write_all(b":")?;
    write_json_string(writer, value)
}

fn write_signal_value<W: Write>(writer: &mut W, value: &SignalValue) -> std::io::Result<()> {
    match value {
        SignalValue::Bool(value) => write!(writer, "{value}"),
        SignalValue::Integer(value) => write!(writer, "{value}"),
        SignalValue::Float(value) => write!(writer, "{value:.6}"),
        SignalValue::Text(value) => write_json_string(writer, value),
    }
}

fn write_json_string<W: Write>(writer: &mut W, text: &str) -> std::io::Result<()> {
    writer.write_all(b"\"")?;
    for ch in text.chars() {
        match ch {
            '"' => writer.write_all(br#"\""#)?,
            '\\' => writer.write_all(br#"\\"#)?,
            '\n' => writer.write_all(br#"\n"#)?,
            '\r' => writer.write_all(br#"\r"#)?,
            '\t' => writer.write_all(br#"\t"#)?,
            ch if ch <= '\u{1F}' => write!(writer, "\\u{:04X}", ch as u32)?,
            ch => write!(writer, "{ch}")?,
        }
    }
    writer.write_all(b"\"")
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{
        TRACK_EVENT_I2C, WaveCaptureWindow, WaveEventNote, WaveMarkerNote, WaveRecorder,
        signal_aliases,
    };

    #[derive(Debug, Deserialize)]
    struct DecodedMsgpackPayload {
        version: u8,
        signals: Vec<DecodedMsgpackSignal>,
        markers: Vec<DecodedMsgpackMarker>,
    }

    #[derive(Debug, Deserialize)]
    struct DecodedMsgpackSignal(
        String,
        String,
        String,
        String,
        Vec<String>,
        String,
        String,
        Option<String>,
        bool,
    );

    #[derive(Debug, Deserialize)]
    struct DecodedMsgpackMarker(u64, Option<String>);

    #[test]
    fn signal_aliases_include_iic_variants() {
        let aliases = signal_aliases("event.i2c", "I2C events", "protocol", "i2c");
        assert!(aliases.iter().any(|alias| alias == "event.iic"));
        assert!(aliases.iter().any(|alias| alias == "iic"));
    }

    #[test]
    fn signal_aliases_include_serial_variants() {
        let aliases = signal_aliases("uart1.tx", "TX", "protocol", "uart1");
        assert!(aliases.iter().any(|alias| alias == "serial1 tx"));
        assert!(aliases.iter().any(|alias| alias == "serial 1"));
    }

    #[test]
    fn recorder_registers_uart_interrupt_flag_signals() {
        let recorder = WaveRecorder::new_with_window(WaveCaptureWindow::bounded(0, Some(100)));
        let ids = recorder
            .signals
            .iter()
            .map(|signal| signal.def.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"uart1.ti"));
        assert!(ids.contains(&"uart1.ri"));
        assert!(ids.contains(&"uart2.ti"));
        assert!(ids.contains(&"uart2.ri"));
    }

    #[test]
    fn signal_aliases_include_adc_variants_for_ain_inputs() {
        let aliases = signal_aliases("analog.rd1_v", "RD1/AIN1", "analog", "pcf8591_ne555");
        assert!(aliases.iter().any(|alias| alias == "rd1/adc1"));
        assert!(aliases.iter().any(|alias| alias == "rd1 adc1"));
    }

    #[test]
    fn recorder_window_filters_out_of_range_events() {
        let mut recorder =
            WaveRecorder::new_with_window(WaveCaptureWindow::bounded(100, Some(200)));

        recorder.record_event_note(WaveEventNote::new(90, TRACK_EVENT_I2C, "before"));
        recorder.record_event_note(WaveEventNote::new(100, TRACK_EVENT_I2C, "inside"));
        recorder.record_event_note(WaveEventNote::new(210, TRACK_EVENT_I2C, "after"));

        assert_eq!(recorder.events.len(), 1);
        assert_eq!(recorder.events[0].time_ns, 100);
        assert_eq!(recorder.events[0].label, "inside");
    }

    #[test]
    fn recorder_window_filters_and_trims_markers() {
        let mut recorder =
            WaveRecorder::new_with_window(WaveCaptureWindow::bounded(100, Some(200)));

        recorder.record_marker_note(WaveMarkerNote::anonymous(90));
        recorder.record_marker_note(WaveMarkerNote::named(120, "  boot  "));
        recorder.record_marker_note(WaveMarkerNote::named(180, "   "));
        recorder.record_marker_note(WaveMarkerNote::named(210, "after"));

        assert_eq!(
            recorder.marker_records(),
            vec![(120, Some(String::from("boot"))), (180, None)]
        );
    }

    #[test]
    fn msgpack_payload_contains_signal_metadata_and_markers() {
        let mut recorder = WaveRecorder::new_with_window(WaveCaptureWindow::bounded(0, Some(100)));
        recorder.record_marker_note(WaveMarkerNote::named(42, "irq"));
        recorder.record_marker_note(WaveMarkerNote::anonymous(84));
        let payload = recorder.build_msgpack_payload().expect("build msgpack");
        let decoded: DecodedMsgpackPayload =
            rmp_serde::from_slice(&payload).expect("decode msgpack");

        assert_eq!(decoded.version, 1);
        assert!(!decoded.signals.is_empty());

        let first = &decoded.signals[0];
        assert!(!first.0.is_empty());
        assert!(!first.1.is_empty());
        assert!(!first.2.is_empty());
        assert!(!first.3.is_empty());
        assert!(!first.5.is_empty());
        assert!(!first.6.is_empty());
        let _ = (&first.4, &first.7, first.8);

        assert_eq!(decoded.markers.len(), 2);
        assert_eq!(decoded.markers[0].0, 42);
        assert_eq!(decoded.markers[0].1.as_deref(), Some("irq"));
        assert_eq!(decoded.markers[1].0, 84);
        assert_eq!(decoded.markers[1].1, None);
    }
}
