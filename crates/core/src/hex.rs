use anyhow::{Context, Result, bail};
use ihex::Record;

pub fn load_ihex(input: &str) -> Result<Vec<u8>> {
    let mut image = vec![0_u8; 0x10000];
    let mut upper = 0_u32;
    let mut highest = 0_usize;

    for record in ihex::Reader::new(input) {
        let record = record.context("解析 IHEX 记录失败")?;
        match record {
            Record::Data { offset, value } => {
                let base = upper + u32::from(offset);
                for (index, byte) in value.into_iter().enumerate() {
                    let address = base as usize + index;
                    if address >= image.len() {
                        bail!("HEX 地址越界: 0x{address:04X}");
                    }
                    image[address] = byte;
                    highest = highest.max(address + 1);
                }
            }
            Record::ExtendedLinearAddress(value) => {
                upper = u32::from(value) << 16;
            }
            Record::EndOfFile => break,
            Record::StartSegmentAddress { .. }
            | Record::StartLinearAddress(_)
            | Record::ExtendedSegmentAddress(_) => {}
        }
    }

    if highest == 0 {
        bail!("HEX 文件中没有可执行数据");
    }

    image.truncate(highest);
    Ok(image)
}
