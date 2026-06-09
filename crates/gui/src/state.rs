use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use stcjudge::{
    BoardSnapshot, CheckpointRecord, ScriptReplSession, ScriptRunControl, ScriptRunEvent,
    Simulator, WaveCaptureOptions, estimate_checkpoint_total,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppTab {
    Debug,
    Judge,
    Script,
    Wave,
}

impl AppTab {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Debug => "调试台",
            Self::Judge => "评测运行",
            Self::Script => "脚本工作台",
            Self::Wave => "波形",
        }
    }
}

pub(crate) struct GuiSession {
    pub(crate) hex_path: Option<PathBuf>,
    pub(crate) sim: Simulator,
    pub(crate) snapshot: BoardSnapshot,
    pub(crate) running: bool,
    pub(crate) last_tick: Instant,
    pub(crate) trace_cpu: bool,
    pub(crate) wave: WaveSettings,
    pub(crate) repl: ScriptReplSession,
}

impl GuiSession {
    pub(crate) fn empty() -> Self {
        let sim = Simulator::nop(false);
        let snapshot = sim.snapshot();
        Self {
            hex_path: None,
            sim,
            snapshot,
            running: false,
            last_tick: Instant::now(),
            trace_cpu: false,
            wave: WaveSettings::default(),
            repl: ScriptReplSession::new(),
        }
    }

    pub(crate) fn wave_options(&self) -> WaveCaptureOptions {
        self.wave.options()
    }

    pub(crate) fn load_hex(&mut self, path: PathBuf) -> Result<()> {
        let sim = Simulator::from_hex_path_with_options(&path, self.trace_cpu, self.wave_options())
            .with_context(|| format!("加载 HEX 失败: {}", path.display()))?;
        self.hex_path = Some(path);
        self.sim = sim;
        self.repl.reset();
        self.running = false;
        self.refresh();
        Ok(())
    }

    pub(crate) fn reload(&mut self) -> Result<()> {
        let Some(path) = self.hex_path.clone() else {
            return Ok(());
        };
        self.load_hex(path)
    }

    pub(crate) fn refresh(&mut self) {
        self.snapshot = self.sim.snapshot();
    }

    pub(crate) fn eval_repl(&mut self, source: &str) -> Result<Vec<String>> {
        let output = self.repl.eval(&mut self.sim, source)?;
        self.refresh();

        let mut lines = Vec::new();
        lines.push(format!("repl:{} > {}", output.line_no, source.trim()));
        for print in output.prints {
            lines.push(format!("print: {print}"));
        }
        if let Some(value) = output.value {
            lines.push(format!("=> {value}"));
        }
        if lines.len() == 1 {
            lines.push("=> ok".to_owned());
        }
        Ok(lines)
    }

