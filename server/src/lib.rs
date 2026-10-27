//! Platform-independent adaptive policy for the Memory Sentry server.
//!
//! This module intentionally contains no Win32 side effects. The policy can be
//! unit-tested on any CI runner before Windows process manipulation is enabled.

pub const MIN_CAP_MB: u32 = 12;
pub const MAX_CAP_MB: u32 = 64;
pub const INCREASE_STEP_MB: u32 = 8;
pub const DECREASE_STEP_MB: u32 = 4;
pub const HIGH_CPU_PERCENT: f64 = 10.0;
pub const LOW_CPU_PERCENT: f64 = 3.0;
pub const HIGH_CPU_DURATION_MS: u64 = 6_000;
pub const LOW_CPU_DURATION_MS: u64 = 10_000;
pub const ADJUSTMENT_COOLDOWN_MS: u64 = 8_000;
pub const LOW_WORKING_SET_MARGIN_MB: f64 = 6.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessRole {
    Main,
    Gpu,
    Renderer,
    Utility,
    Crashpad,
    Helper,
    Audio,
}

impl ProcessRole {
    /// Default ceiling offered by the per-process manual policy.
    pub fn manual_default_cap_mb(self) -> u32 {
        match self {
            Self::Main | Self::Audio => 36,
            Self::Gpu | Self::Renderer => 28,
            Self::Utility | Self::Helper => 24,
            Self::Crashpad => 16,
        }
    }

    /// Starting automatic ceiling. Audio is fixed at its baseline and is not adaptive.
    pub fn baseline_cap_mb(self) -> Option<u32> {
        match self {
            Self::Main | Self::Audio => Some(36),
            Self::Gpu | Self::Renderer => Some(28),
            Self::Utility | Self::Helper => Some(24),
            Self::Crashpad => Some(16),
        }
    }

    pub fn minimum_cap_mb(self) -> Option<u32> {
        match self {
            Self::Audio => None,
            Self::Main => Some(24),
            Self::Gpu | Self::Helper => Some(16),
            Self::Renderer | Self::Utility | Self::Crashpad => Some(12),
        }
    }

