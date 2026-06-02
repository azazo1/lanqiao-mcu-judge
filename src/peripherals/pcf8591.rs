use super::AnalogInputs;

#[derive(Debug, Default, Clone)]
pub(crate) struct Pcf8591 {
    control: u8,
    dac_value: u8,
    expecting_control: bool,
}

impl Pcf8591 {
    pub(crate) fn begin_write(&mut self) {
        self.expecting_control = true;
    }

    pub(crate) fn write_byte(&mut self, value: u8) {
        if self.expecting_control {
            self.control = value;
            self.expecting_control = false;
        } else {
            self.dac_value = value;
        }
    }

    pub(crate) fn read_byte(&self, analog: &AnalogInputs) -> u8 {
        analog.channel_value(self.control & 0x03)
    }
}
