use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use eframe::egui;
use stcjudge::{
    BoardSnapshot, KeyId, KeyMode, ResetMode, ScriptRunControl, ScriptRunEvent, ScriptRunEventSink,
    SignalId, Simulator, VoltageChannel, estimate_checkpoint_total,
    run_script_source_with_events_and_control,
};

use crate::{
    api::judge_api_catalog,
    script_editor::{SCRIPT_EDITOR_ID, insert_snippet_at_editor_cursor},
    state::{AppTab, GuiSession, JudgeState, ReloadResult, UiFeedback, UiFeedbackKind},
    style::{install_cjk_fonts, tune_ui_style},
    syntax::highlight_rhai,
    widgets::{
        UartOutputMode, draw_board_overview, draw_checkpoint_table, draw_logs, draw_ports,
        input_f32_row, path_label, show_tab_scroll, slider_f32_row, uart_row, wave_path_row,
    },
};

const KEY_ORDER: [KeyId; 16] = [
    KeyId::S7,
    KeyId::S11,
    KeyId::S15,
    KeyId::S19,
    KeyId::S6,
    KeyId::S10,
    KeyId::S14,
    KeyId::S18,
    KeyId::S5,
    KeyId::S9,
    KeyId::S13,
    KeyId::S17,
    KeyId::S4,
    KeyId::S8,
    KeyId::S12,
    KeyId::S16,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UartOutputSignature {
    mode: UartOutputMode,
    uart1_raw_len: usize,
    uart1_text_error_len: usize,
    uart2_raw_len: usize,
    uart2_text_error_len: usize,
}

impl UartOutputSignature {
    fn from_snapshot(snapshot: &BoardSnapshot, mode: UartOutputMode) -> Self {
        Self {
            mode,
            uart1_raw_len: snapshot.uart1_raw.len(),
            uart1_text_error_len: snapshot.uart1_text_error.as_ref().map_or(0, String::len),
            uart2_raw_len: snapshot.uart2_raw.len(),
            uart2_text_error_len: snapshot.uart2_text_error.as_ref().map_or(0, String::len),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TemporaryKeyPress {
    key: KeyId,
    restore_pressed: bool,
}

pub struct StcjudgeGuiApp {
    session: GuiSession,
    tab: AppTab,
    judge: JudgeState,
    feedbacks: Vec<UiFeedback>,
    logs: Vec<String>,
    run_ms_input: String,
    run_us_input: String,
    sim_speed_limit_multiplier: f64,
    temperature_input: String,
    distance_input: String,
    frequency_input: String,
    rd1_input: String,
    rb2_input: String,
    uart1_input: String,
    uart2_input: String,
    uart_output_mode: UartOutputMode,
    uart_stick_to_bottom: bool,
    uart_output_signature: Option<UartOutputSignature>,
    wave_html_input: String,
    wave_json_input: String,
    wave_msgpack_input: String,
    wave_start_input: String,
    wave_end_input: String,
    repl_input: String,
    repl_history: Vec<String>,
    temporary_key_press: Option<TemporaryKeyPress>,
}

impl Default for StcjudgeGuiApp {
    fn default() -> Self {
        Self {
            session: GuiSession::empty(),
            tab: AppTab::Debug,
            judge: JudgeState::default(),
            feedbacks: Vec::new(),
            logs: Vec::new(),
            run_ms_input: "100".to_owned(),
            run_us_input: "1000".to_owned(),
            sim_speed_limit_multiplier: 1.0,
            temperature_input: "25.0".to_owned(),
            distance_input: "30.0".to_owned(),
            frequency_input: "1000.0".to_owned(),
            rd1_input: "2.5".to_owned(),
            rb2_input: "2.5".to_owned(),
            uart1_input: String::new(),
            uart2_input: String::new(),
            uart_output_mode: UartOutputMode::Text,
            uart_stick_to_bottom: true,
            uart_output_signature: None,
            wave_html_input: String::new(),
            wave_json_input: String::new(),
            wave_msgpack_input: String::new(),
            wave_start_input: "0".to_owned(),
            wave_end_input: String::new(),
            repl_input: "display_text()".to_owned(),
            repl_history: Vec::new(),
            temporary_key_press: None,
        }
    }
}

impl StcjudgeGuiApp {
    pub fn new(ctx: &eframe::CreationContext<'_>) -> Self {
        install_cjk_fonts(&ctx.egui_ctx);
        tune_ui_style(&ctx.egui_ctx);
        Self::default()
    }

    fn log(&mut self, message: impl Into<String>) {
        self.push_log(message.into());
    }

    fn notice(&mut self, message: impl Into<String>) {
        self.push_feedback(UiFeedbackKind::Info, message.into());
    }

    fn error(&mut self, message: impl Into<String>) {
        self.push_feedback(UiFeedbackKind::Error, message.into());
    }

    fn push_feedback(&mut self, kind: UiFeedbackKind, message: String) {
        self.push_log(message.clone());
        self.feedbacks.push(UiFeedback::new(kind, message));
        if self.feedbacks.len() > 6 {
            let excess = self.feedbacks.len().saturating_sub(6);
            self.feedbacks.drain(0..excess);
        }
    }

    fn push_log(&mut self, message: String) {
        self.logs.push(message);
        if self.logs.len() > 200 {
            let excess = self.logs.len().saturating_sub(200);
            self.logs.drain(0..excess);
        }
    }

    fn run_action(&mut self, action: impl FnOnce(&mut GuiSession) -> Result<()>) {
        if let Err(err) = action(&mut self.session) {
            self.error(err.to_string());
        }
    }

    fn select_hex_file(&mut self) {
        if let Err(err) = self.apply_wave_window() {
            self.error(err);
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Intel HEX", &["hex"])
            .pick_file()
        else {
            return;
        };
        match self.session.load_hex(path.clone()) {
            Ok(()) => self.notice(format!("已加载 {}", path.display())),
            Err(err) => self.error(err.to_string()),
        }
    }

    fn poll_judge_events(&mut self) {
        let Some(receiver) = self.judge.receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        while let Ok(event) = receiver.try_recv() {
            match &event {
                ScriptRunEvent::Finished(report) => {
                    if report.success() {
                        self.notice(format!(
                            "评测完成: {} 个评测点, {} 个失败",
                            report.total, report.failed
                        ));
                    } else {
                        self.error(format!(
                            "评测完成: {} 个评测点, {} 个失败",
                            report.total, report.failed
                        ));
                    }
                }
                ScriptRunEvent::Failed(report) => {
                    let message = report.error.as_deref().unwrap_or("评测脚本执行失败");
                    self.error(format!("评测运行失败: {message}"));
                }
                _ => {}
            }
            self.judge.update_from_event(event);
            if self.judge.receiver.is_none() && !self.judge.running {
                keep_receiver = false;
            }
        }
        if keep_receiver && self.judge.receiver.is_none() {
            self.judge.receiver = Some(receiver);
        }
    }

    fn start_judge_run(&mut self) {
        if self.judge.running {
            return;
        }
        if let Err(err) = self.apply_wave_window() {
            self.error(err);
            return;
        }
        let Some(hex_path) = self.session.hex_path.clone() else {
            self.error("请先加载 HEX 文件");
            return;
        };
        let source = self.judge.script_source.clone();
        if source.trim().is_empty() {
            self.error("请先选择或编写评测脚本");
            return;
        }
        let label = self
            .judge
            .script_path
            .as_ref()
            .map(|path| format!("file:{}", path.display()))
            .unwrap_or_else(|| "gui:script".to_owned());
        let wave_options = self.session.wave_options();
        let trace_cpu = self.session.trace_cpu;
        let (sender, receiver): (Sender<ScriptRunEvent>, Receiver<ScriptRunEvent>) =
            mpsc::channel();
        let control = ScriptRunControl::new();
        self.judge.reset_run_state();
        self.judge.running = true;
        self.judge.estimated_total = estimate_checkpoint_total(&source);
        self.judge.receiver = Some(receiver);
        self.judge.control = Some(control.clone());
        thread::spawn(move || {
            let sink = ScriptRunEventSink::new(move |event| {
                let _ = sender.send(event);
            });
            match Simulator::from_hex_path_with_options(&hex_path, trace_cpu, wave_options) {
                Ok(sim) => {
                    let _ = run_script_source_with_events_and_control(
                        sim, &label, &source, sink, control,
                    );
                }
                Err(err) => {
                    let report = stcjudge::ScriptRunReport {
                        total: 0,
                        failed: 0,
                        estimated_total_ckpts: estimate_checkpoint_total(&source),
                        records: Vec::new(),
                        report: String::new(),
                        error: Some(err.to_string()),
                    };
                    let _ = sink_event_failed(report, sink);
                }
            }
        });
    }

    fn pause_judge_run(&mut self) {
        let Some(control) = &self.judge.control else {
            self.error("当前没有正在运行的评测");
            return;
        };
        control.pause();
        self.judge.paused = true;
        self.notice("评测已暂停");
    }

    fn resume_judge_run(&mut self) {
        let Some(control) = &self.judge.control else {
            self.error("当前没有已暂停的评测");
            return;
        };
        control.resume();
        self.judge.paused = false;
        self.notice("评测已继续");
    }

    fn terminate_judge_run(&mut self) {
        let Some(control) = &self.judge.control else {
            self.error("当前没有正在运行的评测");
            return;
        };
        control.terminate();
        self.judge.paused = false;
        self.notice("正在终止评测");
    }

    fn draw_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for tab in [AppTab::Debug, AppTab::Judge, AppTab::Script, AppTab::Wave] {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
    }

    fn draw_feedback_toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.feedbacks.retain(|feedback| !feedback.expired(now));
        if self.feedbacks.is_empty() {
            return;
        }
        let mut close_index = None;
        egui::Area::new(egui::Id::new("feedback-toasts"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.vertical(|ui| {
                    for (index, feedback) in self.feedbacks.iter().enumerate() {
                        let color = feedback_color(feedback.kind);
                        egui::Frame::new()
                            .fill(color.gamma_multiply(0.16))
                            .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.75)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 7))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.colored_label(color, &feedback.message);
                                    if ui.small_button("关闭").clicked() {
                                        close_index = Some(index);
                                    }
                                });
                            });
                        ui.add_space(6.0);
                    }
                });
            });
        if let Some(index) = close_index {
            self.feedbacks.remove(index);
        }
    }

    fn draw_debug_tab(&mut self, ui: &mut egui::Ui) {
        self.draw_debug_toolbar(ui);
        ui.separator();

        let log_space = 132.0_f32.min((ui.available_height() * 0.3).max(92.0));
        let main_height = (ui.available_height() - log_space).max(220.0);
        ui.columns(2, |columns| {
            egui::ScrollArea::vertical()
                .id_salt("debug-tab-left-scroll")
                .auto_shrink([false, false])
                .max_height(main_height)
                .show(&mut columns[0], |ui| {
                    self.draw_debug_tab_content(ui);
                });
            egui::ScrollArea::vertical()
                .id_salt("debug-tab-input-scroll")
                .auto_shrink([false, false])
                .max_height(main_height)
                .show(&mut columns[1], |ui| {
                    self.draw_inputs_panel(ui);
                });
        });
        ui.separator();
        draw_logs(ui, &self.logs);
    }

    fn draw_debug_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("打开 HEX").clicked() {
                self.select_hex_file();
            }
            if ui.button("重新加载").clicked() {
                match self.session.reload() {
                    Ok(result) => {
                        self.temporary_key_press = None;
                        match result {
                            ReloadResult::Hex => self.notice("已重新加载 HEX"),
                            ReloadResult::Empty => self.notice("已重置空仿真器"),
                        }
                    }
                    Err(err) => self.error(err.to_string()),
                }
            }
            if ui.button("复位").clicked() {
                self.run_action(|session| {
                    session.sim.reset()?;
                    session.refresh();
                    Ok(())
                });
            }
            if ui.button("复位 CPU").clicked() {
                self.run_action(|session| {
                    session.sim.reset_with_mode(ResetMode::Cpu)?;
                    session.refresh();
                    Ok(())
                });
            }
            if ui
                .button(if self.session.running {
                    "暂停"
                } else {
                    "运行"
                })
                .clicked()
            {
                self.session.running = !self.session.running;
                self.session.last_tick = Instant::now();
            }
            if ui.button("单步").clicked() {
                self.run_action(|session| {
                    session.sim.step()?;
                    session.refresh();
                    Ok(())
                });
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("run_ms");
            ui.add_sized(
                [120.0, 28.0],
                egui::TextEdit::singleline(&mut self.run_ms_input),
            );
            if ui.button("执行").clicked() {
                let text = self.run_ms_input.clone();
                self.run_action(|session| {
                    let value = text.trim().parse::<u64>().context("run_ms 必须是整数")?;
                    session.sim.run_ms(value)?;
                    session.refresh();
                    Ok(())
                });
            }
            ui.separator();
            ui.label("run_us");
            ui.add_sized(
                [120.0, 28.0],
                egui::TextEdit::singleline(&mut self.run_us_input),
            );
            if ui.button("执行").clicked() {
                let text = self.run_us_input.clone();
                self.run_action(|session| {
                    let value = text.trim().parse::<u64>().context("run_us 必须是整数")?;
                    session.sim.run_us(value)?;
                    session.refresh();
                    Ok(())
                });
            }
            ui.separator();
            ui.label("速度上限");
            let speed_response = ui.add_sized(
                [96.0, 28.0],
                egui::DragValue::new(&mut self.sim_speed_limit_multiplier)
                    .range(0.01..=100.0)
                    .speed(0.1)
                    .suffix("x"),
            );
            if speed_response.changed() {
                self.sim_speed_limit_multiplier =
                    self.sim_speed_limit_multiplier.clamp(0.01, 100.0);
            }
        });
    }

    fn draw_debug_tab_content(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.session.snapshot.clone();
        let signature = UartOutputSignature::from_snapshot(&snapshot, self.uart_output_mode);
        let force_uart_scroll_bottom = self.uart_stick_to_bottom
            && self
                .uart_output_signature
                .is_some_and(|previous| previous != signature);
        let mut clear_uart_output = false;
        draw_board_overview(
            ui,
            &snapshot,
            &mut self.uart_output_mode,
            &mut self.uart_stick_to_bottom,
            force_uart_scroll_bottom,
            &mut clear_uart_output,
        );
        if clear_uart_output {
            self.run_action(|session| {
                session.sim.uart1_clear_output();
                session.sim.uart2_clear_output();
                session.refresh();
                Ok(())
            });
            self.uart_output_signature = None;
            ui.ctx().request_repaint();
        }
        self.uart_output_signature = Some(UartOutputSignature::from_snapshot(
            &self.session.snapshot,
            self.uart_output_mode,
        ));
        ui.separator();
        draw_ports(ui, &snapshot);
    }

    fn draw_inputs_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("按键");
        let mut primary_down_key = None;
        egui::Grid::new("keys-grid")
            .num_columns(4)
            .spacing([5.0, 5.0])
            .show(ui, |ui| {
                for (index, key) in KEY_ORDER.into_iter().enumerate() {
                    let pressed = self.session.snapshot.key_states[key_snapshot_index(key)];
                    let display_pressed = pressed
                        || self
                            .temporary_key_press
                            .is_some_and(|temporary| temporary.key == key);
                    let label = format!("{key:?}");
                    let response = ui
                        .add_sized(
                            [50.0, 28.0],
                            egui::Button::new(label).selected(display_pressed),
                        )
                        .on_hover_text("左键按住临时按下, 右键切换锁定");
                    if response.clicked_by(egui::PointerButton::Secondary) {
                        self.set_key_pressed(key, !pressed);
                    }
                    let primary_down_on_key = response.is_pointer_button_down_on()
                        && ui.input(|input| input.pointer.primary_down());
                    if primary_down_on_key {
                        primary_down_key = Some((key, pressed));
                    }
                    if index % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        if self.sync_temporary_key_press(primary_down_key) {
            ui.ctx().request_repaint();
        }

        ui.horizontal_wrapped(|ui| {
            ui.label("按键模式");
            let mut mode = self.session.snapshot.key_mode;
            if ui
                .selectable_label(mode == KeyMode::Keyboard, "矩阵键盘")
                .clicked()
            {
                mode = KeyMode::Keyboard;
            }
            if ui
                .selectable_label(mode == KeyMode::Button, "独立按键")
                .clicked()
            {
                mode = KeyMode::Button;
            }
            if mode != self.session.snapshot.key_mode {
                self.run_action(|session| {
                    session.sim.key_mode(mode);
                    session.refresh();
                    Ok(())
                });
            }
        });

        ui.add_space(4.0);
        ui.heading("传感器和跳帽");
        if let Some(result) = input_f32_row(ui, "温度 C", &mut self.temperature_input) {
            match result {
                Ok(value) => self.run_action(|session| {
                    session.sim.set_temperature_c(value);
                    session.refresh();
                    Ok(())
                }),
                Err(err) => self.error(err.to_string()),
            }
        }
        if let Some(result) = input_f32_row(ui, "距离 cm", &mut self.distance_input) {
            match result {
                Ok(value) => self.run_action(|session| {
                    session.sim.set_distance_cm(value);
                    session.refresh();
                    Ok(())
                }),
                Err(err) => self.error(err.to_string()),
            }
        }
        if let Some(result) = slider_f32_row(
            ui,
            "NE555 Hz",
            &mut self.frequency_input,
            0.0..=40_000.0,
            " Hz",
        ) {
            match result {
                Ok(value) => self.run_action(|session| {
                    session.sim.set_frequency_hz(value);
                    session.refresh();
                    Ok(())
                }),
                Err(err) => self.error(err.to_string()),
            }
        }
        if let Some(result) = slider_f32_row(ui, "光敏 RD1 V", &mut self.rd1_input, 0.0..=5.0, " V")
        {
            match result {
                Ok(value) => self.run_action(|session| {
                    session.sim.set_voltage_channel(VoltageChannel::Rd1, value);
                    session.refresh();
                    Ok(())
                }),
                Err(err) => self.error(err.to_string()),
            }
        }
        if let Some(result) = slider_f32_row(ui, "滑动 RB2 V", &mut self.rb2_input, 0.0..=5.0, " V")
        {
            match result {
                Ok(value) => self.run_action(|session| {
                    session.sim.set_voltage_channel(VoltageChannel::Rb2, value);
                    session.refresh();
                    Ok(())
                }),
                Err(err) => self.error(err.to_string()),
            }
        }

        ui.horizontal(|ui| {
            let installed = self.session.snapshot.jumper_net_sig_to_sig_out;
            ui.label("NET_SIG -> SIG_OUT");
            if ui
                .button(if installed {
                    "断开跳帽"
                } else {
                    "连接跳帽"
                })
                .clicked()
            {
                self.run_action(|session| {
                    if installed {
                        session.sim.jumper_off(SignalId::NetSig, SignalId::SigOut)?;
                    } else {
                        session.sim.jumper_on(SignalId::NetSig, SignalId::SigOut)?;
                    }
                    session.refresh();
                    Ok(())
                });
            }
        });

        ui.add_space(4.0);
        ui.heading("UART");
        if let Some(bytes) = uart_row(ui, "UART1", &mut self.uart1_input) {
            self.run_action(|session| {
                session.sim.uart1_write(&bytes)?;
                session.refresh();
                Ok(())
            });
        }
        if let Some(bytes) = uart_row(ui, "UART2", &mut self.uart2_input) {
            self.run_action(|session| {
                session.sim.uart2_write(&bytes)?;
                session.refresh();
                Ok(())
            });
        }

        ui.add_space(4.0);
        self.draw_repl_panel(ui);
    }

    fn set_key_pressed(&mut self, key: KeyId, pressed: bool) {
        self.run_action(|session| {
            session.sim.set_key_id(key, pressed);
            session.refresh();
            Ok(())
        });
    }

    fn sync_temporary_key_press(&mut self, active: Option<(KeyId, bool)>) -> bool {
        match (self.temporary_key_press, active) {
            (Some(current), Some((key, _))) if current.key == key => false,
            (Some(current), Some((key, restore_pressed))) => {
                self.set_key_pressed(current.key, current.restore_pressed);
                self.temporary_key_press = Some(TemporaryKeyPress {
                    key,
                    restore_pressed,
                });
                self.set_key_pressed(key, true);
                true
            }
            (None, Some((key, restore_pressed))) => {
                self.temporary_key_press = Some(TemporaryKeyPress {
                    key,
                    restore_pressed,
                });
                self.set_key_pressed(key, true);
                true
            }
            (Some(current), None) => {
                self.set_key_pressed(current.key, current.restore_pressed);
                self.temporary_key_press = None;
                true
            }
            (None, None) => false,
        }
    }

    fn draw_repl_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("REPL");
        let mut run_repl = false;
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let layout_job = highlight_rhai(ui, text.as_str(), wrap_width);
            ui.fonts_mut(|fonts| fonts.layout_job(layout_job))
        };
        ui.add(
            egui::TextEdit::multiline(&mut self.repl_input)
                .code_editor()
                .desired_width(ui.available_width())
                .desired_rows(4)
                .layouter(&mut layouter),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("执行").clicked() {
                run_repl = true;
            }
            if ui.button("重置 REPL").clicked() {
                self.session.repl.reset();
                self.repl_history.clear();
                self.notice("REPL 已重置");
            }
            if ui.button("清空输出").clicked() {
                self.repl_history.clear();
            }
        });
        if run_repl {
            self.run_repl_input();
        }

        egui::ScrollArea::vertical()
            .id_salt("debug-repl-history")
            .auto_shrink([false, false])
            .max_height(160.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.repl_history {
                    ui.monospace(line);
                }
            });
    }

    fn run_repl_input(&mut self) {
        let source = self.repl_input.clone();
        match self.session.eval_repl(&source) {
            Ok(lines) => {
                for line in lines {
                    self.push_repl_history(line);
                }
            }
            Err(err) => {
                let message = err.to_string();
                self.push_repl_history(format!("repl error: {message}"));
                self.error(message);
            }
        }
    }

    fn push_repl_history(&mut self, line: String) {
        self.repl_history.push(line);
        if self.repl_history.len() > 100 {
            let excess = self.repl_history.len().saturating_sub(100);
            self.repl_history.drain(0..excess);
        }
    }

    fn draw_judge_tab(&mut self, ui: &mut egui::Ui) {
        self.draw_judge_tab_content(ui);
    }

    fn draw_judge_tab_content(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("选择 HEX").clicked() {
                self.select_hex_file();
            }
            if ui.button("选择脚本").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Rhai", &["rhai"])
                    .pick_file()
            {
                match self.judge.load_script(path.clone()) {
                    Ok(()) => self.notice(format!("已加载脚本 {}", path.display())),
                    Err(err) => self.error(err.to_string()),
                }
            }
            if ui
                .add_enabled(!self.judge.running, egui::Button::new("运行评测"))
                .clicked()
            {
                self.start_judge_run();
            }
            if ui
                .add_enabled(
                    self.judge.running,
                    egui::Button::new(if self.judge.paused {
                        "继续"
                    } else {
                        "暂停"
                    }),
                )
                .clicked()
            {
                if self.judge.paused {
                    self.resume_judge_run();
                } else {
                    self.pause_judge_run();
                }
            }
            if ui
                .add_enabled(self.judge.running, egui::Button::new("终止"))
                .clicked()
            {
                self.terminate_judge_run();
            }
            if self.judge.running {
                ui.spinner();
            }
        });
        ui.label(path_label("HEX", self.session.hex_path.as_deref()));
        ui.label(path_label("脚本", self.judge.script_path.as_deref()));
        self.draw_progress(ui);
        ui.separator();
        let table_height = ui.available_height().max(180.0);
        draw_checkpoint_table(ui, &self.judge.rows, table_height);
        if let Some(error) = &self.judge.last_error {
            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
        }
    }

    fn draw_progress(&self, ui: &mut egui::Ui) {
        let total = self.judge.estimated_total;
        if total > 0 && self.judge.completed <= total {
            let progress = self.judge.completed as f32 / total as f32;
            let text = if self.judge.paused {
                format!("已暂停, 已完成 {}/{} 个评测点", self.judge.completed, total)
            } else {
                format!("已完成 {}/{} 个评测点", self.judge.completed, total)
            };
            ui.add(egui::ProgressBar::new(progress).text(text));
        } else if self.judge.running {
            ui.horizontal(|ui| {
                ui.spinner();
                if self.judge.paused {
                    ui.label(format!("已暂停, 已完成 {} 个评测点", self.judge.completed));
                } else {
                    ui.label(format!("已完成 {} 个评测点", self.judge.completed));
                }
            });
        } else {
            ui.label(format!("已完成 {} 个评测点", self.judge.completed));
        }
        if self.judge.current_step > 0 {
            ui.small(format!(
                "line {} step {} sim {} ns {}",
                self.judge.current_line,
                self.judge.current_step,
                self.judge.sim_time_ns,
                self.judge.phase
            ));
        }
    }

    fn draw_script_tab(&mut self, ui: &mut egui::Ui) {
        let script_editor_id = ui.make_persistent_id(SCRIPT_EDITOR_ID);
        ui.horizontal_wrapped(|ui| {
            if ui.button("选择脚本").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Rhai", &["rhai"])
                    .pick_file()
            {
                match self.judge.load_script(path.clone()) {
                    Ok(()) => self.notice(format!("已加载脚本 {}", path.display())),
                    Err(err) => self.error(err.to_string()),
                }
            }
            if ui
                .add_enabled(!self.judge.running, egui::Button::new("运行当前脚本"))
                .clicked()
            {
                self.start_judge_run();
            }
        });
        let panel_height = ui.available_height().max(240.0);
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let layout_job = highlight_rhai(ui, text.as_str(), wrap_width);
                    ui.fonts_mut(|fonts| fonts.layout_job(layout_job))
                };
                egui::ScrollArea::vertical()
                    .id_salt("script-editor-scroll")
                    .auto_shrink([false, false])
                    .max_height(panel_height)
                    .show(ui, |ui| {
                        let editor_width = ui.available_width().max(240.0);
                        ui.add(
                            egui::TextEdit::multiline(&mut self.judge.script_source)
                                .id(script_editor_id)
                                .code_editor()
                                .desired_width(editor_width)
                                .desired_rows(28)
                                .layouter(&mut layouter),
                        );
                    });
            });
            egui::ScrollArea::vertical()
                .id_salt("script-api-scroll")
                .auto_shrink([false, false])
                .max_height(panel_height)
                .show(&mut columns[1], |ui| {
                    ui.heading("评测 API");
                    for item in judge_api_catalog() {
                        ui.group(|ui| {
                            ui.strong(item.signature);
                            ui.label(item.description);
                            if ui.button("插入片段").clicked() {
                                insert_snippet_at_editor_cursor(
                                    ui.ctx(),
                                    script_editor_id,
                                    &mut self.judge.script_source,
                                    item.snippet,
                                );
                            }
                        });
                    }
                    ui.separator();
                    ui.label("常量: S4..S19, L1..L8, RD1, RB2, NET_SIG, SIG_OUT");
                });
        });
    }

    fn draw_wave_tab(&mut self, ui: &mut egui::Ui) {
        show_tab_scroll(ui, "wave-tab-scroll", |ui| {
            self.draw_wave_tab_content(ui);
        });
    }

    fn draw_wave_tab_content(&mut self, ui: &mut egui::Ui) {
        ui.label("波形配置会在重新加载 HEX 或启动评测时生效");
        if let Some(path) = wave_path_row(ui, "HTML", &mut self.wave_html_input) {
            self.session.wave.html_path = path;
        }
        if let Some(path) = wave_path_row(ui, "JSON", &mut self.wave_json_input) {
            self.session.wave.json_path = path;
        }
        if let Some(path) = wave_path_row(ui, "Msgpack", &mut self.wave_msgpack_input) {
            self.session.wave.msgpack_path = path;
        }
        ui.separator();
        ui.heading("导出窗口");
        ui.horizontal_wrapped(|ui| {
            ui.label("起始");
            ui.add_sized(
                [120.0, 28.0],
                egui::TextEdit::singleline(&mut self.wave_start_input),
            );
            ui.label("结束");
            ui.add_sized(
                [120.0, 28.0],
                egui::TextEdit::singleline(&mut self.wave_end_input).hint_text("留空为不限"),
            );
            if ui.button("应用时间窗口").clicked() {
                match parse_wave_window(&self.wave_start_input, &self.wave_end_input) {
                    Ok((start_ns, end_ns)) => {
                        self.session.wave.start_ns = start_ns;
                        self.session.wave.end_ns = end_ns;
                        self.notice("已应用波形时间窗口");
                    }
                    Err(err) => self.error(err),
                }
            }
            if ui.button("全程导出").clicked() {
                self.wave_start_input = "0".to_owned();
                self.wave_end_input.clear();
                self.session.wave.start_ns = 0;
                self.session.wave.end_ns = None;
                self.notice("已切换为全程波形导出");
            }
        });
        ui.small("时间支持 ns, us, ms, s 后缀, 例如 500us, 20ms, 1.5s");
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button("应用并重新加载 HEX").clicked() {
                match self.apply_wave_window() {
                    Ok(()) => {
                        if self.session.hex_path.is_some() {
                            match self.session.reload() {
                                Ok(_) => self.notice("已用当前波形配置重新加载 HEX"),
                                Err(err) => self.error(err.to_string()),
                            }
                        } else {
                            self.notice("已应用波形配置, 加载 HEX 后生效");
                        }
                    }
                    Err(err) => self.error(err),
                }
            }
            if ui.button("清空全部导出路径").clicked() {
                self.wave_html_input.clear();
                self.wave_json_input.clear();
                self.wave_msgpack_input.clear();
                self.session.wave.html_path = None;
                self.session.wave.json_path = None;
                self.session.wave.msgpack_path = None;
                self.notice("已关闭波形文件导出");
            }
        });
        ui.separator();
        let status = if self.session.wave.enabled() {
            "已启用"
        } else {
            "未启用"
        };
        ui.label(format!("导出状态: {status}"));
        ui.label(path_label("HTML", self.session.wave.html_path.as_deref()));
        ui.label(path_label("JSON", self.session.wave.json_path.as_deref()));
        ui.label(path_label(
            "Msgpack",
            self.session.wave.msgpack_path.as_deref(),
        ));
        ui.label(format!("起始: {} ns", self.session.wave.start_ns));
        match self.session.wave.end_ns {
            Some(end_ns) => ui.label(format!("结束: {end_ns} ns")),
            None => ui.label("结束: 不限"),
        };
    }

    fn apply_wave_window(&mut self) -> std::result::Result<(), String> {
        let (start_ns, end_ns) = parse_wave_window(&self.wave_start_input, &self.wave_end_input)?;
        self.session.wave.start_ns = start_ns;
        self.session.wave.end_ns = end_ns;
        Ok(())
    }
}