    pub fn maximum_cap_mb(self) -> Option<u32> {
        match self {
            Self::Main => Some(36),
            Self::Audio => None,
            _ => self
                .baseline_cap_mb()
                .map(|base| MAX_CAP_MB.min(base.saturating_add(24))),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PolicySample {
    pub role: ProcessRole,
    pub current_cap_mb: u32,
    pub working_set_mb: f64,
    pub cpu_percent: f64,
    pub high_cpu_ms: u64,
    pub low_cpu_ms: u64,
    pub last_adjustment_ms: u64,
    pub now_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdjustmentDirection {
    Increase,
    Decrease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdjustmentReason {
    SustainedHighCpu,
    LowCpuAndMemoryHeadroom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Adjustment {
    pub next_cap_mb: u32,
    pub direction: AdjustmentDirection,
    pub reason: AdjustmentReason,
}

pub fn estimate_cpu_percent(
    previous_cpu_ms: u64,
    current_cpu_ms: u64,
    elapsed_ms: u64,
) -> Option<f64> {
    if elapsed_ms == 0 || current_cpu_ms < previous_cpu_ms {
        return None;
    }
    let elapsed_cpu = current_cpu_ms.saturating_sub(previous_cpu_ms) as f64;
    Some(((elapsed_cpu / elapsed_ms as f64) * 100.0).clamp(0.0, 100.0))
}

pub fn working_set_at_ceiling(working_set_mb: f64, ceiling_mb: u32) -> bool {
    working_set_mb.is_finite() && working_set_mb >= ceiling_mb as f64
}

/// Produces an advisory next ceiling only. It does not call any Windows API.
pub fn decide_adjustment(sample: PolicySample) -> Option<Adjustment> {
    let minimum = sample.role.minimum_cap_mb()?;
    let maximum = sample.role.maximum_cap_mb()?;

    if !sample.working_set_mb.is_finite()
        || sample.working_set_mb < 0.0
        || !sample.cpu_percent.is_finite()
        || !(0.0..=100.0).contains(&sample.cpu_percent)
        || sample.current_cap_mb < minimum
        || sample.current_cap_mb > maximum
        || sample.now_ms.saturating_sub(sample.last_adjustment_ms) < ADJUSTMENT_COOLDOWN_MS
    {
        return None;
    }

    if sample.cpu_percent >= HIGH_CPU_PERCENT
        && sample.high_cpu_ms >= HIGH_CPU_DURATION_MS
        && working_set_at_ceiling(sample.working_set_mb, sample.current_cap_mb)
    {
        let next = maximum.min(sample.current_cap_mb.saturating_add(INCREASE_STEP_MB));
        return (next > sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Increase,
            reason: AdjustmentReason::SustainedHighCpu,
        });
    }

    if sample.cpu_percent <= LOW_CPU_PERCENT
        && sample.low_cpu_ms >= LOW_CPU_DURATION_MS
        && sample.working_set_mb <= sample.current_cap_mb as f64 - LOW_WORKING_SET_MARGIN_MB
    {
        let next = minimum.max(sample.current_cap_mb.saturating_sub(DECREASE_STEP_MB));
        return (next < sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Decrease,
            reason: AdjustmentReason::LowCpuAndMemoryHeadroom,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(role: ProcessRole) -> PolicySample {
        PolicySample {
            role,
            current_cap_mb: role.baseline_cap_mb().unwrap_or(24),
            working_set_mb: 18.0,
            cpu_percent: 1.0,
            high_cpu_ms: 0,
            low_cpu_ms: 12_000,
            last_adjustment_ms: 0,
            now_ms: 20_000,
        }
    }

    #[test]
    fn main_baseline_and_floor_are_preserved() {
        assert_eq!(ProcessRole::Main.baseline_cap_mb(), Some(36));
        assert_eq!(ProcessRole::Main.minimum_cap_mb(), Some(24));
        assert_eq!(ProcessRole::Main.maximum_cap_mb(), Some(36));
    }

    #[test]
    fn sustained_cpu_increases_ceiling_by_eight() {
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 28;
        s.working_set_mb = 28.0;
        s.cpu_percent = 18.0;
        s.high_cpu_ms = 7_000;
        let decision = decide_adjustment(s).expect("expected an increase");
        assert_eq!(decision.next_cap_mb, 36);
        assert_eq!(decision.reason, AdjustmentReason::SustainedHighCpu);
    }

    #[test]
    fn high_cpu_below_the_working_set_ceiling_does_not_raise_the_limit() {
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 28;
        s.working_set_mb = 27.9;
        s.cpu_percent = 18.0;
        s.high_cpu_ms = 20_000;
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn working_set_must_reach_the_full_ceiling() {
        assert!(!working_set_at_ceiling(27.99, 28));
        assert!(working_set_at_ceiling(28.0, 28));
        assert!(!working_set_at_ceiling(f64::NAN, 28));
    }

    #[test]
    fn main_ceiling_cannot_grow_above_thirty_six_megabytes() {
        let mut s = sample(ProcessRole::Main);
        s.cpu_percent = 50.0;
        s.high_cpu_ms = 20_000;
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn quiet_process_with_headroom_decreases_ceiling_by_four() {
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 28;
        s.working_set_mb = 15.0;
        let decision = decide_adjustment(s).expect("expected a decrease");
        assert_eq!(decision.next_cap_mb, 24);
        assert_eq!(decision.reason, AdjustmentReason::LowCpuAndMemoryHeadroom);
    }

    #[test]
    fn audio_has_a_fixed_automatic_ceiling_and_is_not_adaptive() {
        let mut s = sample(ProcessRole::Audio);
        s.cpu_percent = 50.0;
        s.high_cpu_ms = 20_000;
        assert_eq!(ProcessRole::Audio.baseline_cap_mb(), Some(36));
        assert_eq!(ProcessRole::Audio.minimum_cap_mb(), None);
        assert_eq!(ProcessRole::Audio.maximum_cap_mb(), None);
        assert_eq!(decide_adjustment(s), None);
        assert_eq!(ProcessRole::Audio.manual_default_cap_mb(), 36);
    }

    #[test]
    fn main_and_audio_manual_defaults_are_thirty_six_megabytes() {
        assert_eq!(ProcessRole::Main.manual_default_cap_mb(), 36);
        assert_eq!(ProcessRole::Audio.manual_default_cap_mb(), 36);
    }

    #[test]
    fn cooldown_and_invalid_cpu_samples_do_not_adjust() {
        let mut s = sample(ProcessRole::Renderer);
        s.last_adjustment_ms = 19_000;
        assert_eq!(decide_adjustment(s), None);
        s.last_adjustment_ms = 0;
        s.cpu_percent = f64::NAN;
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn main_process_cannot_fall_below_twenty_four_megabytes() {
        let mut s = sample(ProcessRole::Main);
        s.current_cap_mb = 28;
        s.working_set_mb = 10.0;
        assert_eq!(decide_adjustment(s).map(|d| d.next_cap_mb), Some(24));
    }

    #[test]
    fn cpu_estimation_rejects_invalid_intervals_and_caps_at_one_core() {
        assert_eq!(estimate_cpu_percent(10, 20, 0), None);
        assert_eq!(estimate_cpu_percent(20, 10, 100), None);
        assert_eq!(estimate_cpu_percent(0, 4_000, 1_000), Some(100.0));
        assert_eq!(estimate_cpu_percent(1_000, 1_250, 1_000), Some(25.0));
    }
}
