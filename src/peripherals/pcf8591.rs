use std::mem;

use crate::wave::{TRACK_EVENT_ADC_DAC, WaveCaptureWindow, WaveEventNote};

use super::{
    analog::AnalogInputs,
    i2c_slave::{I2cSlaveDevice, I2cSlaveFrontend, I2cSlaveTiming},
};

#[derive(Debug, Clone)]
pub(crate) struct Pcf8591 {
    control: u8,
    dac_value: u8,
    adc_data: u8,
    selected_channel: u8,
    expecting_control: bool,
    frontend: I2cSlaveFrontend,
    wave_window: WaveCaptureWindow,
    wave_events: Option<Vec<WaveEventNote>>,
}

impl Default for Pcf8591 {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Pcf8591 {
    pub(crate) fn new(wave_enabled: bool) -> Self {
        Self::new_with_wave_window(WaveCaptureWindow::from_enabled(wave_enabled))
    }

    pub(crate) fn new_with_wave_window(wave_window: WaveCaptureWindow) -> Self {
        Self {
            control: 0,
            dac_value: 0,
            adc_data: 0x80,
            selected_channel: 0,
            expecting_control: false,
            frontend: I2cSlaveFrontend::default(),
            wave_window,
            wave_events: wave_window.enabled().then(Vec::new),
        }
    }

    const ADDRESS_BYTE: u8 = 0x90;
    const TIMING: I2cSlaveTiming = I2cSlaveTiming {
        min_scl_low_ns: 4_800,
        min_scl_high_ns: 4_100,
        min_start_stop_scl_high_ns: 4_100,
    };

    pub(crate) fn sample_i2c(
        &mut self,
        time_ns: u64,
        scl_high: bool,
        sda_high: bool,
        analog: &AnalogInputs,
    ) {
        let mut frontend = mem::take(&mut self.frontend);
        frontend.sample(time_ns, scl_high, sda_high, self, analog);
        self.frontend = frontend;
    }

    pub(crate) fn dac_value(&self) -> u8 {
        self.dac_value
    }

    pub(crate) fn dac_voltage_v(&self) -> f32 {
        f32::from(self.dac_value) / 255.0 * 5.0
    }

    pub(crate) fn adc_data(&self) -> u8 {
        self.adc_data
    }

    pub(crate) fn selected_channel(&self) -> u8 {
        self.effective_channel()
    }

    pub(crate) fn sda_drive_low(&self) -> bool {
        self.frontend.sda_drive_low()
    }

    pub(crate) fn scl_drive_low(&self) -> bool {
        self.frontend.scl_drive_low()
    }

    pub(crate) fn settle_i2c_lines(&mut self, sda_high: bool) {
        self.frontend.settle_lines(sda_high);
    }

