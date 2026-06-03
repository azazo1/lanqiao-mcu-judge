use std::collections::BTreeSet;

use anyhow::{Result, bail};
use tracing::debug;

use crate::ids::SignalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineDrive {
    HighZ,
    PullHigh,
    DriveHigh,
    DriveLow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineResolution {
    pub(crate) level: bool,
    pub(crate) conflict: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct JumperCap {
    left: SignalId,
    right: SignalId,
}

impl JumperCap {
    fn new(left: SignalId, right: SignalId) -> Self {
        if left <= right {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct BoardJumpers {
    installed: BTreeSet<JumperCap>,
}

impl BoardJumpers {
    pub(crate) fn install(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        if left == right {
            bail!("不能把跳帽扣在同一个信号上: {}", left.as_str());
        }
        let inserted = self.installed.insert(JumperCap::new(left, right));
        debug!(
            from = left.as_str(),
            to = right.as_str(),
            inserted,
            "更新板级跳帽"
        );
        Ok(())
    }

    pub(crate) fn remove(&mut self, left: SignalId, right: SignalId) -> Result<()> {
        if left == right {
            bail!("不能从同一个信号上拆跳帽: {}", left.as_str());
        }
        let removed = self.installed.remove(&JumperCap::new(left, right));
        debug!(
            from = left.as_str(),
            to = right.as_str(),
            removed,
            "更新板级跳帽"
        );
        Ok(())
    }

    pub(crate) fn is_installed(&self, left: SignalId, right: SignalId) -> bool {
        if left == right {
            return true;
        }
        self.installed.contains(&JumperCap::new(left, right))
    }

    pub(crate) fn describe(&self) -> String {
        if self.installed.is_empty() {
            return "none".to_string();
        }
        self.installed
            .iter()
            .map(|cap| format!("{}<->{}", cap.left.as_str(), cap.right.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub(crate) fn resolve_line(drives: &[LineDrive]) -> LineResolution {
    let mut has_pull_high = false;
    let mut has_drive_high = false;
    let mut has_drive_low = false;

    for drive in drives {
        match drive {
            LineDrive::HighZ => {}
            LineDrive::PullHigh => has_pull_high = true,
            LineDrive::DriveHigh => has_drive_high = true,
            LineDrive::DriveLow => has_drive_low = true,
        }
    }

    if has_drive_low && has_drive_high {
        return LineResolution {
            level: false,
            conflict: true,
        };
    }

    if has_drive_low {
        return LineResolution {
            level: false,
            conflict: false,
        };
    }

    if has_drive_high || has_pull_high {
        return LineResolution {
            level: true,
            conflict: false,
        };
    }

    LineResolution {
        level: false,
        conflict: false,
    }
}