fn parse_wave_window(
    start_text: &str,
    end_text: &str,
) -> std::result::Result<(u64, Option<u64>), String> {
    let start_ns = parse_time_ns(start_text)?;
    let end_ns = if end_text.trim().is_empty() {
        None
    } else {
        Some(parse_time_ns(end_text)?)
    };
    if let Some(end_ns) = end_ns
        && end_ns < start_ns
    {
        return Err("波形结束时间不能小于起始时间".to_owned());
    }
    Ok((start_ns, end_ns))
}

fn parse_time_ns(value: &str) -> std::result::Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("时间参数不能为空".to_owned());
    }
    let lowered = trimmed.to_ascii_lowercase();
    for (suffix, scale) in [
        ("ns", 1_u64),
        ("us", 1_000_u64),
        ("ms", 1_000_000_u64),
        ("s", 1_000_000_000_u64),
    ] {
        if let Some(number_part) = lowered.strip_suffix(suffix) {
            return parse_time_number(number_part, scale, trimmed);
        }
    }
    parse_time_number(&lowered, 1, trimmed)
}

fn parse_time_number(
    number_part: &str,
    scale_ns: u64,
    original: &str,
) -> std::result::Result<u64, String> {
    let normalized = number_part.replace('_', "");
    if normalized.is_empty() {
        return Err(format!("时间参数缺少数值: {original}"));
    }
    if normalized.matches('.').count() > 1 {
        return Err(format!("时间参数格式错误: {original}"));
    }
    if let Some((integer_part, fraction_part)) = normalized.split_once('.') {
        return parse_fractional_time(integer_part, fraction_part, scale_ns, original);
    }
    if !normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("时间参数格式错误: {original}"));
    }
    let value = normalized
        .parse::<u128>()
        .map_err(|_| format!("时间参数数值过大: {original}"))?;
    let total = value
        .checked_mul(u128::from(scale_ns))
        .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
    u64::try_from(total).map_err(|_| format!("时间参数数值过大: {original}"))
}

