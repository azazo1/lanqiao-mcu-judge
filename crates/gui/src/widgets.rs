use std::{path::Path, path::PathBuf};

use anyhow::{Context, Result};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use stcjudge::{BoardSnapshot, CheckpointRecord, CheckpointStatus, LedId};

const LED_ORDER: [LedId; 8] = [
    LedId::L1,
    LedId::L2,
    LedId::L3,
    LedId::L4,
    LedId::L5,
    LedId::L6,
    LedId::L7,
    LedId::L8,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum UartOutputMode {
    Text,
    Hex,
}

struct UartOutputView<'a> {
    label: &'a str,
    text: &'a str,
    error: Option<&'a str>,
    raw: &'a [u16],
}

pub(crate) fn show_tab_scroll(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .show(ui, add_contents);
}

pub(crate) fn draw_board_overview(
    ui: &mut egui::Ui,
    snapshot: &BoardSnapshot,
    uart_mode: &mut UartOutputMode,
    uart_stick_to_bottom: &mut bool,
    force_scroll_bottom: bool,
    clear_uart_output: &mut bool,
) {
    ui.heading("板级状态");
    ui.monospace(format!("time: {}", readable_time_ns(snapshot.sim_time_ns)));
    ui.monospace(format!("pc: 0x{:04X}", snapshot.pc));
    ui.monospace(format!(
        "seg: \"{}\" [{}]",
        snapshot.display_text,
        snapshot
            .seg_raw
            .iter()
            .map(|value| format!("{value:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    draw_seg_display(ui, snapshot);
    draw_led_row(ui, snapshot);
    ui.label(format!(
        "relay={} motor={} buzzer={}",
        snapshot.relay_on, snapshot.motor_on, snapshot.buzzer_on
    ));
    let mode_before = *uart_mode;
    let stick_before = *uart_stick_to_bottom;
    let mut stick_changed = false;
    ui.horizontal(|ui| {
        ui.label("UART 输出");
        ui.selectable_value(uart_mode, UartOutputMode::Text, "字符");
        ui.selectable_value(uart_mode, UartOutputMode::Hex, "HEX");
        stick_changed = ui.checkbox(uart_stick_to_bottom, "自动滚到底部").changed();
        if ui.button("清空输出").clicked() {
            *clear_uart_output = true;
        }
    });
    let stick_requested = *uart_stick_to_bottom;
    let force_scroll_bottom = stick_requested
        && (force_scroll_bottom || *uart_mode != mode_before || (stick_changed && !stick_before));
    let uart1_at_bottom = draw_uart_output(
        ui,
        UartOutputView {
            label: "uart1",
            text: &snapshot.uart1_text,
            error: snapshot.uart1_text_error.as_deref(),
            raw: &snapshot.uart1_raw,
        },
        *uart_mode,
        stick_requested,
        force_scroll_bottom,
    );
    let uart2_at_bottom = draw_uart_output(
        ui,
        UartOutputView {
            label: "uart2",
            text: &snapshot.uart2_text,
            error: snapshot.uart2_text_error.as_deref(),
            raw: &snapshot.uart2_raw,
        },
        *uart_mode,
        stick_requested,
        force_scroll_bottom,
    );
    if !stick_requested && (uart1_at_bottom || uart2_at_bottom) {
        *uart_stick_to_bottom = true;
    }
}

pub(crate) fn draw_ports(ui: &mut egui::Ui, snapshot: &BoardSnapshot) {
    ui.heading("端口和锁存器");
    ui.monospace(format!(
        "latch: {}",
        snapshot
            .port_latch
            .iter()
            .enumerate()
            .map(|(index, value)| format!("P{index}={value:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    ui.monospace(format!(
        "input: {}",
        snapshot
            .port_input
            .iter()
            .enumerate()
            .map(|(index, value)| format!("P{index}={value:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    ui.monospace(format!(
        "effective: [{}]",
        hex_bytes(&snapshot.board_latches_effective)
    ));
    ui.monospace(format!(
        "port: [{}]",
        hex_bytes(&snapshot.board_latches_port)
    ));
    ui.monospace(format!(
        "xdata: [{}]",
        hex_bytes(&snapshot.board_latches_xdata)
    ));
    ui.label(format!(
        "RD1={:.3} V RB2={:.3} V",
        snapshot.analog_rd1_v, snapshot.analog_rb2_v
    ));
    ui.label(format!(
        "ADC ch{} code={} {:.3} V",
        snapshot.adc_channel, snapshot.adc_code, snapshot.adc_channel_voltage_v
    ));
    ui.label(format!(
        "DAC code={} {:.3} V",
        snapshot.dac_code, snapshot.dac_voltage_v
    ));
    ui.label(format!(
        "NE555 level={} {:.2} Hz",
        snapshot.ne555_level, snapshot.ne555_frequency_hz
    ));
}

fn draw_seg_display(ui: &mut egui::Ui, snapshot: &BoardSnapshot) {
    let available_width = ui.available_width().max(1.0);
    let gap = 4.0;
    let digit_width = ((available_width - gap * 7.0) / 8.0).clamp(16.0, 38.0);
    let digit_height = digit_width * 1.3;
    let width = digit_width * 8.0 + gap * 7.0;
    let height = digit_height + 8.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let active = egui::Color32::from_rgb(238, 82, 68);
    let inactive = ui
        .visuals()
        .widgets
        .noninteractive
        .bg_stroke
        .color
        .gamma_multiply(0.45);
    let background = ui.visuals().extreme_bg_color.gamma_multiply(0.85);
    painter.rect_filled(rect, egui::CornerRadius::same(4), background);

    for (index, raw) in snapshot.seg_raw.iter().copied().enumerate() {
        let x = rect.left() + index as f32 * (digit_width + gap);
        let digit_rect = egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + 4.0),
            egui::vec2(digit_width, digit_height),
        );
        draw_seg_digit(&painter, digit_rect, !raw, active, inactive);
    }

    if response.hovered() {
        response.on_hover_text(format!(
            "seg raw: {}",
            snapshot
                .seg_raw
                .iter()
                .map(|value| format!("{value:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
}

fn draw_seg_digit(
    painter: &egui::Painter,
    rect: egui::Rect,
    pattern: u8,
    active: egui::Color32,
    inactive: egui::Color32,
) {
    let thickness = (rect.width() * 0.13).clamp(2.0, 5.0);
    let dot_radius = thickness * 0.65;
    let dot_space = dot_radius * 2.4;
    let body = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.right() - dot_space, rect.bottom()),
    )
    .shrink(2.0);
    let x0 = body.left();
    let x1 = body.right();
    let y0 = body.top();
    let y1 = body.bottom();
    let mid = body.center().y;
    let half = thickness * 0.5;
    let radius = egui::CornerRadius::same(2);

    let segments = [
        (
            0x01,
            egui::Rect::from_min_max(
                egui::pos2(x0 + thickness, y0),
                egui::pos2(x1 - thickness, y0 + thickness),
            ),
        ),
        (
            0x02,
            egui::Rect::from_min_max(
                egui::pos2(x1 - thickness, y0 + thickness),
                egui::pos2(x1, mid - half),
            ),
        ),
        (
            0x04,
            egui::Rect::from_min_max(
                egui::pos2(x1 - thickness, mid + half),
                egui::pos2(x1, y1 - thickness),
            ),
        ),
        (
            0x08,
            egui::Rect::from_min_max(
                egui::pos2(x0 + thickness, y1 - thickness),
                egui::pos2(x1 - thickness, y1),
            ),
        ),
        (
            0x10,
            egui::Rect::from_min_max(
                egui::pos2(x0, mid + half),
                egui::pos2(x0 + thickness, y1 - thickness),
            ),
        ),
        (
            0x20,
            egui::Rect::from_min_max(
                egui::pos2(x0, y0 + thickness),
                egui::pos2(x0 + thickness, mid - half),
            ),
        ),
        (
            0x40,
            egui::Rect::from_min_max(
                egui::pos2(x0 + thickness, mid - half),
                egui::pos2(x1 - thickness, mid + half),
            ),
        ),
    ];

    for (bit, segment) in segments {
        painter.rect_filled(
            segment,
            radius,
            if pattern & bit != 0 { active } else { inactive },
        );
    }

    painter.circle_filled(
        egui::pos2(
            rect.right() - dot_radius - 1.0,
            rect.bottom() - dot_radius - 2.0,
        ),
        dot_radius,
        if pattern & 0x80 != 0 {
            active
        } else {
            inactive
        },
    );
}

fn draw_led_row(ui: &mut egui::Ui, snapshot: &BoardSnapshot) {
    ui.horizontal_wrapped(|ui| {
        ui.label("LED");
        for (index, led) in LED_ORDER.into_iter().enumerate() {
            let on = snapshot.led_states[index];
            let text = format!("{led:?}");
            ui.colored_label(
                if on {
                    egui::Color32::from_rgb(20, 140, 80)
                } else {
                    egui::Color32::GRAY
                },
                text,
            );
        }
    });
}

fn draw_uart_output(
    ui: &mut egui::Ui,
    view: UartOutputView<'_>,
    mode: UartOutputMode,
    stick_to_bottom: bool,
    force_scroll_bottom: bool,
) -> bool {
    let mut at_scroll_bottom = false;
    ui.label(format!("{}:", view.label));
    let focus_id = ui.make_persistent_id(format!("board-overview-{}-output-focus", view.label));
    let scroll_focused = ui
        .ctx()
        .data_mut(|data| data.get_temp::<bool>(focus_id).unwrap_or(false));
    let stroke_color = if scroll_focused {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            let content = match mode {
                UartOutputMode::Text => view.text.to_owned(),
                UartOutputMode::Hex => format_uart_hex(view.raw),
            };
            let content = match (mode, view.error) {
                (UartOutputMode::Text, Some(error)) => format!("{content}\n{error}"),
                _ => content,
            };
            let output = if content.is_empty() {
                "-"
            } else {
                content.as_str()
            };
            let scroll_source = if scroll_focused {
                egui::scroll_area::ScrollSource::ALL
            } else {
                egui::scroll_area::ScrollSource::SCROLL_BAR
            };
            let mut output = egui::ScrollArea::both()
                .id_salt(format!("board-overview-{}-output", view.label))
                .auto_shrink([false, false])
                .max_height(56.0)
                .scroll_source(scroll_source)
                .stick_to_bottom(stick_to_bottom)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.monospace(output);
                });
            let clicked_inside = ui.rect_contains_pointer(output.inner_rect)
                && ui.input(|input| input.pointer.primary_clicked());
            let clicked_outside = !ui.rect_contains_pointer(output.inner_rect)
                && ui.input(|input| input.pointer.primary_clicked());
            if clicked_inside {
                ui.ctx().data_mut(|data| {
                    data.insert_temp(focus_id, true);
                });
            } else if clicked_outside {
                ui.ctx().data_mut(|data| {
                    data.insert_temp(focus_id, false);
                });
            }
            if scroll_focused && ui.rect_contains_pointer(output.inner_rect) {
                ui.input_mut(|input| {
                    input.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }
            let max_offset_y = (output.content_size.y - output.inner_rect.height()).max(0.0);
            let has_vertical_overflow = max_offset_y > 1.0;
            at_scroll_bottom = has_vertical_overflow && output.state.offset.y >= max_offset_y - 1.0;
            if force_scroll_bottom
                && has_vertical_overflow
                && output.state.offset.y < max_offset_y - 1.0
            {
                output.state.offset.y = max_offset_y;
                output.state.store(ui.ctx(), output.id);
                at_scroll_bottom = true;
                ui.ctx().request_repaint();
            }
        });
    at_scroll_bottom
}

fn format_uart_hex(raw: &[u16]) -> String {
    raw.iter()
        .map(|value| {
            if *value <= 0xFF {
                format!("{value:02X}")
            } else {
                format!("{value:03X}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn draw_logs(
    ui: &mut egui::Ui,
    logs: &mut Vec<String>,
    expanded: &mut bool,
    max_height: f32,
) {
    let latest = logs
        .last()
        .cloned()
        .unwrap_or_else(|| "暂无日志".to_owned());
    ui.horizontal(|ui| {
        let toggle_label = if *expanded {
            "收起日志"
        } else {
            "展开日志"
        };
        if ui.button(toggle_label).clicked() {
            *expanded = !*expanded;
        }
        ui.small(format!("{} 条", logs.len()));
        ui.separator();
        if logs.is_empty() {
            ui.small(&latest);
        } else {
            ui.add(egui::Label::new(egui::RichText::new(&latest).monospace().small()).truncate())
                .on_hover_text(&latest);
        }
        if ui
            .add_enabled(!logs.is_empty(), egui::Button::new("清空"))
            .clicked()
        {
            logs.clear();
        }
    });

    if *expanded {
        egui::ScrollArea::vertical()
            .max_height(max_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for log in logs {
                    ui.monospace(log);
                }
            });
    }
}

pub(crate) fn draw_checkpoint_table(ui: &mut egui::Ui, rows: &[CheckpointRecord], max_height: f32) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            draw_checkpoint_table_inner(ui, rows, max_height);
        });
}

fn draw_checkpoint_table_inner(ui: &mut egui::Ui, rows: &[CheckpointRecord], max_height: f32) {
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .auto_shrink([false, false])
        .max_scroll_height(max_height)
        .stick_to_bottom(true)
        .column(Column::auto())
        .column(
            Column::initial(240.0)
                .at_least(120.0)
                .clip(true)
                .resizable(true),
        )
        .column(
            Column::initial(240.0)
                .at_least(120.0)
                .clip(true)
                .resizable(true),
        )
        .column(Column::remainder().at_least(120.0).clip(true))
        .column(Column::exact(72.0))
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("序号");
            });
            header.col(|ui| {
                ui.strong("测试条件");
            });
            header.col(|ui| {
                ui.strong("期望结果");
            });
            header.col(|ui| {
                ui.strong("实际结果");
            });
            header.col(|ui| {
                ui.strong("状态");
            });
        })
        .body(|mut body| {
            for row in rows {
                body.row(24.0, |mut row_ui| {
                    row_ui.col(|ui| {
                        ui.label(row.index.to_string());
                    });
                    row_ui.col(|ui| {
                        truncated_cell(ui, &row.condition);
                    });
                    row_ui.col(|ui| {
                        truncated_cell(ui, &row.expected);
                    });
                    row_ui.col(|ui| {
                        let actual = if row.actual.is_empty() {
                            "-"
                        } else {
                            row.actual.as_str()
                        };
                        if row.status == CheckpointStatus::Failed {
                            colored_truncated_cell(ui, actual, status_color(row.status));
                        } else {
                            truncated_cell(ui, actual);
                        }
                    });
                    row_ui.col(|ui| {
                        ui.colored_label(status_color(row.status), status_label(row.status));
                    });
                });
            }
        });
}

fn truncated_cell(ui: &mut egui::Ui, text: &str) {
    let response = ui.add(egui::Label::new(text).truncate());
    if !text.is_empty() && response.hovered() {
        response.on_hover_text(text);
    }
}

fn colored_truncated_cell(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let response = ui.add(egui::Label::new(egui::RichText::new(text).color(color)).truncate());
    if !text.is_empty() && response.hovered() {
        response.on_hover_text(text);
    }
}

pub(crate) fn input_f32_row(
    ui: &mut egui::Ui,
    label: &str,
    text: &mut String,
) -> Option<Result<f32>> {
    let mut parsed = None;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(text);
        if ui.button("设置").clicked() {
            parsed = Some(
                text.trim()
                    .parse::<f32>()
                    .with_context(|| format!("{label} 必须是数字")),
            );
        }
    });
    parsed
}

pub(crate) fn slider_f32_row(
    ui: &mut egui::Ui,
    label: &str,
    text: &mut String,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) -> Option<Result<f32>> {
    let mut selected = None;
    let parsed = text.trim().parse::<f32>().ok();
    let range_start = *range.start();
    let range_end = *range.end();
    let mut value = parsed.unwrap_or(range_start).clamp(range_start, range_end);
    let inline = ui.available_width() >= 420.0;

    if inline {
        ui.horizontal(|ui| {
            ui.label(label);
            let slider_width = ui.available_width().max(140.0);
            let response = ui.add_sized(
                [slider_width, 24.0],
                egui::Slider::new(&mut value, range.clone())
                    .suffix(suffix)
                    .show_value(true),
            );
            if response.changed() {
                *text = format!("{value:.3}");
                selected = Some(Ok(value));
            }
        });
    } else {
        ui.vertical(|ui| {
            ui.label(label);
            let response = ui.add(
                egui::Slider::new(&mut value, range)
                    .suffix(suffix)
                    .show_value(true),
            );
            if response.changed() {
                *text = format!("{value:.3}");
                selected = Some(Ok(value));
            }
        });
    }
    selected
}

pub(crate) fn uart_row(ui: &mut egui::Ui, label: &str, text: &mut String) -> Option<Vec<u8>> {
    let mut bytes = None;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(text);
        if ui.button("发送").clicked() {
            bytes = Some(text.as_bytes().to_vec());
        }
    });
    bytes
}

