use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use tracing::warn;

pub(crate) const TRACK_EVENT_CPU: &str = "event.cpu";
pub(crate) const TRACK_EVENT_I2C: &str = "event.i2c";
pub(crate) const TRACK_EVENT_ONEWIRE: &str = "event.onewire";
pub(crate) const TRACK_EVENT_UART1: &str = "event.uart1";
pub(crate) const TRACK_EVENT_UART2: &str = "event.uart2";
pub(crate) const TRACK_EVENT_ADC_DAC: &str = "event.adc_dac";
pub(crate) const TRACK_EVENT_DS1302: &str = "event.ds1302";

const KEY_NAMES: [&str; 16] = [
    "S4", "S5", "S6", "S7", "S8", "S9", "S10", "S11", "S12", "S13", "S14", "S15", "S16", "S17",
    "S18", "S19",
];
const LED_NAMES: [&str; 8] = ["L1", "L2", "L3", "L4", "L5", "L6", "L7", "L8"];

#[derive(Debug, Clone, Default)]
pub(crate) struct WaveCaptureOptions {
    pub(crate) html_path: Option<PathBuf>,
    pub(crate) json_path: Option<PathBuf>,
    pub(crate) start_ns: u64,
    pub(crate) end_ns: Option<u64>,
}

impl WaveCaptureOptions {
    pub(crate) fn enabled(&self) -> bool {
        self.html_path.is_some() || self.json_path.is_some()
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
    pub(crate) uart2_tx_high: bool,
    pub(crate) uart2_rx_high: bool,
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

impl SignalValue {
    fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
            (Self::Text(left), Self::Text(right)) => left == right,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
struct SamplePoint {
    time_ns: u64,
    value: SignalValue,
}

#[derive(Debug, Clone)]
struct EventRecord {
    track_id: String,
    time_ns: u64,
    label: String,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
struct SignalRecord {
    def: SignalDef,
    last_value: Option<SignalValue>,
    points: Vec<SamplePoint>,
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

#[derive(Debug, Default, Clone)]
struct I2cEventDecoder {
    initialized: bool,
    prev_scl: bool,
    prev_sda: bool,
    active: bool,
    bit_count: u8,
    shift: u8,
    waiting_ack: bool,
    expecting_address: bool,
    reading: bool,
    last_byte: u8,
}

impl I2cEventDecoder {
    fn observe(&mut self, time_ns: u64, scl_high: bool, sda_high: bool) -> Vec<WaveEventNote> {
        let mut events = Vec::new();
        if !self.initialized {
            self.initialized = true;
            self.prev_scl = scl_high;
            self.prev_sda = sda_high;
            return events;
        }

        let start = self.prev_sda && !sda_high && self.prev_scl && scl_high;
        let stop = !self.prev_sda && sda_high && self.prev_scl && scl_high;

        if start {
            let label = if self.active {
                "REPEATED START"
            } else {
                "START"
            };
            events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, label));
            self.active = true;
            self.bit_count = 0;
            self.shift = 0;
            self.waiting_ack = false;
            self.expecting_address = true;
        }

        if self.active && !self.prev_scl && scl_high {
            if self.waiting_ack {
                let ack = !sda_high;
                let label = if ack { "ACK" } else { "NACK" };
                events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, label));
                self.waiting_ack = false;
                if self.expecting_address {
                    self.reading = self.last_byte & 0x01 != 0;
                    self.expecting_address = false;
                }
            } else {
                self.shift = (self.shift << 1) | u8::from(sda_high);
                self.bit_count += 1;
                if self.bit_count == 8 {
                    let byte = self.shift;
                    let note = if self.expecting_address {
                        WaveEventNote::with_detail(
                            time_ns,
                            TRACK_EVENT_I2C,
                            format!(
                                "ADDR 0x{:02X} {}",
                                byte,
                                if byte & 0x01 != 0 { "R" } else { "W" }
                            ),
                            format!("raw=0x{byte:02X}"),
                        )
                    } else if self.reading {
                        WaveEventNote::new(time_ns, TRACK_EVENT_I2C, format!("RX 0x{byte:02X}"))
                    } else {
                        WaveEventNote::new(time_ns, TRACK_EVENT_I2C, format!("TX 0x{byte:02X}"))
                    };
                    events.push(note);
                    self.last_byte = byte;
                    self.shift = 0;
                    self.bit_count = 0;
                    self.waiting_ack = true;
                }
            }
        }

        if stop && self.active {
            events.push(WaveEventNote::new(time_ns, TRACK_EVENT_I2C, "STOP"));
            self.active = false;
            self.bit_count = 0;
            self.shift = 0;
            self.waiting_ack = false;
            self.expecting_address = false;
            self.reading = false;
        }

        self.prev_scl = scl_high;
        self.prev_sda = sda_high;
        events
    }
}

pub(crate) struct WaveRecorder {
    options: WaveCaptureOptions,
    window: WaveCaptureWindow,
    signal_lookup: HashMap<String, usize>,
    signals: Vec<SignalRecord>,
    events: Vec<EventRecord>,
    i2c_decoder: I2cEventDecoder,
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
            events: Vec::new(),
            i2c_decoder: I2cEventDecoder::default(),
            observed_start_ns: None,
            observed_end_ns: None,
        };
        recorder.register_defaults();
        recorder
    }

    pub(crate) fn enabled(&self) -> bool {
        self.window.enabled()
    }

    pub(crate) fn captures_time(&self, time_ns: u64) -> bool {
        self.window.includes(time_ns)
    }

    pub(crate) fn window(&self) -> WaveCaptureWindow {
        self.window
    }

    #[cfg(test)]
    pub(crate) fn new_with_window(window: WaveCaptureWindow) -> Self {
        let mut recorder = Self {
            options: WaveCaptureOptions::default(),
            window,
            signal_lookup: HashMap::new(),
            signals: Vec::new(),
            events: Vec::new(),
            i2c_decoder: I2cEventDecoder::default(),
            observed_start_ns: None,
            observed_end_ns: None,
        };
        recorder.register_defaults();
        recorder
    }

    pub(crate) fn observe_snapshot(&mut self, snapshot: &WaveSnapshot) {
        if !self.window.includes(snapshot.time_ns) {
            return;
        }

        for port in 0..6 {
            self.record_integer(
                &format!("pin.p{port}"),
                snapshot.time_ns,
                i64::from(snapshot.port_input[port]),
            );
            self.record_integer(
                &format!("latch.p{port}"),
                snapshot.time_ns,
                i64::from(snapshot.port_latch[port]),
            );
            for bit in 0..8 {
                let pin_high = snapshot.port_input[port] & (1 << bit) != 0;
                let latch_high = snapshot.port_latch[port] & (1 << bit) != 0;
                self.record_bool(&format!("pin.p{port}.{bit}"), snapshot.time_ns, pin_high);
                self.record_bool(
                    &format!("latch.p{port}.{bit}"),
                    snapshot.time_ns,
                    latch_high,
                );
            }
        }

        for slot in 0..4 {
            self.record_integer(
                &format!("board.effective.{slot}"),
                snapshot.time_ns,
                i64::from(snapshot.board_latches_effective[slot]),
            );
            self.record_integer(
                &format!("board.port.{slot}"),
                snapshot.time_ns,
                i64::from(snapshot.board_latches_port[slot]),
            );
            self.record_integer(
                &format!("board.xdata.{slot}"),
                snapshot.time_ns,
                i64::from(snapshot.board_latches_xdata[slot]),
            );
        }

        self.record_bool("signal.sig_out", snapshot.time_ns, snapshot.signal_sig_out);
        self.record_bool(
            "jumper.net_sig_sig_out",
            snapshot.time_ns,
            snapshot.jumper_net_sig_to_sig_out,
        );

        self.record_bool("i2c.master_scl", snapshot.time_ns, snapshot.i2c_master_scl);
        self.record_bool("i2c.master_sda", snapshot.time_ns, snapshot.i2c_master_sda);
        self.record_bool("i2c.bus_scl", snapshot.time_ns, snapshot.i2c_bus_scl);
        self.record_bool("i2c.bus_sda", snapshot.time_ns, snapshot.i2c_bus_sda);
        self.record_bool(
            "i2c.slave_scl_low",
            snapshot.time_ns,
            snapshot.i2c_slave_scl_low,
        );
        self.record_bool(
            "i2c.slave_sda_low",
            snapshot.time_ns,
            snapshot.i2c_slave_sda_low,
        );

        self.record_bool(
            "onewire.master_high",
            snapshot.time_ns,
            snapshot.onewire_master_high,
        );
        self.record_bool(
            "onewire.bus_high",
            snapshot.time_ns,
            snapshot.onewire_bus_high,
        );
        self.record_bool(
            "onewire.device_low",
            snapshot.time_ns,
            snapshot.onewire_device_low,
        );

        self.record_bool("ds1302.ce", snapshot.time_ns, snapshot.ds1302_ce);
        self.record_bool("ds1302.clk", snapshot.time_ns, snapshot.ds1302_clk);
        self.record_bool("ds1302.io", snapshot.time_ns, snapshot.ds1302_io);

        self.record_bool("uart1.tx", snapshot.time_ns, snapshot.uart1_tx_high);
        self.record_bool("uart1.rx", snapshot.time_ns, snapshot.uart1_rx_high);
        self.record_bool("uart2.tx", snapshot.time_ns, snapshot.uart2_tx_high);
        self.record_bool("uart2.rx", snapshot.time_ns, snapshot.uart2_rx_high);

        for (index, name) in KEY_NAMES.iter().enumerate() {
            self.record_bool(
                &format!("key.{}", name.to_ascii_lowercase()),
                snapshot.time_ns,
                snapshot.key_states[index],
            );
        }

        for (index, name) in LED_NAMES.iter().enumerate() {
            self.record_bool(
                &format!("led.{}", name.to_ascii_lowercase()),
                snapshot.time_ns,
                snapshot.led_states[index],
            );
        }
        self.record_bool("output.relay", snapshot.time_ns, snapshot.relay_on);
        self.record_bool("output.motor", snapshot.time_ns, snapshot.motor_on);
        self.record_bool("output.buzzer", snapshot.time_ns, snapshot.buzzer_on);

        self.record_text("seg.text", snapshot.time_ns, snapshot.seg_text.clone());
        for digit in 0..8 {
            self.record_text(
                &format!("seg.d{}.text", digit + 1),
                snapshot.time_ns,
                snapshot.seg_chars[digit].to_string(),
            );
            self.record_integer(
                &format!("seg.d{}.raw", digit + 1),
                snapshot.time_ns,
                i64::from(snapshot.seg_raw[digit]),
            );
        }

        self.record_float(
            "analog.rd1_v",
            snapshot.time_ns,
            f64::from(snapshot.analog_rd1_v),
        );
        self.record_float(
            "analog.rb2_v",
            snapshot.time_ns,
            f64::from(snapshot.analog_rb2_v),
        );
        self.record_integer(
            "pcf8591.adc_code",
            snapshot.time_ns,
            i64::from(snapshot.adc_code),
        );
        self.record_integer(
            "pcf8591.adc_channel",
            snapshot.time_ns,
            i64::from(snapshot.adc_channel),
        );
        self.record_float(
            "pcf8591.adc_channel_v",
            snapshot.time_ns,
            f64::from(snapshot.adc_channel_voltage_v),
        );
        self.record_integer(
            "pcf8591.dac_code",
            snapshot.time_ns,
            i64::from(snapshot.dac_code),
        );
        self.record_float(
            "pcf8591.dac_v",
            snapshot.time_ns,
            f64::from(snapshot.dac_voltage_v),
        );

        self.record_bool("ne555.level", snapshot.time_ns, snapshot.ne555_level);
        self.record_float(
            "ne555.frequency_hz",
            snapshot.time_ns,
            f64::from(snapshot.ne555_frequency_hz),
        );

        for note in
            self.i2c_decoder
                .observe(snapshot.time_ns, snapshot.i2c_bus_scl, snapshot.i2c_bus_sda)
        {
            self.record_event_note(note);
        }
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
            track_id: note.track_id.to_string(),
            time_ns: note.time_ns,
            label: note.label,
            detail: note.detail,
        });
    }

    fn register_defaults(&mut self) {
        for port in 0..6 {
            self.register_signal(
                format!("pin.p{port}"),
                format!("P{port} pin byte"),
                "pins",
                format!("P{port} pins"),
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            self.register_signal(
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
                self.register_signal(
                    format!("pin.p{port}.{bit}"),
                    format!("P{port}.{bit} pin"),
                    "pins",
                    format!("P{port} pins"),
                    SignalKind::Digital,
                    "bool",
                    None,
                    false,
                );
                self.register_signal(
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
            self.register_signal(
                format!("board.effective.{slot}"),
                format!("effective latch {slot}"),
                "board_latches",
                "effective",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            self.register_signal(
                format!("board.port.{slot}"),
                format!("port latch {slot}"),
                "board_latches",
                "port",
                SignalKind::Integer,
                "hex8",
                None,
                false,
            );
            self.register_signal(
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
            self.register_signal(
                id,
                label,
                "board_signals",
                "jumpers",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
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
            self.register_signal(
                id,
                label,
                "protocol",
                "i2c",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
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
            self.register_signal(
                id,
                label,
                "protocol",
                "onewire",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
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
            ("uart2.tx", "TX", "uart2", false),
            ("uart2.rx", "RX", "uart2", false),
        ] {
            self.register_signal(
                id,
                label,
                "protocol",
                group,
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
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
            self.register_signal(
                id,
                label,
                "protocol",
                "ds1302",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
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
            let _ = index;
            self.register_signal(
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

        for name in LED_NAMES {
            self.register_signal(
                format!("led.{}", name.to_ascii_lowercase()),
                name,
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
            self.register_signal(
                id,
                label,
                "outputs",
                "board_outputs",
                SignalKind::Digital,
                "bool",
                None,
                visible,
            );
        }

        self.register_signal(
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
            self.register_signal(
                format!("seg.d{digit}.text"),
                format!("D{digit} char"),
                "display",
                "seg_digits",
                SignalKind::Text,
                "text",
                None,
                false,
            );
            self.register_signal(
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
            self.register_signal(
                id,
                label,
                "analog",
                "pcf8591_ne555",
                kind,
                format,
                unit,
                visible,
            );
        }
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
    ) {
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
            last_value: None,
            points: Vec::new(),
        });
    }

    fn record_bool(&mut self, id: &str, time_ns: u64, value: bool) {
        self.record_value(id, time_ns, SignalValue::Bool(value));
    }

    fn record_integer(&mut self, id: &str, time_ns: u64, value: i64) {
        self.record_value(id, time_ns, SignalValue::Integer(value));
    }

    fn record_float(&mut self, id: &str, time_ns: u64, value: f64) {
        self.record_value(id, time_ns, SignalValue::Float(value));
    }

    fn record_text(&mut self, id: &str, time_ns: u64, value: String) {
        self.record_value(id, time_ns, SignalValue::Text(value));
    }

    fn record_value(&mut self, id: &str, time_ns: u64, value: SignalValue) {
        let Some(index) = self.signal_lookup.get(id).copied() else {
            return;
        };
        let should_push = match self.signals[index].last_value.as_ref() {
            Some(last) => !last.same_as(&value),
            None => true,
        };
        if should_push {
            self.mark_observed_time(time_ns);
            let record = &mut self.signals[index];
            record.last_value = Some(value.clone());
            record.points.push(SamplePoint { time_ns, value });
        }
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

    fn flush_json_path(&self, path: &Path) -> std::io::Result<()> {
        let mut writer = BufWriter::new(File::create(path)?);
        self.write_json_payload(&mut writer)?;
        writer.flush()
    }

    fn flush_html_path(&self, path: &Path) -> std::io::Result<()> {
        let mut payload = Vec::new();
        self.write_json_payload(&mut payload)?;
        let payload =
            String::from_utf8(payload).map_err(|err| std::io::Error::other(err.to_string()))?;
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(html_template_prefix().as_bytes())?;
        writer.write_all(payload.as_bytes())?;
        writer.write_all(html_template_suffix().as_bytes())?;
        writer.flush()
    }

    fn write_json_payload<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let start_ns = self.observed_start_ns.unwrap_or(self.options.start_ns);
        let end_ns = self.observed_end_ns.unwrap_or(start_ns);
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
            write_json_field(writer, "track_id", &event.track_id)?;
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

fn html_template_prefix() -> &'static str {
    r###"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>stcjudge wave</title>
<style>
html,
body {
  height: 100%;
  margin: 0;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  background: #0f1220;
  color: #e8ebff;
  overflow: hidden;
}
.shell {
  display: grid;
  grid-template-columns: minmax(0, 320px) minmax(0, 1fr);
  height: 100vh;
  min-height: 0;
}
.shell.sidebar-collapsed {
  grid-template-columns: 0 minmax(0, 1fr);
}
.sidebar {
  grid-column: 1;
  border-right: 1px solid #2a3152;
  background: #151a2d;
  padding: 16px;
  overflow: auto;
  min-height: 0;
  min-width: 0;
  transition:
    padding 0.16s ease,
    opacity 0.16s ease,
    border-right-color 0.16s ease;
}
.shell.sidebar-collapsed .sidebar {
  display: none;
}
.main {
  grid-column: 2;
  display: grid;
  grid-template-rows: auto 1fr;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.shell.sidebar-collapsed .main {
  grid-column: 1 / -1;
}
.header {
  position: sticky;
  top: 0;
  z-index: 20;
  background: #12172a;
  border-bottom: 1px solid #2a3152;
}
.toolbar {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  grid-template-rows: auto auto;
  column-gap: 12px;
  row-gap: 6px;
  align-items: center;
  padding: 12px 16px;
  background: #12172a;
  overflow: hidden;
}
.toolbar-actions {
  grid-row: 1 / span 2;
  grid-column: 1;
  display: flex;
  flex-wrap: nowrap;
  gap: 8px;
  align-items: center;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  align-self: center;
}
.toolbar-toggle.active {
  background: #30406d;
  border-color: #7ea1dc;
}
.toolbar-info {
  grid-row: 1 / span 2;
  grid-column: 2;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: minmax(calc(1.4em + 2px), auto) minmax(calc(1.4em + 2px), auto);
  row-gap: 4px;
  align-items: center;
  width: 100%;
  min-width: 0;
}
.toolbar-slot {
  display: block;
  text-align: left;
  white-space: nowrap;
  line-height: 1.4;
  min-width: 0;
  min-height: calc(1.4em + 2px);
  overflow-x: auto;
  overflow-y: hidden;
}
.toolbar-slot-stats {
  grid-row: 1;
}
.toolbar-slot-cursor {
  grid-row: 2;
}
.marker-panel {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  padding: 0 16px 12px 16px;
  background: #12172a;
}
.marker-controls {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}
.marker-input {
  width: 150px;
  margin: 0;
  box-sizing: border-box;
  background: #101628;
  color: #eef2ff;
  border: 1px solid #354675;
  border-radius: 6px;
  padding: 7px 10px;
}
.marker-strip {
  display: flex;
  flex: 1 1 320px;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  min-width: 0;
}
.marker-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: 1px solid #354675;
  border-radius: 999px;
  background: #182234;
  color: #eef2ff;
  cursor: pointer;
  user-select: none;
}
.marker-chip.active {
  background: #22314e;
  border-color: #7ea1dc;
}
.marker-swatch {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--marker-color, #facc15);
  flex: 0 0 auto;
}
.marker-remove {
  min-width: 20px;
  padding: 2px 6px;
  border-radius: 999px;
  line-height: 1;
}
.marker-status {
  color: #f6c3c3;
  font-size: 12px;
  white-space: nowrap;
}
.overview-row {
  display: grid;
  grid-template-columns: 220px 1fr;
  align-items: center;
  gap: 0;
  padding: 0 16px 12px 16px;
}
.overview-gutter {
  height: 40px;
}
button,
input {
  font: inherit;
}
button {
  background: #243156;
  color: #eef2ff;
  border: 1px solid #354675;
  border-radius: 6px;
  padding: 6px 10px;
  cursor: pointer;
}
button:hover {
  background: #30406d;
}
button:disabled {
  background: #182234;
  color: #7b86b9;
  border-color: #2b3857;
  cursor: default;
}
button:disabled:hover {
  background: #182234;
}
input[type="search"] {
  width: 100%;
  box-sizing: border-box;
  background: #101628;
  color: #eef2ff;
  border: 1px solid #354675;
  border-radius: 6px;
  padding: 8px 10px;
  margin-bottom: 12px;
}
.sidebar-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}
.sidebar-title {
  color: #eef2ff;
  font-size: 13px;
  font-weight: 700;
}
.sidebar-close {
  padding: 5px 8px;
}
.stats {
  color: #9aa4d6;
  font-size: 12px;
  white-space: nowrap;
}
.coverage-track {
  position: relative;
  width: 100%;
  height: 40px;
  border: 1px solid #354675;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), rgba(255, 255, 255, 0)),
    linear-gradient(90deg, #0b1020, #11182c);
  overflow: visible;
  user-select: none;
  box-sizing: border-box;
}
.coverage-range-label {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  max-width: calc(100% - 8px);
  padding: 2px 6px;
  border: 1px solid rgba(126, 161, 220, 0.45);
  border-radius: 999px;
  background: rgba(10, 14, 27, 0.88);
  color: #eef2ff;
  font-size: 11px;
  line-height: 1.2;
  white-space: nowrap;
  pointer-events: none;
  z-index: 4;
}
.coverage-range-label.inside {
  background: rgba(22, 31, 54, 0.94);
}
.coverage-markers {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 1;
}
.coverage-marker {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 12px;
  transform: translateX(-6px);
  pointer-events: auto;
  cursor: ew-resize;
  z-index: 3;
}
.coverage-marker.active::before {
  width: 4px;
  opacity: 1;
}
.coverage-marker.active::after {
  border-top-width: 10px;
}
.coverage-marker::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 50%;
  width: 2px;
  transform: translateX(-50%);
  background: var(--marker-color, #facc15);
  opacity: 0.95;
}
.coverage-marker::after {
  content: "";
  position: absolute;
  top: 0;
  left: 50%;
  width: 0;
  height: 0;
  transform: translateX(-50%);
  border-left: 5px solid transparent;
  border-right: 5px solid transparent;
  border-top: 8px solid var(--marker-color, #facc15);
  filter: drop-shadow(0 0 4px rgba(12, 16, 32, 0.65));
}
.coverage-window {
  position: absolute;
  top: 0;
  bottom: 0;
  min-width: 2px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.12), rgba(255, 255, 255, 0.02)),
    linear-gradient(90deg, rgba(110, 168, 255, 0.38), rgba(138, 255, 193, 0.32));
  box-shadow:
    0 0 0 1px rgba(212, 232, 255, 0.18) inset,
    0 0 0 1px rgba(58, 92, 150, 0.55);
  cursor: grab;
  z-index: 2;
}
.coverage-window.dragging {
  cursor: grabbing;
}
.coverage-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 10px;
  background: rgba(255, 255, 255, 0.18);
  cursor: ew-resize;
}
.coverage-handle-left {
  left: 0;
}
.coverage-handle-right {
  right: 0;
}
.group {
  margin-bottom: 14px;
  border: 1px solid #273153;
  border-radius: 8px;
  overflow: hidden;
}
.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  background: #1b2240;
  font-weight: 700;
}
.group-body {
  padding: 10px 12px;
  display: grid;
  gap: 6px;
  background: #13192d;
}
label {
  display: flex;
  align-items: center;
  gap: 8px;
  line-height: 1.3;
}
.viewer {
  overflow: auto;
  background: #0b0f1d;
  min-height: 0;
  padding: 0 16px;
  box-sizing: border-box;
  position: relative;
}
.viewer-ruler {
  position: sticky;
  top: 0;
  z-index: 8;
  margin: 0 -16px;
  padding: 0 16px;
  background: #0d1324;
  border-bottom: 1px solid #1f2744;
}
canvas {
  display: block;
}
.legend {
  color: #9aa4d6;
  font-size: 12px;
}
</style>
</head>
<body>
<div class="shell" id="shell">
  <aside class="sidebar" id="sidebar-pane">
    <div class="sidebar-top">
      <span class="sidebar-title">Signals</span>
      <button id="sidebar-close" class="sidebar-close" type="button">Hide</button>
    </div>
    <input id="search" type="search" placeholder="Filter signals">
    <div class="legend">Categories and signals can be combined freely.</div>
    <div id="sidebar"></div>
  </aside>
  <section class="main">
    <div class="header">
      <div class="toolbar">
        <div class="toolbar-actions">
          <button id="sidebar-toggle" class="toolbar-toggle" type="button">Filters</button>
          <button id="zoom-in">Zoom in</button>
          <button id="zoom-out">Zoom out</button>
          <button id="reset">Reset</button>
          <button id="show-default">Default</button>
          <button id="hide-all">Hide all</button>
        </div>
        <div class="toolbar-info">
          <span class="stats toolbar-slot toolbar-slot-stats" id="stats"></span>
          <span class="stats toolbar-slot toolbar-slot-cursor" id="cursor-info"></span>
        </div>
      </div>
      <div class="marker-panel">
        <div class="marker-controls">
          <input id="marker-time" class="marker-input" type="text" placeholder="time, e.g. 12.5 ms">
          <input id="marker-label" class="marker-input" type="text" placeholder="label, optional">
          <button id="marker-add">Add marker</button>
          <button id="marker-add-cursor">Use cursor</button>
        </div>
        <div class="marker-strip" id="marker-strip"></div>
        <span class="marker-status" id="marker-status"></span>
      </div>
      <div class="overview-row">
        <div class="overview-gutter"></div>
        <div class="coverage-track" id="coverage-info" aria-label="window coverage">
          <span class="coverage-range-label" id="coverage-range-start"></span>
          <span class="coverage-range-label" id="coverage-range-end"></span>
          <div class="coverage-markers" id="coverage-markers"></div>
          <div class="coverage-window" id="coverage-window">
            <div class="coverage-handle coverage-handle-left" id="coverage-handle-left"></div>
            <div class="coverage-handle coverage-handle-right" id="coverage-handle-right"></div>
          </div>
        </div>
      </div>
    </div>
    <div class="viewer" id="viewer">
      <div class="viewer-ruler" id="viewer-ruler">
        <canvas id="ruler-canvas"></canvas>
      </div>
      <canvas id="canvas"></canvas>
    </div>
  </section>
</div>
<script id="wave-data" type="application/json">
"###
}

fn html_template_suffix() -> &'static str {
    r###"
</script>
<script>
const data = JSON.parse(document.getElementById("wave-data").textContent);
const shell = document.getElementById("shell");
const sidebarPane = document.getElementById("sidebar-pane");
const sidebar = document.getElementById("sidebar");
const sidebarToggle = document.getElementById("sidebar-toggle");
const sidebarClose = document.getElementById("sidebar-close");
const header = document.querySelector(".header");
const viewer = document.getElementById("viewer");
const viewerRuler = document.getElementById("viewer-ruler");
const rulerCanvas = document.getElementById("ruler-canvas");
const rulerCtx = rulerCanvas.getContext("2d");
const canvas = document.getElementById("canvas");
const ctx = canvas.getContext("2d");
const coverageInfo = document.getElementById("coverage-info");
const coverageRangeStart = document.getElementById("coverage-range-start");
const coverageRangeEnd = document.getElementById("coverage-range-end");
const coverageMarkers = document.getElementById("coverage-markers");
const coverageWindow = document.getElementById("coverage-window");
const coverageHandleLeft = document.getElementById("coverage-handle-left");
const coverageHandleRight = document.getElementById("coverage-handle-right");
const stats = document.getElementById("stats");
const cursorInfo = document.getElementById("cursor-info");
const search = document.getElementById("search");
const markerTimeInput = document.getElementById("marker-time");
const markerLabelInput = document.getElementById("marker-label");
const markerAddButton = document.getElementById("marker-add");
const markerAddCursorButton = document.getElementById("marker-add-cursor");
const markerStrip = document.getElementById("marker-strip");
const markerStatus = document.getElementById("marker-status");

const SIGNAL_LABEL_WIDTH = 220;
const VIEWER_SIDE_PADDING_X = 32;
const VIEWER_ROW_HEIGHT = 54;
const VIEWER_TOP_PADDING = 8;
const VIEWER_RULER_HEIGHT = 36;

const signals = data.signals.map(signal => ({
  ...signal,
  aliases: Array.isArray(signal.aliases) ? signal.aliases.map(alias => String(alias).toLowerCase()) : [],
}));
const signalById = new Map(signals.map(signal => [signal.id, signal]));
const samples = data.samples;
const events = data.events;
const eventsByTrack = new Map();
for (const event of events) {
  if (!eventsByTrack.has(event.track_id)) {
    eventsByTrack.set(event.track_id, []);
  }
  eventsByTrack.get(event.track_id).push(event);
}

const groups = new Map();
for (const signal of signals) {
  const key = `${signal.category}::${signal.group}`;
  if (!groups.has(key)) {
    groups.set(key, {
      category: signal.category,
      group: signal.group,
      signals: [],
    });
  }
  groups.get(key).signals.push(signal);
}

const selected = new Set(
  signals.filter(signal => signal.default_visible).map(signal => signal.id)
);
let signalOrder = signals.map(signal => signal.id);

let viewStart = data.start_ns;
let viewEnd = data.end_ns;
let dragState = null;
let hoverState = null;
let hoverActionSignalId = null;
let hoverHandleSignalId = null;
let rowLayout = [];
let markers = [];
let nextMarkerId = 1;
let activeMarkerId = null;
let sidebarCollapsed = false;

function loadSidebarCollapsed() {
  try {
    return window.localStorage.getItem("stcjudge.wave.sidebarCollapsed") === "1";
  } catch (_error) {
    return false;
  }
}

function storeSidebarCollapsed(collapsed) {
  try {
    window.localStorage.setItem("stcjudge.wave.sidebarCollapsed", collapsed ? "1" : "0");
  } catch (_error) {}
}

function applySidebarCollapsed(collapsed, options = {}) {
  const { persist = true, shouldRender = true } = options;
  sidebarCollapsed = Boolean(collapsed);
  shell.classList.toggle("sidebar-collapsed", sidebarCollapsed);
  sidebarPane.setAttribute("aria-hidden", sidebarCollapsed ? "true" : "false");
  sidebarToggle.classList.toggle("active", !sidebarCollapsed);
  sidebarToggle.setAttribute("aria-expanded", sidebarCollapsed ? "false" : "true");
  sidebarToggle.title = sidebarCollapsed ? "Show filters" : "Hide filters";
  if (persist) {
    storeSidebarCollapsed(sidebarCollapsed);
  }
  if (sidebarCollapsed && sidebarPane.contains(document.activeElement)) {
    sidebarToggle.focus();
  }
  if (shouldRender) {
    render();
  }
}

function toggleSidebarCollapsed() {
  applySidebarCollapsed(!sidebarCollapsed);
}

function searchQuery() {
  return search.value.trim().toLowerCase();
}

function clampView() {
  const fullSpan = Math.max(1, data.end_ns - data.start_ns);
  const span = Math.max(1, viewEnd - viewStart);
  if (span > fullSpan) {
    viewStart = data.start_ns;
    viewEnd = data.end_ns;
    return;
  }
  if (viewStart < data.start_ns) {
    viewEnd += data.start_ns - viewStart;
    viewStart = data.start_ns;
  }
  if (viewEnd > data.end_ns) {
    viewStart -= viewEnd - data.end_ns;
    viewEnd = data.end_ns;
  }
  viewStart = Math.max(data.start_ns, viewStart);
  viewEnd = Math.min(data.end_ns, viewEnd);
}

function totalSpanNs() {
  return Math.max(1, data.end_ns - data.start_ns);
}

function clampTimeNs(timeNs) {
  return Math.max(data.start_ns, Math.min(data.end_ns, Math.round(timeNs)));
}

function formatMarkerTime(timeNs) {
  const stepNs = niceTimeStepNs(Math.max(1, viewSpanNs() / 6));
  return formatTimeNs(timeNs, stepNs, chooseTimeUnit(stepNs));
}

function markerById(markerId) {
  return markers.find(marker => marker.id === markerId) || null;
}

function sortMarkers() {
  markers.sort((left, right) => {
    if (left.t !== right.t) {
      return left.t - right.t;
    }
    return left.id - right.id;
  });
}

function setMarkerStatus(text) {
  markerStatus.textContent = text || "";
}

function markerInView(marker) {
  return marker.t >= viewStart && marker.t <= viewEnd;
}

function panViewToMarker(marker) {
  if (marker.t < viewStart) {
    const delta = marker.t - viewStart;
    viewStart += delta;
    viewEnd += delta;
  } else if (marker.t > viewEnd) {
    const delta = marker.t - viewEnd;
    viewStart += delta;
    viewEnd += delta;
  }
  clampView();
}

function focusMarker(markerId, options = {}) {
  const { ensureVisible = false, shouldRender = true } = options;
  const marker = markerById(markerId);
  if (!marker) {
    if (activeMarkerId !== null) {
      activeMarkerId = null;
      if (shouldRender) {
        render();
      }
    }
    return false;
  }
  activeMarkerId = marker.id;
  if (ensureVisible && !markerInView(marker)) {
    panViewToMarker(marker);
  }
  setMarkerStatus("");
  if (shouldRender) {
    render();
  }
  return true;
}

function clearActiveMarker(options = {}) {
  const { shouldRender = true } = options;
  if (activeMarkerId === null) {
    return;
  }
  activeMarkerId = null;
  if (shouldRender) {
    render();
  }
}

function addMarker(timeNs, label) {
  const markerId = nextMarkerId;
  markers.push({
    id: markerId,
    t: clampTimeNs(timeNs),
    label: label ? label.trim() || null : null,
  });
  nextMarkerId += 1;
  sortMarkers();
  setMarkerStatus("");
  focusMarker(markerId, { ensureVisible: true });
}

function updateMarkerTime(markerId, timeNs) {
  const marker = markerById(markerId);
  if (!marker) {
    return;
  }
  marker.t = clampTimeNs(timeNs);
  sortMarkers();
  render();
}

function removeMarker(markerId) {
  markers = markers.filter(marker => marker.id !== markerId);
  if (activeMarkerId === markerId) {
    activeMarkerId = null;
  }
  setMarkerStatus("");
  render();
}

function renderMarkerStrip() {
  markerStrip.innerHTML = "";
  if (!markers.length) {
    const empty = document.createElement("span");
    empty.className = "legend";
    empty.textContent = "No markers";
    markerStrip.appendChild(empty);
    return;
  }
  for (const marker of markers) {
    const chip = document.createElement("div");
    chip.className = marker.id === activeMarkerId ? "marker-chip active" : "marker-chip";
    chip.style.setProperty("--marker-color", markerColor(marker));
    chip.title = markerTitle(marker);
    chip.addEventListener("mousedown", event => {
      event.preventDefault();
      event.stopPropagation();
      focusMarker(marker.id, { ensureVisible: true });
    });

    const swatch = document.createElement("span");
    swatch.className = "marker-swatch";
    chip.appendChild(swatch);

    const text = document.createElement("span");
    text.textContent = marker.label
      ? `${marker.label} @ ${formatMarkerTime(marker.t)}`
      : formatMarkerTime(marker.t);
    chip.appendChild(text);

    const removeButton = document.createElement("button");
    removeButton.type = "button";
    removeButton.className = "marker-remove";
    removeButton.textContent = "x";
    removeButton.addEventListener("mousedown", event => {
      event.preventDefault();
      event.stopPropagation();
      removeMarker(marker.id);
    });
    chip.appendChild(removeButton);
    markerStrip.appendChild(chip);
  }
}

function parseMarkerTime(text) {
  const match = text.trim().match(/^([0-9]+(?:\.[0-9]+)?)\s*(ns|us|ms|s)?$/i);
  if (!match) {
    return null;
  }
  const value = Number(match[1]);
  if (!Number.isFinite(value)) {
    return null;
  }
  const unit = (match[2] || "ns").toLowerCase();
  const scale = unit === "s" ? 1_000_000_000 : unit === "ms" ? 1_000_000 : unit === "us" ? 1_000 : 1;
  return clampTimeNs(value * scale);
}

function addMarkerFromInputs(timeNs) {
  const resolvedTime = timeNs ?? parseMarkerTime(markerTimeInput.value);
  if (resolvedTime === null) {
    setMarkerStatus("Invalid marker time");
    return;
  }
  addMarker(resolvedTime, markerLabelInput.value);
}

function coverageRect() {
  return coverageInfo.getBoundingClientRect();
}

function coverageRatioAtClientX(clientX) {
  const rect = coverageRect();
  return Math.max(0, Math.min(1, (clientX - rect.left) / Math.max(1, rect.width)));
}

function minViewSpanRatio() {
  return minViewSpanNs() / totalSpanNs();
}

function setViewFromRatios(startRatio, endRatio) {
  const total = totalSpanNs();
  viewStart = data.start_ns + startRatio * total;
  viewEnd = data.start_ns + endRatio * total;
  clampView();
  render();
}

function viewSpanNs() {
  return Math.max(1, viewEnd - viewStart);
}

function minViewSpanNs() {
  return Math.max(50, totalSpanNs() / 1_000_000);
}

function minCoverageWindowRatio() {
  const rect = coverageRect();
  return Math.max(1 / Math.max(1, rect.width), minViewSpanNs() / totalSpanNs());
}

function clampUnit(value) {
  return Math.max(0, Math.min(1, value));
}

function setViewFromAnchor(anchorNs, anchorRatio, spanNs) {
  const clampedRatio = clampUnit(anchorRatio);
  const safeSpan = Math.max(minViewSpanNs(), Math.min(totalSpanNs(), spanNs));
  viewStart = anchorNs - safeSpan * clampedRatio;
  viewEnd = viewStart + safeSpan;
  clampView();
  render();
}

function panViewByNs(deltaNs) {
  viewStart += deltaNs;
  viewEnd += deltaNs;
  clampView();
  render();
}

function wheelDeltaScale(deltaMode) {
  if (deltaMode === 1) {
    return 16;
  }
  if (deltaMode === 2) {
    return Math.max(window.innerHeight, 1);
  }
  return 1;
}

function primaryWheelDelta(event) {
  const scale = wheelDeltaScale(event.deltaMode);
  const deltaX = event.deltaX * scale;
  const deltaY = event.deltaY * scale;
  return Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
}

function wheelZoomFactor(event) {
  const delta = primaryWheelDelta(event);
  const steps = Math.max(1, Math.abs(delta) / 120);
  return delta < 0 ? Math.pow(0.85, steps) : Math.pow(1 / 0.85, steps);
}

function panViewFromWheel(event, referenceWidth) {
  const delta = primaryWheelDelta(event);
  const safeWidth = Math.max(referenceWidth, 240);
  const deltaNs = viewSpanNs() * delta / safeWidth;
  panViewByNs(deltaNs);
}

function zoomViewAroundCenter(event) {
  const centerNs = (viewStart + viewEnd) / 2;
  setViewFromAnchor(centerNs, 0.5, viewSpanNs() * wheelZoomFactor(event));
}

function viewerWaveMetrics() {
  const rect = canvas.getBoundingClientRect();
  const waveLeft = SIGNAL_LABEL_WIDTH;
  const waveWidth = Math.max(1, rect.width - waveLeft);
  return { rect, waveLeft, waveWidth };
}

function viewerRatioAtClientX(clientX) {
  const { rect, waveLeft, waveWidth } = viewerWaveMetrics();
  return clampUnit((clientX - rect.left - waveLeft) / waveWidth);
}

function zoomViewAroundViewerCursor(event) {
  const ratio = viewerRatioAtClientX(event.clientX);
  const anchorNs = viewStart + viewSpanNs() * ratio;
  setViewFromAnchor(anchorNs, ratio, viewSpanNs() * wheelZoomFactor(event));
}

function handleViewerWheelGesture(event) {
  if (event.ctrlKey) {
    zoomViewAroundViewerCursor(event);
    return true;
  }
  if (event.shiftKey) {
    panViewFromWheel(event, viewerWaveMetrics().waveWidth);
    return true;
  }
  if (event.altKey) {
    zoomViewAroundCenter(event);
    return true;
  }
  return false;
}

function visibleSignals() {
  const query = searchQuery();
  return signalOrder
    .map(id => signalById.get(id))
    .filter(signal => {
      const matches = matchesQuery(signal, query);
      if (!query) {
        return selected.has(signal.id);
      }
      return matches;
    });
}

function reorderSignal(sourceId, sourceIndex, targetIndex, visibleIds) {
  const filteredOrder = signalOrder.filter(id => id !== sourceId);
  const remainingVisible = visibleIds.filter(id => id !== sourceId);
  const safeIndex = normalizeReorderIndex(sourceIndex, targetIndex, visibleIds.length);
  const beforeId = remainingVisible[safeIndex] || null;
  if (beforeId) {
    const insertIndex = filteredOrder.indexOf(beforeId);
    filteredOrder.splice(insertIndex, 0, sourceId);
    signalOrder = filteredOrder;
    return;
  }
  const afterId = remainingVisible[safeIndex - 1] || null;
  if (afterId) {
    const insertIndex = filteredOrder.indexOf(afterId);
    filteredOrder.splice(insertIndex + 1, 0, sourceId);
    signalOrder = filteredOrder;
    return;
  }
  filteredOrder.unshift(sourceId);
  signalOrder = filteredOrder;
}

function normalizeReorderIndex(sourceIndex, targetIndex, visibleCount) {
  const normalizedIndex = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
  const remainingVisibleCount = Math.max(0, visibleCount - 1);
  return Math.max(0, Math.min(normalizedIndex, remainingVisibleCount));
}

function reorderWouldChange(sourceIndex, targetIndex, visibleCount) {
  return normalizeReorderIndex(sourceIndex, targetIndex, visibleCount) !== sourceIndex;
}

function reorderTargetIndexAt(logicalY) {
  if (!rowLayout.length) {
    return 0;
  }
  for (const row of rowLayout) {
    if (logicalY < row.top + row.height / 2) {
      return row.index;
    }
  }
  return rowLayout.length;
}

function insertionLineY(targetIndex) {
  if (!rowLayout.length) {
    return 24;
  }
  if (targetIndex <= 0) {
    return rowLayout[0].top;
  }
  if (targetIndex >= rowLayout.length) {
    return rowLayout[rowLayout.length - 1].bottom;
  }
  return rowLayout[targetIndex].top;
}

function rowHandleRect(rowTop, rowHeight) {
  const left = 8;
  const width = 16;
  const height = 18;
  const top = rowTop + (rowHeight - height) / 2;
  return {
    left,
    right: left + width,
    top,
    bottom: top + height,
  };
}

function rowHandleAt(logicalX, logicalY) {
  return rowLayout.find(row => {
    if (!row.handle) {
      return false;
    }
    return (
      logicalX >= row.handle.left &&
      logicalX <= row.handle.right &&
      logicalY >= row.handle.top &&
      logicalY <= row.handle.bottom
    );
  }) || null;
}

function labelTextX() {
  return 34;
}

function labelMetaX() {
  return 34;
}

function actionRowHit(logicalX, logicalY) {
  return rowLayout.find(row => {
    if (!row.action) {
      return false;
    }
    return (
      logicalX >= row.action.left &&
      logicalX <= row.action.right &&
      logicalY >= row.action.top &&
      logicalY <= row.action.bottom
    );
  }) || null;
}

function termMatchesQuery(term, query) {
  if (!term) {
    return false;
  }
  if (term === query) {
    return true;
  }
  const tokens = term.split(/[^a-z0-9]+/).filter(Boolean);
  if (query.length <= 2) {
    return tokens.some(token => token === query);
  }
  if (term.startsWith(query)) {
    return true;
  }
  if (tokens.some(token => token === query || token.startsWith(query))) {
    return true;
  }
  return query.length >= 3 && term.includes(query);
}

function matchesQuery(signal, query) {
  if (!query) {
    return true;
  }
  const searchTerms = [
    signal.label.toLowerCase(),
    signal.id.toLowerCase(),
    signal.category.toLowerCase(),
    signal.group.toLowerCase(),
    ...signal.aliases,
  ];
  return searchTerms.some(term => termMatchesQuery(term, query));
}

function buildSidebar() {
  sidebar.innerHTML = "";
  const query = searchQuery();
  for (const { category, group, signals } of [...groups.values()].sort((a, b) => {
    return `${a.category}/${a.group}`.localeCompare(`${b.category}/${b.group}`);
  })) {
    const filteredSignals = signals.filter(signal => matchesQuery(signal, query));
    if (!filteredSignals.length) {
      continue;
    }
    const block = document.createElement("section");
    block.className = "group";

    const header = document.createElement("div");
    header.className = "group-header";

    const title = document.createElement("span");
    title.textContent = `${category} / ${group}`;
    header.appendChild(title);

    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.textContent = "toggle";
    toggle.addEventListener("click", () => {
      const anyOff = signals.some(signal => !selected.has(signal.id));
      for (const signal of signals) {
        if (anyOff) {
          selected.add(signal.id);
        } else {
          selected.delete(signal.id);
        }
      }
      buildSidebar();
      render();
    });
    header.appendChild(toggle);
    block.appendChild(header);

    const body = document.createElement("div");
    body.className = "group-body";
    for (const signal of filteredSignals) {
      const label = document.createElement("label");
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = selected.has(signal.id);
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) {
          selected.add(signal.id);
        } else {
          selected.delete(signal.id);
        }
        render();
      });
      label.appendChild(checkbox);
      const text = document.createElement("span");
      text.textContent = signal.label;
      label.appendChild(text);
      body.appendChild(label);
    }
    block.appendChild(body);
    sidebar.appendChild(block);
  }
}

function pointValueText(signal, value) {
  if (signal.kind === "digital") {
    return value ? "1" : "0";
  }
  if (signal.format === "hex8") {
    return `0x${Number(value).toString(16).toUpperCase().padStart(2, "0")}`;
  }
  if (signal.format === "float") {
    return `${Number(value).toFixed(3)}${signal.unit ? ` ${signal.unit}` : ""}`;
  }
  return String(value);
}

function chooseTimeUnit(referenceNs) {
  if (referenceNs >= 1_000_000_000) {
    return { suffix: "s", scale: 1_000_000_000 };
  }
  if (referenceNs >= 1_000_000) {
    return { suffix: "ms", scale: 1_000_000 };
  }
  if (referenceNs >= 1_000) {
    return { suffix: "us", scale: 1_000 };
  }
  return { suffix: "ns", scale: 1 };
}

function decimalsForStep(stepNs, scale) {
  if (scale === 1) {
    return 0;
  }
  const stepInUnit = Math.abs(stepNs / scale);
  if (stepInUnit >= 1) {
    return 0;
  }
  if (stepInUnit >= 0.1) {
    return 1;
  }
  if (stepInUnit >= 0.01) {
    return 2;
  }
  if (stepInUnit >= 0.001) {
    return 3;
  }
  return 4;
}

function formatTimeNs(timeNs, stepNs, unit) {
  const resolvedUnit = unit || chooseTimeUnit(Math.max(1, Math.abs(stepNs)));
  const decimals = decimalsForStep(stepNs, resolvedUnit.scale);
  return `${(timeNs / resolvedUnit.scale).toFixed(decimals)} ${resolvedUnit.suffix}`;
}

function formatPercent(ratio) {
  return `${(ratio * 100).toFixed(1)}%`;
}

function measureCoverageRangeLabel(node, text) {
  node.textContent = text;
  node.style.visibility = "hidden";
  node.style.left = "0px";
  node.classList.remove("inside");
  return node.offsetWidth;
}

function niceTimeStepNs(targetNs) {
  const safeTarget = Math.max(1, targetNs);
  const exponent = Math.floor(Math.log10(safeTarget));
  const base = 10 ** exponent;
  for (const multiplier of [1, 2, 5, 10]) {
    const step = multiplier * base;
    if (step >= safeTarget) {
      return step;
    }
  }
  return 10 * base;
}

function buildGridMarks(waveWidth) {
  const span = Math.max(1, viewEnd - viewStart);
  const targetCount = Math.max(4, Math.round(waveWidth / 160));
  const stepNs = niceTimeStepNs(span / targetCount);
  const startTick = Math.ceil(viewStart / stepNs);
  const endTick = Math.floor(viewEnd / stepNs);
  const marks = [];
  for (let tick = startTick; tick <= endTick; tick += 1) {
    marks.push(tick * stepNs);
    if (marks.length >= 256) {
      break;
    }
  }
  return { marks, stepNs };
}

function sampleArray(signal) {
  return samples[signal.id] || [];
}

function segmentAt(points, timeNs) {
  if (!points.length) {
    return null;
  }
  let current = points[0];
  for (const point of points) {
    if (point.t > timeNs) {
      break;
    }
    current = point;
  }
  return current;
}

function xOf(timeNs, waveLeft, waveWidth) {
  const span = Math.max(1, viewEnd - viewStart);
  return waveLeft + (timeNs - viewStart) * waveWidth / span;
}

function hashText(text) {
  let hash = 2166136261;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function eventColors(event) {
  const label = event.label.toUpperCase();
  const palette = [
    { line: "#38bdf8", text: "#e0f2fe" },
    { line: "#818cf8", text: "#e0e7ff" },
    { line: "#c084fc", text: "#f3e8ff" },
    { line: "#f472b6", text: "#fce7f3" },
    { line: "#f97316", text: "#ffedd5" },
    { line: "#facc15", text: "#fef9c3" },
    { line: "#34d399", text: "#d1fae5" },
    { line: "#2dd4bf", text: "#ccfbf1" },
  ];
  if (label.startsWith("START")) {
    return { line: "#67e8f9", text: "#cffafe" };
  }
  if (label.startsWith("REPEATED START")) {
    return { line: "#22d3ee", text: "#a5f3fc" };
  }
  if (label.startsWith("STOP")) {
    return { line: "#f87171", text: "#fecaca" };
  }
  if (label.startsWith("ACK")) {
    return { line: "#4ade80", text: "#dcfce7" };
  }
  if (label.startsWith("NACK")) {
    return { line: "#fb7185", text: "#ffe4e6" };
  }
  if (label.startsWith("ADDR")) {
    return { line: "#60a5fa", text: "#dbeafe" };
  }
  if (label.startsWith("TX")) {
    return { line: "#f59e0b", text: "#fef3c7" };
  }
  if (label.startsWith("RX")) {
    return { line: "#a78bfa", text: "#ede9fe" };
  }
  if (label === "INT0 ENTER") {
    return { line: "#38bdf8", text: "#e0f2fe" };
  }
  if (label === "T0 ENTER") {
    return { line: "#f97316", text: "#ffedd5" };
  }
  if (label === "INT1 ENTER") {
    return { line: "#a78bfa", text: "#ede9fe" };
  }
  if (label === "T1 ENTER") {
    return { line: "#34d399", text: "#d1fae5" };
  }
  if (label === "UART ENTER") {
    return { line: "#f472b6", text: "#fce7f3" };
  }
  if (label.includes("ENTER")) {
    return palette[hashText(`${event.track_id}:${event.label}`) % palette.length];
  }
  return palette[hashText(`${event.track_id}:${event.label}`) % palette.length];
}

function markerColor(marker) {
  const palette = [
    "#facc15",
    "#fb7185",
    "#60a5fa",
    "#4ade80",
    "#c084fc",
    "#2dd4bf",
    "#f97316",
    "#f472b6",
  ];
  const seed = marker.label ? `${marker.label}:${marker.t}` : `marker:${marker.t}:${marker.id}`;
  return palette[hashText(seed) % palette.length];
}

function markerTitle(marker) {
  const timeText = formatMarkerTime(marker.t);
  return marker.label ? `${marker.label} @ ${timeText}` : timeText;
}

function renderCoverageMarkers() {
  coverageMarkers.innerHTML = "";
  if (!markers.length) {
    return;
  }
  const total = totalSpanNs();
  for (const marker of markers) {
    if (marker.t < data.start_ns || marker.t > data.end_ns) {
      continue;
    }
    const ratio = clampUnit((marker.t - data.start_ns) / total);
    const node = document.createElement("div");
    node.className = marker.id === activeMarkerId ? "coverage-marker active" : "coverage-marker";
    node.style.left = `${ratio * 100}%`;
    node.style.setProperty("--marker-color", markerColor(marker));
    node.title = markerTitle(marker);
    node.setAttribute("aria-label", markerTitle(marker));
    node.addEventListener("mousedown", event => {
      event.preventDefault();
      event.stopPropagation();
      if (activeMarkerId !== marker.id) {
        focusMarker(marker.id, { ensureVisible: true });
        return;
      }
      focusMarker(marker.id, { shouldRender: false });
      dragState = {
        kind: "marker-move",
        markerId: marker.id,
        source: "coverage",
      };
      render();
    });
    coverageMarkers.appendChild(node);
  }
}

function renderCoverageRangeLabels(startPx, endPx, trackWidth, stepNs, timeUnit) {
  const startText = formatTimeNs(viewStart, stepNs, timeUnit);
  const endText = formatTimeNs(viewEnd, stepNs, timeUnit);
  const gap = 8;
  const trackRight = Math.max(1, trackWidth);
  const rowWidth = coverageInfo.parentElement.getBoundingClientRect().width;
  const leftOutsideLimit = -Math.max(0, coverageInfo.offsetLeft - 4);
  const rightOutsideLimit =
    trackRight + Math.max(0, rowWidth - coverageInfo.offsetLeft - trackRight - 4);
  const startWidth = measureCoverageRangeLabel(coverageRangeStart, startText);
  const endWidth = measureCoverageRangeLabel(coverageRangeEnd, endText);
  let startLeft = startPx - gap - startWidth;
  let endLeft = endPx + gap;
  let startInside = false;
  let endInside = false;

  if (startLeft < leftOutsideLimit) {
    startLeft = Math.max(4, Math.min(startPx + gap, trackRight - startWidth - 4));
    startInside = true;
  }
  if (endLeft + endWidth > rightOutsideLimit) {
    endLeft = Math.max(4, Math.min(endPx - gap - endWidth, trackRight - endWidth - 4));
    endInside = true;
  }
  if (startLeft + startWidth + gap > endLeft) {
    if (!endInside) {
      endLeft = Math.max(4, Math.min(endPx - gap - endWidth, trackRight - endWidth - 4));
      endInside = true;
    } else if (!startInside) {
      startLeft = Math.max(4, Math.min(startPx + gap, trackRight - startWidth - 4));
      startInside = true;
    } else {
      startLeft = Math.max(leftOutsideLimit, startPx - gap - startWidth);
      endLeft = Math.min(rightOutsideLimit - endWidth, endPx + gap);
      startInside = false;
      endInside = false;
    }
  }
  if (startLeft + startWidth + gap > endLeft) {
    startLeft = Math.max(leftOutsideLimit, 4 - startWidth);
    endLeft = Math.min(rightOutsideLimit - endWidth, trackRight + 4);
    startInside = false;
    endInside = false;
  }

  coverageRangeStart.style.left = `${startLeft}px`;
  coverageRangeEnd.style.left = `${endLeft}px`;
  coverageRangeStart.classList.toggle("inside", startInside);
  coverageRangeEnd.classList.toggle("inside", endInside);
  coverageRangeStart.style.visibility = "visible";
  coverageRangeEnd.style.visibility = "visible";
}

function renderWaveMarkers(leftLabel, waveWidth, top, height) {
  const labelTop = 4;
  for (const marker of markers) {
    if (marker.t < viewStart || marker.t > viewEnd) {
      continue;
    }
    const x = xOf(marker.t, leftLabel, waveWidth);
    const color = markerColor(marker);
    ctx.save();
    ctx.setLineDash(marker.id === activeMarkerId ? [7, 4] : [4, 4]);
    ctx.strokeStyle = color;
    ctx.lineWidth = marker.id === activeMarkerId ? 2 : 1;
    ctx.beginPath();
    ctx.moveTo(x, top - 8);
    ctx.lineTo(x, height);
    ctx.stroke();
    ctx.setLineDash([]);
    const label = marker.label ? `${marker.label} @ ${formatMarkerTime(marker.t)}` : formatMarkerTime(marker.t);
    const boxWidth = Math.max(54, ctx.measureText(label).width + 10);
    const boxLeft = Math.max(
      leftLabel + 4,
      Math.min(x - boxWidth / 2, leftLabel + waveWidth - boxWidth - 4)
    );
    ctx.fillStyle = marker.id === activeMarkerId ? "#1d2946" : "#15203a";
    ctx.fillRect(boxLeft, labelTop, boxWidth, 18);
    ctx.strokeStyle = color;
    ctx.strokeRect(boxLeft, labelTop, boxWidth, 18);
    ctx.fillStyle = "#eef2ff";
    ctx.fillText(label, boxLeft + 5, labelTop + 13);
    ctx.restore();
  }
}

function renderTimeRuler(width, leftLabel, waveWidth, grid, timeUnit) {
  rulerCanvas.width = width * window.devicePixelRatio;
  rulerCanvas.height = VIEWER_RULER_HEIGHT * window.devicePixelRatio;
  rulerCanvas.style.width = `${width}px`;
  rulerCanvas.style.height = `${VIEWER_RULER_HEIGHT}px`;
  rulerCtx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);
  rulerCtx.clearRect(0, 0, width, VIEWER_RULER_HEIGHT);
  rulerCtx.fillStyle = "#0d1324";
  rulerCtx.fillRect(0, 0, width, VIEWER_RULER_HEIGHT);
  rulerCtx.fillStyle = "#11182c";
  rulerCtx.fillRect(0, 0, leftLabel, VIEWER_RULER_HEIGHT);
  rulerCtx.strokeStyle = "#273153";
  rulerCtx.beginPath();
  rulerCtx.moveTo(leftLabel + 0.5, 0);
  rulerCtx.lineTo(leftLabel + 0.5, VIEWER_RULER_HEIGHT);
  rulerCtx.moveTo(0, VIEWER_RULER_HEIGHT - 0.5);
  rulerCtx.lineTo(width, VIEWER_RULER_HEIGHT - 0.5);
  rulerCtx.stroke();
  rulerCtx.font = "12px ui-monospace, monospace";
  rulerCtx.textAlign = "center";
  rulerCtx.textBaseline = "middle";
  for (const time of grid.marks) {
    const x = xOf(time, leftLabel, waveWidth);
    rulerCtx.strokeStyle = "#39456b";
    rulerCtx.beginPath();
    rulerCtx.moveTo(x + 0.5, VIEWER_RULER_HEIGHT - 13);
    rulerCtx.lineTo(x + 0.5, VIEWER_RULER_HEIGHT - 1);
    rulerCtx.stroke();
    rulerCtx.fillStyle = "#9aa4d6";
    rulerCtx.fillText(formatTimeNs(time, grid.stepNs, timeUnit), x, 12);
  }
  if (!hoverState) {
    return;
  }
  const timeNs = clampTimeNs(hoverState.timeNs);
  const x = xOf(timeNs, leftLabel, waveWidth);
  const text = formatTimeNs(timeNs, grid.stepNs, timeUnit);
  const textWidth = rulerCtx.measureText(text).width;
  const boxWidth = Math.max(52, textWidth + 10);
  const boxLeft = Math.max(
    leftLabel + 4,
    Math.min(x - boxWidth / 2, leftLabel + waveWidth - boxWidth - 4)
  );
  rulerCtx.strokeStyle = "#ffd166";
  rulerCtx.beginPath();
  rulerCtx.moveTo(x + 0.5, 0);
  rulerCtx.lineTo(x + 0.5, VIEWER_RULER_HEIGHT);
  rulerCtx.stroke();
  rulerCtx.fillStyle = "#182234";
  rulerCtx.fillRect(boxLeft, VIEWER_RULER_HEIGHT - 22, boxWidth, 16);
  rulerCtx.strokeStyle = "#ffd166";
  rulerCtx.strokeRect(boxLeft + 0.5, VIEWER_RULER_HEIGHT - 21.5, boxWidth - 1, 15);
  rulerCtx.fillStyle = "#fff1be";
  rulerCtx.fillText(text, boxLeft + boxWidth / 2, VIEWER_RULER_HEIGHT - 14);
}

function timeAtViewerLogicalX(logicalX, logicalWidth) {
  const waveLeft = SIGNAL_LABEL_WIDTH;
  const waveWidth = Math.max(1, logicalWidth - waveLeft);
  const ratio = clampUnit((logicalX - waveLeft) / waveWidth);
  return clampTimeNs(viewStart + viewSpanNs() * ratio);
}

function markerAtViewerLogicalPoint(logicalX, logicalY, logicalWidth, logicalHeight) {
  if (logicalY < 0 || logicalY > logicalHeight) {
    return null;
  }
  const waveLeft = SIGNAL_LABEL_WIDTH;
  const waveWidth = Math.max(1, logicalWidth - waveLeft);
  let best = null;
  let bestDistance = 7;
  for (const marker of markers) {
    if (marker.t < viewStart || marker.t > viewEnd) {
      continue;
    }
    const x = xOf(marker.t, waveLeft, waveWidth);
    const distance = Math.abs(x - logicalX);
    if (distance <= bestDistance) {
      best = marker;
      bestDistance = distance;
    }
  }
  return best;
}

function viewerMarkerAtClientPoint(clientX, clientY) {
  const rect = canvas.getBoundingClientRect();
  const logicalWidth = canvas.width / window.devicePixelRatio;
  const logicalHeight = canvas.height / window.devicePixelRatio;
  const logicalX = (clientX - rect.left) * canvas.width / Math.max(1, rect.width) / window.devicePixelRatio;
  const logicalY = (clientY - rect.top) * canvas.height / Math.max(1, rect.height) / window.devicePixelRatio;
  return markerAtViewerLogicalPoint(logicalX, logicalY, logicalWidth, logicalHeight);
}

function eventPathContainsSelector(event, selector) {
  return event.composedPath().some(node => node instanceof Element && node.closest(selector));
}

function shouldKeepMarkerFocusOnMouseDown(event) {
  if (eventPathContainsSelector(event, ".marker-chip") || eventPathContainsSelector(event, ".coverage-marker")) {
    return true;
  }
  if (event.composedPath().includes(canvas)) {
    return Boolean(viewerMarkerAtClientPoint(event.clientX, event.clientY));
  }
  return false;
}

function renderDigital(signal, rowTop, rowHeight, waveLeft, waveWidth) {
  const points = sampleArray(signal);
  if (!points.length) {
    return;
  }
  const highY = rowTop + 16;
  const lowY = rowTop + rowHeight - 12;
  const startPoint = segmentAt(points, viewStart) || points[0];
  ctx.strokeStyle = "#83a8ff";
  ctx.lineWidth = 2;
  ctx.beginPath();
  let currentValue = Boolean(startPoint.v);
  let currentX = waveLeft;
  ctx.moveTo(currentX, currentValue ? highY : lowY);
  for (const point of points) {
    if (point.t < viewStart || point.t > viewEnd) {
      continue;
    }
    const x = xOf(point.t, waveLeft, waveWidth);
    ctx.lineTo(x, currentValue ? highY : lowY);
    currentValue = Boolean(point.v);
    ctx.lineTo(x, currentValue ? highY : lowY);
    currentX = x;
  }
  ctx.lineTo(waveLeft + waveWidth, currentValue ? highY : lowY);
  ctx.stroke();
}

function renderBusLike(signal, rowTop, rowHeight, waveLeft, waveWidth) {
  const points = sampleArray(signal);
  if (!points.length) {
    return;
  }
  const visible = [];
  const startPoint = segmentAt(points, viewStart) || points[0];
  visible.push({ t: viewStart, v: startPoint.v });
  for (const point of points) {
    if (point.t >= viewStart && point.t <= viewEnd) {
      visible.push(point);
    }
  }
  for (let index = 0; index < visible.length; index += 1) {
    const point = visible[index];
    const nextTime = index + 1 < visible.length ? visible[index + 1].t : viewEnd;
    const left = xOf(point.t, waveLeft, waveWidth);
    const right = xOf(nextTime, waveLeft, waveWidth);
    const width = Math.max(1, right - left);
    ctx.fillStyle = signal.kind === "text" ? "#27405f" : "#1d3557";
    ctx.fillRect(left, rowTop + 8, width, rowHeight - 16);
    ctx.strokeStyle = "#7fb8ff";
    ctx.strokeRect(left, rowTop + 8, width, rowHeight - 16);
    const text = pointValueText(signal, point.v);
    if (width > 30) {
      ctx.fillStyle = "#eef3ff";
      ctx.fillText(text, left + 6, rowTop + rowHeight / 2 + 4);
    }
  }
}

function renderAnalog(signal, rowTop, rowHeight, waveLeft, waveWidth) {
  const points = sampleArray(signal);
  if (!points.length) {
    return;
  }
  const visible = [];
  const startPoint = segmentAt(points, viewStart) || points[0];
  visible.push({ t: viewStart, v: Number(startPoint.v) });
  for (const point of points) {
    if (point.t >= viewStart && point.t <= viewEnd) {
      visible.push({ t: point.t, v: Number(point.v) });
    }
  }
  let min = visible[0].v;
  let max = visible[0].v;
  for (const point of visible) {
    min = Math.min(min, point.v);
    max = Math.max(max, point.v);
  }
  if (min === max) {
    min -= 1;
    max += 1;
  }
  ctx.strokeStyle = "#8affc1";
  ctx.lineWidth = 2;
  ctx.beginPath();
  for (let index = 0; index < visible.length; index += 1) {
    const point = visible[index];
    const x = xOf(point.t, waveLeft, waveWidth);
    const ratio = (point.v - min) / (max - min);
    const y = rowTop + rowHeight - 12 - ratio * (rowHeight - 24);
    if (index === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.stroke();
  ctx.fillStyle = "#9aa4d6";
  ctx.fillText(`${min.toFixed(3)} .. ${max.toFixed(3)}${signal.unit ? ` ${signal.unit}` : ""}`, waveLeft + 8, rowTop + 14);
}

function renderEvent(signal, rowTop, rowHeight, waveLeft, waveWidth) {
  const trackEvents = eventsByTrack.get(signal.id) || [];
  const laneBaselines = [
    rowTop + 16,
    rowTop + rowHeight - 10,
    rowTop + Math.round(rowHeight / 2) + 4,
  ];
  const laneRightEdges = laneBaselines.map(() => Number.NEGATIVE_INFINITY);
  const labelGap = 12;
  for (const event of trackEvents) {
    if (event.t < viewStart || event.t > viewEnd) {
      continue;
    }
    const x = xOf(event.t, waveLeft, waveWidth);
    const colors = eventColors(event);
    ctx.strokeStyle = colors.line;
    ctx.beginPath();
    ctx.moveTo(x, rowTop + 6);
    ctx.lineTo(x, rowTop + rowHeight - 6);
    ctx.stroke();
    ctx.fillStyle = colors.text;
    const labelX = x + 4;
    const labelWidth = ctx.measureText(event.label).width;
    let laneIndex = laneRightEdges.findIndex(rightEdge => labelX >= rightEdge + labelGap);
    if (laneIndex === -1) {
      laneIndex = 0;
      for (let index = 1; index < laneRightEdges.length; index += 1) {
        if (laneRightEdges[index] < laneRightEdges[laneIndex]) {
          laneIndex = index;
        }
      }
    }
    laneRightEdges[laneIndex] = labelX + labelWidth;
    ctx.fillText(event.label, labelX, laneBaselines[laneIndex]);
  }
}

function valueAt(signal, timeNs) {
  const point = segmentAt(sampleArray(signal), timeNs);
  return point ? point.v : null;
}

function nearestEvent(signal, timeNs, toleranceNs) {
  const trackEvents = eventsByTrack.get(signal.id) || [];
  let best = null;
  let bestDelta = toleranceNs;
  for (const event of trackEvents) {
    const delta = Math.abs(event.t - timeNs);
    if (delta <= bestDelta) {
      bestDelta = delta;
      best = event;
    }
  }
  return best;
}

function rowActionRect(rowTop, rowHeight, leftLabel) {
  const size = 16;
  const right = leftLabel - 16;
  const left = right - size;
  const top = rowTop + 10;
  return {
    left,
    right,
    top,
    bottom: top + size,
  };
}

function renderCursor(rowHeight, top, leftLabel, waveWidth, height) {
  if (!hoverState) {
    cursorInfo.textContent = "";
    return;
  }
  const span = Math.max(1, viewEnd - viewStart);
  const referenceStepNs = niceTimeStepNs(span / Math.max(4, Math.round(waveWidth / 160)));
  const timeUnit = chooseTimeUnit(referenceStepNs);
  const timeNs = hoverState.timeNs;
  const x = xOf(timeNs, leftLabel, waveWidth);
  ctx.strokeStyle = "#ffd166";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(x, top - 8);
  ctx.lineTo(x, height);
  ctx.stroke();

  const row = rowLayout.find(item => hoverState.y >= item.top && hoverState.y < item.bottom);
  if (!row) {
    cursorInfo.textContent = `t=${formatTimeNs(timeNs, referenceStepNs, timeUnit)}`;
    return;
  }

  if (row.signal.kind === "event") {
    const toleranceNs = Math.max(1, span / Math.max(200, waveWidth));
    const event = nearestEvent(row.signal, timeNs, toleranceNs);
    if (event) {
      cursorInfo.textContent = event.detail
        ? `${row.signal.label} @ ${formatTimeNs(event.t, referenceStepNs, timeUnit)}: ${event.label} (${event.detail})`
        : `${row.signal.label} @ ${formatTimeNs(event.t, referenceStepNs, timeUnit)}: ${event.label}`;
      return;
    }
    cursorInfo.textContent = `${row.signal.label} @ ${formatTimeNs(timeNs, referenceStepNs, timeUnit)}`;
    return;
  }

  const value = valueAt(row.signal, timeNs);
  if (value === null) {
    cursorInfo.textContent = `${row.signal.label} @ ${formatTimeNs(timeNs, referenceStepNs, timeUnit)}: n/a`;
    return;
  }
  cursorInfo.textContent = `${row.signal.label} @ ${formatTimeNs(timeNs, referenceStepNs, timeUnit)}: ${pointValueText(row.signal, value)}`;
}

function render() {
  const visible = visibleSignals();
  const visibleIds = visible.map(signal => signal.id);
  const visibleKey = visibleIds.join("\n");
  const query = searchQuery();
  const rowHeight = VIEWER_ROW_HEIGHT;
  const top = VIEWER_TOP_PADDING;
  const leftLabel = SIGNAL_LABEL_WIDTH;
  const viewerPaddingX = VIEWER_SIDE_PADDING_X;
  const width = Math.max(leftLabel + 1, viewer.clientWidth - viewerPaddingX);
  const waveWidth = Math.max(1, width - leftLabel);
  const height = top + visible.length * rowHeight + 30;
  const grid = buildGridMarks(waveWidth);
  const timeUnit = chooseTimeUnit(grid.stepNs);
  renderTimeRuler(width, leftLabel, waveWidth, grid, timeUnit);
  canvas.width = width * window.devicePixelRatio;
  canvas.height = height * window.devicePixelRatio;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  ctx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = "#0b0f1d";
  ctx.fillRect(0, 0, width, height);
  ctx.font = "12px ui-monospace, monospace";
  rowLayout = [];
  const totalSpan = Math.max(1, data.end_ns - data.start_ns);
  const startRatio = Math.max(0, Math.min(1, (viewStart - data.start_ns) / totalSpan));
  const endRatio = Math.max(0, Math.min(1, (viewEnd - data.start_ns) / totalSpan));
  const spanRatio = Math.max(0, Math.min(1, (viewEnd - viewStart) / totalSpan));
  const renderedSpanRatio = Math.max(spanRatio, minCoverageWindowRatio());
  const startPx = startRatio * coverageRect().width;
  const endPx = startPx + renderedSpanRatio * coverageRect().width;
  coverageWindow.style.left = `${startRatio * 100}%`;
  coverageWindow.style.width = `${renderedSpanRatio * 100}%`;
  renderCoverageRangeLabels(startPx, endPx, coverageRect().width, grid.stepNs, timeUnit);
  renderCoverageMarkers();
  coverageInfo.title =
    markers.length
      ? `coverage ${formatPercent(startRatio)} .. ${formatPercent(endRatio)} (span ${formatPercent(spanRatio)}), ${markers.length} markers`
      : `coverage ${formatPercent(startRatio)} .. ${formatPercent(endRatio)} (span ${formatPercent(spanRatio)})`;
  coverageInfo.setAttribute(
    "aria-label",
    `coverage ${formatPercent(startRatio)} to ${formatPercent(endRatio)}, span ${formatPercent(spanRatio)}`
  );
  for (const time of grid.marks) {
    const x = xOf(time, leftLabel, waveWidth);
    ctx.strokeStyle = "#202847";
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }

  visible.forEach((signal, index) => {
    const rowTop = top + index * rowHeight;
    const preview = Boolean(query) && !selected.has(signal.id);
    const handle = rowHandleRect(rowTop, rowHeight);
    const action = preview ? null : rowActionRect(rowTop, rowHeight, leftLabel);
    rowLayout.push({
      index,
      signal,
      top: rowTop,
      bottom: rowTop + rowHeight,
      height: rowHeight,
      handle,
      action,
    });
    ctx.save();
    if (preview) {
      ctx.globalAlpha = 0.55;
    }
    const draggingRow = dragState
      && dragState.kind === "reorder"
      && dragState.visibleKey === visibleKey
      && dragState.sourceId === signal.id;
    if (draggingRow) {
      ctx.fillStyle = "rgba(110, 168, 255, 0.14)";
      ctx.fillRect(0, rowTop, width, rowHeight);
      ctx.strokeStyle = "rgba(110, 168, 255, 0.45)";
      ctx.strokeRect(0.5, rowTop + 0.5, width - 1, rowHeight - 1);
    }
    ctx.strokeStyle = "#1f2744";
    ctx.beginPath();
    ctx.moveTo(0, rowTop + rowHeight);
    ctx.lineTo(width, rowTop + rowHeight);
    ctx.stroke();
    const handleActive = hoverHandleSignalId === signal.id
      || (dragState && dragState.kind === "reorder" && dragState.sourceId === signal.id);
    const handleCenterX = (handle.left + handle.right) / 2;
    const handleCenterY = (handle.top + handle.bottom) / 2;
    ctx.fillStyle = handleActive ? "#edf3ff" : "#aebdd6";
    for (const offsetX of [-3, 3]) {
      for (const offsetY of [-4, 0, 4]) {
        ctx.beginPath();
        ctx.arc(handleCenterX + offsetX, handleCenterY + offsetY, handleActive ? 1.45 : 1.2, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    ctx.fillStyle = "#eef2ff";
    ctx.fillText(signal.label, labelTextX(), rowTop + 20);
    ctx.fillStyle = "#7b86b9";
    ctx.fillText(
      preview ? `${signal.category}/${signal.group} preview` : `${signal.category}/${signal.group}`,
      labelMetaX(),
      rowTop + 38
    );
    if (action) {
      const active = hoverActionSignalId === signal.id;
      const centerX = (action.left + action.right) / 2;
      const centerY = (action.top + action.bottom) / 2;
      const radius = (action.right - action.left) / 2;
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
      ctx.fillStyle = active ? "#26344d" : "#182234";
      ctx.fill();
      ctx.strokeStyle = active ? "#95a7c6" : "#5d6f8d";
      ctx.stroke();
      ctx.strokeStyle = active ? "#edf3ff" : "#c6d2e8";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(centerX - 4, centerY);
      ctx.lineTo(centerX + 4, centerY);
      ctx.stroke();
    }
    if (signal.kind === "digital") {
      renderDigital(signal, rowTop, rowHeight, leftLabel, waveWidth);
    } else if (signal.kind === "analog") {
      renderAnalog(signal, rowTop, rowHeight, leftLabel, waveWidth);
    } else if (signal.kind === "integer" || signal.kind === "text") {
      renderBusLike(signal, rowTop, rowHeight, leftLabel, waveWidth);
    } else if (signal.kind === "event") {
      renderEvent(signal, rowTop, rowHeight, leftLabel, waveWidth);
    }
    ctx.restore();
  });

  renderWaveMarkers(leftLabel, waveWidth, top, height);

  if (
    dragState
    && dragState.kind === "reorder"
    && dragState.visibleKey === visibleKey
    && reorderWouldChange(dragState.sourceIndex, dragState.targetIndex, dragState.visibleIds.length)
  ) {
    const y = insertionLineY(dragState.targetIndex);
    ctx.strokeStyle = "#6ea8ff";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(4, y);
    ctx.lineTo(width - 4, y);
    ctx.stroke();
    ctx.fillStyle = "#6ea8ff";
    ctx.fillRect(4, y - 3, 6, 6);
    ctx.fillRect(width - 10, y - 3, 6, 6);
  }

  renderCursor(rowHeight, top, leftLabel, waveWidth, height);
  renderMarkerStrip();
  const previewCount = query
    ? visible.filter(signal => !selected.has(signal.id)).length
    : 0;
  stats.textContent = query
    ? `${visible.length} matched signals, ${previewCount} preview, ${events.length} events, ${markers.length} markers`
    : `${visible.length} visible signals, ${events.length} events, ${markers.length} markers`;
}

document.getElementById("zoom-in").addEventListener("click", () => {
  const center = (viewStart + viewEnd) / 2;
  const span = Math.max(10, (viewEnd - viewStart) / 2);
  viewStart = center - span / 2;
  viewEnd = center + span / 2;
  clampView();
  render();
});

document.getElementById("zoom-out").addEventListener("click", () => {
  const center = (viewStart + viewEnd) / 2;
  const span = Math.max(10, (viewEnd - viewStart) * 2);
  viewStart = center - span / 2;
  viewEnd = center + span / 2;
  clampView();
  render();
});

document.getElementById("reset").addEventListener("click", () => {
  viewStart = data.start_ns;
  viewEnd = data.end_ns;
  render();
});

document.getElementById("show-default").addEventListener("click", () => {
  selected.clear();
  for (const signal of signals) {
    if (signal.default_visible) {
      selected.add(signal.id);
    }
  }
  buildSidebar();
  render();
});

document.getElementById("hide-all").addEventListener("click", () => {
  selected.clear();
  buildSidebar();
  render();
});

markerAddButton.addEventListener("click", () => {
  addMarkerFromInputs();
});

markerAddCursorButton.addEventListener("click", () => {
  if (!hoverState) {
    setMarkerStatus("Move the cursor over the wave first");
    return;
  }
  markerTimeInput.value = formatMarkerTime(hoverState.timeNs);
  addMarkerFromInputs(hoverState.timeNs);
});

markerTimeInput.addEventListener("keydown", event => {
  if (event.key === "Enter") {
    addMarkerFromInputs();
  }
});

markerTimeInput.addEventListener("input", () => {
  setMarkerStatus("");
});

markerLabelInput.addEventListener("keydown", event => {
  if (event.key === "Enter") {
    addMarkerFromInputs();
  }
});

markerLabelInput.addEventListener("input", () => {
  setMarkerStatus("");
});

sidebarToggle.addEventListener("click", () => {
  toggleSidebarCollapsed();
});

sidebarClose.addEventListener("click", () => {
  applySidebarCollapsed(true);
});

search.addEventListener("input", () => {
  buildSidebar();
  render();
});

window.addEventListener("mousedown", event => {
  if (activeMarkerId === null) {
    return;
  }
  if (shouldKeepMarkerFocusOnMouseDown(event)) {
    return;
  }
  clearActiveMarker();
});

window.addEventListener("keydown", event => {
  const target = event.target;
  const editing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  if (!editing && (event.key === "m" || event.key === "M")) {
    if (!hoverState) {
      setMarkerStatus("Move the cursor over the wave first");
      return;
    }
    event.preventDefault();
    addMarker(hoverState.timeNs, markerLabelInput.value);
    markerTimeInput.value = formatMarkerTime(hoverState.timeNs);
    return;
  }
  if ((event.key === "Delete" || event.key === "Backspace") && activeMarkerId !== null) {
    if (editing) {
      return;
    }
    event.preventDefault();
    removeMarker(activeMarkerId);
  }
});

header.addEventListener("wheel", event => {
  if (coverageInfo.contains(event.target)) {
    return;
  }
  if (!event.shiftKey && !event.altKey) {
    return;
  }
  event.preventDefault();
  if (event.shiftKey) {
    panViewFromWheel(event, coverageRect().width);
    return;
  }
  zoomViewAroundCenter(event);
}, { passive: false });

coverageInfo.addEventListener("wheel", event => {
  event.preventDefault();
  if (event.shiftKey) {
    panViewFromWheel(event, coverageRect().width);
    return;
  }
  if (event.altKey) {
    zoomViewAroundCenter(event);
    return;
  }
  const anchorRatio = coverageRatioAtClientX(event.clientX);
  const currentStartRatio = (viewStart - data.start_ns) / totalSpanNs();
  const currentEndRatio = (viewEnd - data.start_ns) / totalSpanNs();
  const factor = wheelZoomFactor(event);
  const currentSpanRatio = Math.max(minViewSpanRatio(), currentEndRatio - currentStartRatio);
  const newSpanRatio = Math.max(minViewSpanRatio(), Math.min(1, currentSpanRatio * factor));
  const anchorOffsetRatio = (anchorRatio - currentStartRatio) / Math.max(currentSpanRatio, 1e-9);
  let nextStartRatio = anchorRatio - newSpanRatio * anchorOffsetRatio;
  let nextEndRatio = nextStartRatio + newSpanRatio;
  if (nextStartRatio < 0) {
    nextEndRatio -= nextStartRatio;
    nextStartRatio = 0;
  }
  if (nextEndRatio > 1) {
    nextStartRatio -= nextEndRatio - 1;
    nextEndRatio = 1;
  }
  setViewFromRatios(
    Math.max(0, Math.min(nextStartRatio, 1 - minViewSpanRatio())),
    Math.min(1, Math.max(nextEndRatio, minViewSpanRatio()))
  );
}, { passive: false });

coverageWindow.addEventListener("mousedown", event => {
  if (event.target === coverageHandleLeft || event.target === coverageHandleRight) {
    return;
  }
  event.preventDefault();
  dragState = {
    kind: "pan",
    startClientX: event.clientX,
    startViewStart: viewStart,
    startViewEnd: viewEnd,
  };
  coverageWindow.classList.add("dragging");
});

coverageHandleLeft.addEventListener("mousedown", event => {
  event.preventDefault();
  event.stopPropagation();
  dragState = {
    kind: "left",
    fixedEndRatio: (viewEnd - data.start_ns) / totalSpanNs(),
  };
  coverageWindow.classList.add("dragging");
});

coverageHandleRight.addEventListener("mousedown", event => {
  event.preventDefault();
  event.stopPropagation();
  dragState = {
    kind: "right",
    fixedStartRatio: (viewStart - data.start_ns) / totalSpanNs(),
  };
  coverageWindow.classList.add("dragging");
});

viewer.addEventListener("wheel", event => {
  if (!handleViewerWheelGesture(event)) {
    return;
  }
  event.preventDefault();
}, { passive: false });

viewer.addEventListener("mousedown", event => {
  const rect = canvas.getBoundingClientRect();
  const logicalWidth = canvas.width / window.devicePixelRatio;
  const logicalHeight = canvas.height / window.devicePixelRatio;
  const logicalX = (event.clientX - rect.left) * canvas.width / Math.max(1, rect.width) / window.devicePixelRatio;
  const logicalY = (event.clientY - rect.top) * canvas.height / Math.max(1, rect.height) / window.devicePixelRatio;
  if (event.button === 0) {
    const marker = markerAtViewerLogicalPoint(logicalX, logicalY, logicalWidth, logicalHeight);
    if (marker) {
      event.preventDefault();
      if (activeMarkerId !== marker.id) {
        focusMarker(marker.id, { shouldRender: false });
        hoverActionSignalId = null;
        hoverHandleSignalId = null;
        hoverState = null;
        render();
        return;
      }
      focusMarker(marker.id, { shouldRender: false });
      hoverActionSignalId = null;
      hoverHandleSignalId = null;
      hoverState = null;
      dragState = {
        kind: "marker-move",
        markerId: marker.id,
        source: "viewer",
      };
      viewer.style.cursor = "ew-resize";
      render();
      return;
    }
    const handleRow = rowHandleAt(logicalX, logicalY);
    if (!handleRow) {
      return;
    }
    event.preventDefault();
    const visibleIds = rowLayout.map(row => row.signal.id);
    hoverActionSignalId = null;
    hoverHandleSignalId = handleRow.signal.id;
    hoverState = null;
    dragState = {
      kind: "reorder",
      sourceId: handleRow.signal.id,
      sourceIndex: handleRow.index,
      targetIndex: handleRow.index,
      visibleIds,
      visibleKey: visibleIds.join("\n"),
    };
    viewer.style.cursor = "grabbing";
    render();
    return;
  }
  if (event.button !== 1) {
    return;
  }
  event.preventDefault();
  hoverActionSignalId = null;
  hoverHandleSignalId = null;
  hoverState = null;
  dragState = {
    kind: "viewer-pan",
    startClientX: event.clientX,
    startViewStart: viewStart,
    startViewEnd: viewEnd,
    waveWidth: viewerWaveMetrics().waveWidth,
  };
  viewer.style.cursor = "grabbing";
  render();
});

viewer.addEventListener("auxclick", event => {
  if (event.button === 1) {
    event.preventDefault();
  }
});

viewer.addEventListener("mouseleave", () => {
  hoverState = null;
  hoverActionSignalId = null;
  hoverHandleSignalId = null;
  if (!dragState || (dragState.kind !== "viewer-pan" && dragState.kind !== "reorder" && dragState.kind !== "marker-move")) {
    viewer.style.cursor = "default";
  }
  render();
});

viewer.addEventListener("mousemove", event => {
  if (dragState && (dragState.kind === "viewer-pan" || dragState.kind === "reorder" || dragState.kind === "marker-move")) {
    return;
  }
  const rect = canvas.getBoundingClientRect();
  const logicalWidth = canvas.width / window.devicePixelRatio;
  const logicalHeight = canvas.height / window.devicePixelRatio;
  const logicalX = (event.clientX - rect.left) * canvas.width / Math.max(1, rect.width) / window.devicePixelRatio;
  const logicalY = (event.clientY - rect.top) * canvas.height / Math.max(1, rect.height) / window.devicePixelRatio;
  const marker = markerAtViewerLogicalPoint(logicalX, logicalY, logicalWidth, logicalHeight);
  const handleRow = rowHandleAt(logicalX, logicalY);
  const actionRow = actionRowHit(logicalX, logicalY);
  hoverHandleSignalId = handleRow ? handleRow.signal.id : null;
  hoverActionSignalId = actionRow ? actionRow.signal.id : null;
  viewer.style.cursor = marker ? "ew-resize" : (handleRow ? "grab" : (actionRow ? "pointer" : "default"));
  const waveLeft = SIGNAL_LABEL_WIDTH;
  const waveWidth = Math.max(1, rect.width - waveLeft);
  const ratio = Math.max(0, Math.min(1, (logicalX - waveLeft) / waveWidth));
  hoverState = {
    timeNs: viewStart + (viewEnd - viewStart) * ratio,
    y: logicalY,
  };
  render();
});

viewer.addEventListener("click", event => {
  const rect = canvas.getBoundingClientRect();
  const logicalX = (event.clientX - rect.left) * canvas.width / Math.max(1, rect.width) / window.devicePixelRatio;
  const logicalY = (event.clientY - rect.top) * canvas.height / Math.max(1, rect.height) / window.devicePixelRatio;
  const actionRow = actionRowHit(logicalX, logicalY);
  if (!actionRow) {
    return;
  }
  selected.delete(actionRow.signal.id);
  buildSidebar();
  render();
});

window.addEventListener("mouseup", () => {
  if (dragState && dragState.kind === "reorder") {
    reorderSignal(dragState.sourceId, dragState.sourceIndex, dragState.targetIndex, dragState.visibleIds);
  }
  dragState = null;
  coverageWindow.classList.remove("dragging");
  viewer.style.cursor = hoverHandleSignalId ? "grab" : (hoverActionSignalId ? "pointer" : "default");
  render();
});

window.addEventListener("mousemove", event => {
  if (!dragState) {
    return;
  }
  if (dragState.kind === "reorder") {
    const rect = canvas.getBoundingClientRect();
    const logicalY = (event.clientY - rect.top) * canvas.height / Math.max(1, rect.height) / window.devicePixelRatio;
    dragState.targetIndex = reorderTargetIndexAt(logicalY);
    render();
    return;
  }
  if (dragState.kind === "pan") {
    const deltaRatio = (event.clientX - dragState.startClientX) / Math.max(1, coverageRect().width);
    const deltaNs = deltaRatio * totalSpanNs();
    viewStart = dragState.startViewStart + deltaNs;
    viewEnd = dragState.startViewEnd + deltaNs;
    clampView();
    render();
    return;
  }
  if (dragState.kind === "left") {
    const nextStartRatio = Math.min(
      coverageRatioAtClientX(event.clientX),
      dragState.fixedEndRatio - minViewSpanRatio()
    );
    setViewFromRatios(
      Math.max(0, nextStartRatio),
      dragState.fixedEndRatio
    );
    return;
  }
  if (dragState.kind === "marker-move") {
    if (dragState.source === "coverage") {
      const ratio = coverageRatioAtClientX(event.clientX);
      updateMarkerTime(dragState.markerId, data.start_ns + ratio * totalSpanNs());
      return;
    }
    const rect = canvas.getBoundingClientRect();
    const logicalWidth = canvas.width / window.devicePixelRatio;
    const logicalX = (event.clientX - rect.left) * canvas.width / Math.max(1, rect.width) / window.devicePixelRatio;
    updateMarkerTime(dragState.markerId, timeAtViewerLogicalX(logicalX, logicalWidth));
    return;
  }
  if (dragState.kind === "viewer-pan") {
    const deltaRatio = (event.clientX - dragState.startClientX) / Math.max(1, dragState.waveWidth);
    const deltaNs = -deltaRatio * Math.max(1, dragState.startViewEnd - dragState.startViewStart);
    viewStart = dragState.startViewStart + deltaNs;
    viewEnd = dragState.startViewEnd + deltaNs;
    clampView();
    render();
    return;
  }
  const nextEndRatio = Math.max(
    coverageRatioAtClientX(event.clientX),
    dragState.fixedStartRatio + minViewSpanRatio()
  );
  setViewFromRatios(
    dragState.fixedStartRatio,
    Math.min(1, nextEndRatio)
  );
});

window.addEventListener("resize", () => {
  render();
});

applySidebarCollapsed(loadSidebarCollapsed(), { persist: false, shouldRender: false });
buildSidebar();
render();
</script>
</body>
</html>
"###
}

#[cfg(test)]
mod tests {
    use super::{
        I2cEventDecoder, TRACK_EVENT_I2C, WaveCaptureWindow, WaveEventNote, WaveRecorder,
        signal_aliases,
    };

    #[test]
    fn i2c_decoder_marks_start_bytes_ack_and_stop() {
        let mut decoder = I2cEventDecoder::default();
        let mut events = Vec::new();

        let states = [
            (0, true, true),
            (10, true, false),
            (20, false, false),
            (30, false, true),
            (40, true, true),
            (50, false, true),
            (60, false, false),
            (70, true, false),
            (80, false, false),
            (90, false, true),
            (100, true, true),
            (110, false, true),
            (120, false, false),
            (130, true, false),
            (140, false, false),
            (150, false, false),
            (160, true, false),
            (170, false, false),
            (180, false, false),
            (190, true, false),
            (200, false, false),
            (210, false, false),
            (220, true, false),
            (230, false, false),
            (240, false, false),
            (250, true, false),
            (260, false, false),
            (270, true, false),
            (280, false, false),
            (290, false, false),
            (300, true, false),
            (310, false, false),
            (320, false, false),
            (330, true, false),
            (340, false, false),
            (350, false, false),
            (360, true, false),
            (370, false, false),
            (380, false, false),
            (390, true, false),
            (400, false, false),
            (410, false, false),
            (420, true, false),
            (430, false, false),
            (440, true, false),
            (450, false, false),
            (460, false, false),
            (470, true, false),
            (480, false, false),
            (490, false, false),
            (500, false, false),
            (510, true, false),
            (520, false, false),
            (530, false, false),
            (540, true, false),
            (550, false, false),
            (560, false, false),
            (570, true, false),
            (580, false, false),
            (590, false, false),
            (600, true, false),
            (610, false, false),
            (620, false, false),
            (630, true, false),
            (640, false, false),
            (650, false, false),
            (660, true, false),
            (670, false, false),
            (680, true, false),
            (690, true, true),
        ];

        for (time_ns, scl, sda) in states {
            events.extend(decoder.observe(time_ns, scl, sda));
        }

        let labels = events
            .iter()
            .filter(|event| event.track_id == TRACK_EVENT_I2C)
            .map(|event| event.label.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["START", "ADDR 0xA0 W", "ACK", "TX 0x00", "ACK", "STOP"]
        );
    }

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
}