fn parse_fractional_time(
    integer_part: &str,
    fraction_part: &str,
    scale_ns: u64,
    original: &str,
) -> std::result::Result<u64, String> {
    if fraction_part.is_empty() {
        return Err(format!("时间参数小数点后不能为空: {original}"));
    }
    if !integer_part.is_empty() && !integer_part.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("时间参数格式错误: {original}"));
    }
    if !fraction_part.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("时间参数格式错误: {original}"));
    }
    let integer = if integer_part.is_empty() {
        0_u128
    } else {
        integer_part
            .parse::<u128>()
            .map_err(|_| format!("时间参数数值过大: {original}"))?
    };
    let fraction = fraction_part
        .parse::<u128>()
        .map_err(|_| format!("时间参数数值过大: {original}"))?;
    let denominator = 10_u128
        .checked_pow(fraction_part.len() as u32)
        .ok_or_else(|| format!("时间参数小数位过多: {original}"))?;
    let scale = u128::from(scale_ns);
    let integer_ns = integer
        .checked_mul(scale)
        .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
    let fraction_scaled = fraction
        .checked_mul(scale)
        .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
    if fraction_scaled % denominator != 0 {
        return Err(format!("时间参数精度不能小于 1ns: {original}"));
    }
    let total = integer_ns
        .checked_add(fraction_scaled / denominator)
        .ok_or_else(|| format!("时间参数数值过大: {original}"))?;
    u64::try_from(total).map_err(|_| format!("时间参数数值过大: {original}"))
}