pub(crate) fn wave_path_row(
    ui: &mut egui::Ui,
    label: &str,
    text: &mut String,
) -> Option<Option<PathBuf>> {
    let mut selected = None;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(text);
        if ui.button("设置").clicked() {
            let path = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
            selected = Some(path);
        }
        if ui.button("清空").clicked() {
            text.clear();
            selected = Some(None);
        }
        if ui.button("选择").clicked()
            && let Some(path) = rfd::FileDialog::new().save_file()
        {
            *text = path.display().to_string();
            selected = Some(Some(path));
        }
    });
    selected
}

fn hex_bytes<const N: usize>(bytes: &[u8; N]) -> String {
    bytes
        .iter()
        .map(|value| format!("{value:02X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn readable_time_ns(ns: u64) -> String {
    let readable = if ns >= 1_000_000_000 {
        format!("{:.1}s", ns as f64 / 1_000_000_000.0)
    } else if ns >= 1_000_000 {
        format!("{:.1}ms", ns as f64 / 1_000_000.0)
    } else if ns >= 1_000 {
        format!("{:.1}us", ns as f64 / 1_000.0)
    } else {
        format!("{ns}ns")
    };
    format!("{ns} ns ({readable})")
}

fn status_label(status: CheckpointStatus) -> &'static str {
    match status {
        CheckpointStatus::Running => "运行中",
        CheckpointStatus::Passed => "通过",
        CheckpointStatus::Failed => "失败",
    }
}

fn status_color(status: CheckpointStatus) -> egui::Color32 {
    match status {
        CheckpointStatus::Running => egui::Color32::from_rgb(225, 170, 70),
        CheckpointStatus::Passed => egui::Color32::from_rgb(50, 180, 110),
        CheckpointStatus::Failed => egui::Color32::from_rgb(220, 80, 80),
    }
}

pub(crate) fn path_label(label: &str, path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("{label}: {}", path.display()),
        None => format!("{label}: 未选择"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_time_ns_picks_units() {
        assert_eq!(readable_time_ns(999), "999 ns (999ns)");
        assert_eq!(readable_time_ns(1_500), "1500 ns (1.5us)");
        assert_eq!(readable_time_ns(15_000_000), "15000000 ns (15.0ms)");
        assert_eq!(readable_time_ns(15_000_000_000), "15000000000 ns (15.0s)");
    }

    #[test]
    fn format_uart_hex_keeps_ninth_bit_symbols() {
        assert_eq!(format_uart_hex(&[0x00, 0x0A, 0xFF, 0x101]), "00 0A FF 101");
    }
}