    pub(crate) fn run_for_ui_frame(&mut self) -> Result<()> {
        if !self.running {
            return Ok(());
        }
        let now = Instant::now();
        if now.duration_since(self.last_tick) < Duration::from_millis(16) {
            return Ok(());
        }
        self.last_tick = now;
        self.sim.run_ms(5)?;
        self.refresh();
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WaveSettings {
    pub(crate) html_path: Option<PathBuf>,
    pub(crate) json_path: Option<PathBuf>,
    pub(crate) msgpack_path: Option<PathBuf>,
    pub(crate) start_ns: u64,
    pub(crate) end_ns: Option<u64>,
}

impl WaveSettings {
    pub(crate) fn options(&self) -> WaveCaptureOptions {
        WaveCaptureOptions {
            html_path: self.html_path.clone(),
            json_path: self.json_path.clone(),
            msgpack_path: self.msgpack_path.clone(),
            start_ns: self.start_ns,
            end_ns: self.end_ns,
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.html_path.is_some() || self.json_path.is_some() || self.msgpack_path.is_some()
    }
}

#[derive(Debug, Default)]
pub(crate) struct JudgeState {
    pub(crate) script_path: Option<PathBuf>,
    pub(crate) script_source: String,
    pub(crate) running: bool,
    pub(crate) paused: bool,
    pub(crate) estimated_total: usize,
    pub(crate) completed: usize,
    pub(crate) phase: String,
    pub(crate) sim_time_ns: u64,
    pub(crate) current_line: usize,
    pub(crate) current_step: u64,
    pub(crate) rows: Vec<CheckpointRecord>,
    pub(crate) last_report: String,
    pub(crate) last_error: Option<String>,
    pub(crate) receiver: Option<Receiver<ScriptRunEvent>>,
    pub(crate) control: Option<ScriptRunControl>,
}

impl JudgeState {
    pub(crate) fn load_script(&mut self, path: PathBuf) -> Result<()> {
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("读取脚本失败: {}", path.display()))?;
        self.estimated_total = estimate_checkpoint_total(&source);
        self.script_source = source;
        self.script_path = Some(path);
        Ok(())
    }

    pub(crate) fn reset_run_state(&mut self) {
        self.running = false;
        self.paused = false;
        self.completed = 0;
        self.phase.clear();
        self.sim_time_ns = 0;
        self.current_line = 0;
        self.current_step = 0;
        self.rows.clear();
        self.last_report.clear();
        self.last_error = None;
        self.receiver = None;
        self.control = None;
    }

    pub(crate) fn update_from_event(&mut self, event: ScriptRunEvent) {
        match event {
            ScriptRunEvent::Started {
                estimated_total_ckpts,
                ..
            } => {
                self.running = true;
                self.paused = false;
                self.estimated_total = estimated_total_ckpts;
                self.completed = 0;
                self.rows.clear();
                self.last_report.clear();
                self.last_error = None;
            }
            ScriptRunEvent::Paused => {
                self.paused = true;
            }
            ScriptRunEvent::Resumed => {
                self.paused = false;
            }
            ScriptRunEvent::Terminating => {
                self.paused = false;
            }
            ScriptRunEvent::Progress {
                phase,
                completed_ckpts,
                estimated_total_ckpts,
                sim_time_ns,
                line,
                step,
            } => {
                self.phase = phase;
                self.completed = completed_ckpts;
                self.estimated_total = estimated_total_ckpts;
                self.sim_time_ns = sim_time_ns;
                self.current_line = line;
                self.current_step = step;
            }
            ScriptRunEvent::CheckpointStarted(record) => {
                self.upsert_row(record);
            }
            ScriptRunEvent::CheckpointFinished(record) => {
                self.completed = self.completed.saturating_add(1);
                self.upsert_row(record);
            }
            ScriptRunEvent::Finished(report) => {
                self.running = false;
                self.paused = false;
                self.completed = report.total;
                self.rows = report.records;
                self.last_report = report.report;
                self.last_error = report.error;
                self.receiver = None;
                self.control = None;
            }
            ScriptRunEvent::Failed(report) => {
                self.running = false;
                self.paused = false;
                self.completed = report.total;
                self.rows = report.records;
                self.last_report = report.report;
                self.last_error = report.error;
                self.receiver = None;
                self.control = None;
            }
        }
    }

    fn upsert_row(&mut self, row: CheckpointRecord) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.row_id == row.row_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiFeedbackKind {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct UiFeedback {
    pub(crate) kind: UiFeedbackKind,
    pub(crate) message: String,
    created_at: Instant,
    ttl: Duration,
}

impl UiFeedback {
    pub(crate) fn new(kind: UiFeedbackKind, message: String) -> Self {
        let ttl = match kind {
            UiFeedbackKind::Info => Duration::from_secs(4),
            UiFeedbackKind::Error => Duration::from_secs(8),
        };
        Self {
            kind,
            message,
            created_at: Instant::now(),
            ttl,
        }
    }

    pub(crate) fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.created_at) >= self.ttl
    }
}