fn key_snapshot_index(key: KeyId) -> usize {
    let (row, column) = key.matrix_position();
    column * 4 + (3 - row)
}

impl eframe::App for StcjudgeGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if let Err(err) = self
            .session
            .run_for_ui_frame(self.sim_speed_limit_multiplier)
        {
            self.session.running = false;
            self.log(err.to_string());
        }
        self.poll_judge_events();
        egui::Frame::central_panel(ui.style())
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                self.draw_top_bar(ui);
                ui.separator();
                match self.tab {
                    AppTab::Debug => self.draw_debug_tab(ui),
                    AppTab::Judge => self.draw_judge_tab(ui),
                    AppTab::Script => self.draw_script_tab(ui),
                    AppTab::Wave => self.draw_wave_tab(ui),
                }
            });
        self.draw_feedback_toasts(ui.ctx());
        if self.session.running || self.judge.running || !self.feedbacks.is_empty() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn feedback_color(kind: UiFeedbackKind) -> egui::Color32 {
    match kind {
        UiFeedbackKind::Info => egui::Color32::from_rgb(42, 150, 95),
        UiFeedbackKind::Error => egui::Color32::from_rgb(180, 55, 55),
    }
}

fn sink_event_failed(report: stcjudge::ScriptRunReport, sink: ScriptRunEventSink) -> Result<()> {
    sink.emit(ScriptRunEvent::Failed(report));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wave_window_accepts_units() {
        assert_eq!(
            parse_wave_window("1us", "2ms").unwrap(),
            (1_000, Some(2_000_000))
        );
        assert_eq!(
            parse_wave_window("1.5s", "").unwrap(),
            (1_500_000_000, None)
        );
    }

    #[test]
    fn parse_wave_window_rejects_invalid_range() {
        let err = parse_wave_window("2ms", "1ms").unwrap_err();
        assert!(err.contains("结束时间"));
    }
}