    pub(crate) fn take_wave_events(&mut self) -> Vec<WaveEventNote> {
        match self.wave_events.as_mut() {
            Some(events) => mem::take(events),
            None => Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn force_lines_for_test(&mut self, scl_low: bool, sda_low: bool) {
        self.frontend.force_drive_levels(scl_low, sda_low);
    }

    fn input_mode(&self) -> u8 {
        (self.control >> 4) & 0x03
    }

    fn auto_increment(&self) -> bool {
        self.control & 0x04 != 0
    }

    fn set_control(&mut self, value: u8) {
        self.control = value & 0x7F;
        self.selected_channel = self.control & 0x03;
    }

    fn max_channel(&self) -> u8 {
        match self.input_mode() {
            0b00 => 3,
            0b01 => 2,
            0b10 => 2,
            0b11 => 1,
            _ => 3,
        }
    }

    fn effective_channel(&self) -> u8 {
        self.selected_channel.min(self.max_channel())
    }

    fn advance_channel(&mut self) {
        if !self.auto_increment() {
            return;
        }
        let channel = self.effective_channel();
        let max_channel = self.max_channel();
        self.selected_channel = if channel >= max_channel {
            0
        } else {
            channel + 1
        };
    }

    fn read_channel_code(&self, channel: u8, analog: &AnalogInputs) -> u8 {
        match self.input_mode() {
            0b00 => analog.channel_value(channel),
            0b01 => match channel {
                0 => self.diff_code(analog.channel_voltage(0), analog.channel_voltage(1)),
                1 => self.diff_code(analog.channel_voltage(1), analog.channel_voltage(0)),
                _ => self.diff_code(analog.channel_voltage(2), analog.channel_voltage(3)),
            },
            0b10 => match channel {
                0 => analog.channel_value(0),
                1 => analog.channel_value(1),
                _ => self.diff_code(analog.channel_voltage(2), analog.channel_voltage(3)),
            },
            0b11 => match channel {
                0 => self.diff_code(analog.channel_voltage(0), analog.channel_voltage(1)),
                _ => self.diff_code(analog.channel_voltage(2), analog.channel_voltage(3)),
            },
            _ => analog.channel_value(channel),
        }
    }

    fn diff_code(&self, positive: f32, negative: f32) -> u8 {
        let diff = ((positive - negative) / 5.0 * 128.0)
            .round()
            .clamp(-128.0, 127.0) as i16;
        (diff as i8) as u8
    }

    fn start_conversion(&mut self, analog: &AnalogInputs) {
        let channel = self.effective_channel();
        self.adc_data = self.read_channel_code(channel, analog);
        self.advance_channel();
    }

    fn push_event<L, D>(&mut self, time_ns: u64, label: L, detail: D)
    where
        L: FnOnce() -> String,
        D: FnOnce() -> String,
    {
        if self.wave_window.includes(time_ns)
            && let Some(events) = self.wave_events.as_mut()
        {
            events.push(WaveEventNote::with_detail(
                time_ns,
                TRACK_EVENT_ADC_DAC,
                label(),
                detail(),
            ));
        }
    }
}

impl I2cSlaveDevice for Pcf8591 {
    type Context = AnalogInputs;

    fn address_byte(&self) -> u8 {
        Self::ADDRESS_BYTE
    }

    fn timing(&self) -> I2cSlaveTiming {
        Self::TIMING
    }

    fn on_addressed_write(&mut self, time_ns: u64, _ctx: &Self::Context) -> bool {
        self.expecting_control = true;
        let control = self.control;
        self.push_event(
            time_ns,
            || "ADDR W".to_owned(),
            || format!("control=0x{control:02X}"),
        );
        true
    }

    fn on_addressed_read(&mut self, time_ns: u64, ctx: &Self::Context) -> Option<u8> {
        let current = self.adc_data;
        let channel = self.effective_channel();
        self.start_conversion(ctx);
        self.push_event(
            time_ns,
            || format!("ADC 0x{current:02X}"),
            || format!("channel={channel}"),
        );
        Some(current)
    }

    fn on_write_byte(&mut self, time_ns: u64, byte: u8, _ctx: &Self::Context) -> bool {
        if self.expecting_control {
            self.set_control(byte);
            self.expecting_control = false;
            let channel = self.effective_channel();
            self.push_event(
                time_ns,
                || format!("CTRL 0x{byte:02X}"),
                || format!("channel={channel}"),
            );
        } else {
            self.dac_value = byte;
            let voltage_v = self.dac_voltage_v();
            self.push_event(
                time_ns,
                || format!("DAC 0x{byte:02X}"),
                || format!("voltage={voltage_v:.3}V"),
            );
        }
        true
    }

    fn on_read_continue(&mut self, time_ns: u64, ctx: &Self::Context) -> u8 {
        let current = self.adc_data;
        let channel = self.effective_channel();
        self.start_conversion(ctx);
        self.push_event(
            time_ns,
            || format!("ADC 0x{current:02X}"),
            || format!("channel={channel}"),
        );
        current
    }
}

#[cfg(test)]
mod tests {
    use super::Pcf8591;
    use crate::peripherals::{AnalogInputs, i2c_slave::I2cSlaveDevice};
    use crate::wave::WaveCaptureWindow;

    #[test]
    fn first_read_returns_power_on_default_then_latest_conversion() {
        let mut pcf = Pcf8591::default();
        let mut analog = AnalogInputs::default();
        analog.set_voltage("AIN1", 4.0).expect("set AIN1");
        pcf.set_control(0x41);

        let first = <Pcf8591 as I2cSlaveDevice>::on_addressed_read(&mut pcf, 0, &analog)
            .expect("pcf read byte");
        let second = <Pcf8591 as I2cSlaveDevice>::on_read_continue(&mut pcf, 0, &analog);

        assert_eq!(first, 0x80);
        assert_eq!(second, 204);
    }

    #[test]
    fn auto_increment_advances_selected_channel_after_each_conversion() {
        let mut pcf = Pcf8591::default();
        let mut analog = AnalogInputs::default();
        analog.set_voltage("AIN1", 4.0).expect("set AIN1");
        analog.set_voltage("AIN3", 1.0).expect("set AIN3");
        pcf.set_control(0x45);

        let first = <Pcf8591 as I2cSlaveDevice>::on_addressed_read(&mut pcf, 0, &analog)
            .expect("pcf read byte");
        let second = <Pcf8591 as I2cSlaveDevice>::on_read_continue(&mut pcf, 0, &analog);
        let third = <Pcf8591 as I2cSlaveDevice>::on_read_continue(&mut pcf, 0, &analog);
        let fourth = <Pcf8591 as I2cSlaveDevice>::on_read_continue(&mut pcf, 0, &analog);

        assert_eq!(first, 0x80);
        assert_eq!(second, 204);
        assert_eq!(third, 0);
        assert_eq!(fourth, 51);
    }

    #[test]
    fn wave_disabled_does_not_buffer_events() {
        let mut pcf = Pcf8591::new(false);
        let analog = AnalogInputs::default();

        assert!(<Pcf8591 as I2cSlaveDevice>::on_write_byte(
            &mut pcf, 0, 0x40, &analog
        ));
        assert!(pcf.take_wave_events().is_empty());
    }

    #[test]
    fn wave_window_skips_events_before_start() {
        let mut pcf = Pcf8591::new_with_wave_window(WaveCaptureWindow::bounded(100, Some(200)));
        let analog = AnalogInputs::default();

        assert!(<Pcf8591 as I2cSlaveDevice>::on_write_byte(
            &mut pcf, 50, 0x40, &analog
        ));
        assert!(pcf.take_wave_events().is_empty());

        assert!(<Pcf8591 as I2cSlaveDevice>::on_write_byte(
            &mut pcf, 150, 0x41, &analog
        ));
        let events = pcf.take_wave_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].time_ns, 150);
    }
}
