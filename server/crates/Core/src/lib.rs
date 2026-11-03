//! Platform-independent adaptive policy for the Memory Sentry server.
//!
//! This module intentionally contains no Win32 side effects. The policy can be
//! unit-tested on any CI runner before Windows process manipulation is enabled.
//!
//! The process-targeting knowledge that differs per managed application is expressed through the
//! [`AppProfile`] trait here, while the concrete implementors (which executable, how command lines
//! map to roles) live in dedicated app crates. That keeps the policy, the Win32 layer and the
//! runtime reusable for any application.

use std::collections::BTreeSet;

/// Hard technical floor for an automatic ceiling. This is a safety bound, not a policy floor: the
/// per-identity learned safe floor normally sits above it, and the page-fault thrash guard is what
/// actually protects a starved process. Below a few megabytes a Chromium process only produces
/// faults, so this also keeps the engine out of degenerate caps.
pub const MIN_CAP_MB: u32 = 4;
/// Starting automatic ceiling, the same for every role. The first minutes after launch stay
/// generous (which is what prevents a freeze when the user returns to the window) and the engine
/// learns each identity's own safe edge from there.
pub const INITIAL_CAP_MB: u32 = 38;
pub const MAX_CAP_MB: u32 = 64;
pub const INCREASE_STEP_MB: u32 = 8;
pub const DECREASE_STEP_MB: u32 = 6;
pub const HIGH_CPU_PERCENT: f64 = 10.0;
pub const LOW_CPU_PERCENT: f64 = 3.0;
pub const HIGH_CPU_DURATION_MS: u64 = 6_000;
pub const LOW_CPU_DURATION_MS: u64 = 10_000;
pub const ADJUSTMENT_COOLDOWN_MS: u64 = 8_000;
pub const LOW_WORKING_SET_MARGIN_MB: f64 = 6.0;
/// How long a backgrounded process must stay quiet before the engine proactively squeezes it toward
/// its floor even without the usual working-set headroom (Chrome's idle-reclaim model).
pub const IDLE_PROACTIVE_MS: u64 = 60_000;
/// Page faults per second above which a capped process is judged to be starved (thrashing). The
/// engine then stops squeezing it and backs the ceiling off — the safety valve that lets the
/// aggressive background floors stay crash-free. Measured on real hardware (Windows 11, eight Spotify
/// processes): adaptive processes sitting at their ceiling in the background peak around 1,000
/// faults/s (Main p95 850, max 1,064; network p95 488), while a manual pin too tight for its renderer
/// sustains 3,500-8,500. This value sits in that gap on purpose: healthy caps never trip it, starved
/// ones do.
pub const THRASH_FAULTS_PER_SEC: f64 = 2_500.0;

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

    /// Starting automatic ceiling: uniform across roles, so a fresh process is never capped below a
    /// level that would force it to refault everything the moment the window comes back.
    pub fn baseline_cap_mb(self) -> Option<u32> {
        Some(INITIAL_CAP_MB)
    }

    /// Absolute floor shared by every role. Per-identity learning can raise it; nothing lowers it.
    pub fn minimum_cap_mb(self) -> Option<u32> {
        Some(MIN_CAP_MB)
    }

    pub fn maximum_cap_mb(self) -> Option<u32> {
        match self {
            Self::Main => Some(38),
            Self::Audio => Some(60),
            Self::Gpu | Self::Renderer => Some(52),
            Self::Utility | Self::Helper => Some(48),
            Self::Crashpad => Some(40),
        }
    }

    /// Relative importance used to decide WHICH role to trim first under memory pressure, modelled
    /// on Android's lmkd oom_adj tiers: a LOWER value is the least essential and
    /// is trimmed/reclaimed first, a HIGHER value is protected. Crashpad is pure housekeeping, the
    /// utility/helper processes are background services, audio and the paint pipeline (GPU/Renderer)
    /// and the browser Main drive what the user sees and hears, so they are released last. The
    /// runtime layer combines this with the live foreground/idle signals to pick reclaim order.
    pub fn trim_priority(self) -> u8 {
        match self {
            Self::Crashpad => 1,
            Self::Helper => 2,
            Self::Utility => 3,
            Self::Audio => 4,
            Self::Gpu => 5,
            Self::Renderer => 6,
            Self::Main => 7,
        }
    }

    /// Stable machine-readable role name. It is the prefix of every persisted cap/usage identity key
    /// and the label the status API reports, so it must stay byte-identical across versions. Local
    /// label only - never a PII signal.
    pub fn label(self) -> &'static str {
        match self {
            Self::Main => "Main",
            Self::Gpu => "GPU",
            Self::Renderer => "Renderer",
            Self::Utility => "Utility",
            Self::Crashpad => "Crashpad",
            Self::Helper => "Helper",
            Self::Audio => "Audio",
        }
    }

    /// Whether a string is one of the known role labels. Used to validate identity keys read back
    /// from the settings/backup files so a malformed or stale role cannot create a phantom cap.
    pub fn is_known_label(name: &str) -> bool {
        matches!(
            name,
            "Main" | "GPU" | "Renderer" | "Utility" | "Crashpad" | "Helper" | "Audio"
        )
    }
}

/// The application-specific knowledge the generic monitor needs to manage one process tree. Everything
/// that differs between the program being limited - its executable name, how its command lines map to
/// the Chromium-style [`ProcessRole`]s, and how to disambiguate sibling processes that share a role -
/// lives behind this trait. The core policy, the Win32 layer and the runtime therefore stay reusable:
/// supporting a new project means supplying a new implementor in its own crate and wiring it in one
/// place, without touching the engine.
pub trait AppProfile {
    /// Lowercased executable base names to enumerate (e.g. `["spotify.exe"]`).
    fn target_executables(&self) -> &'static [&'static str];

    /// The exact image name used to filter OS process queries (e.g. `"Spotify.exe"`).
    fn process_image_name(&self) -> &'static str;

    /// Whether a command line is a spawned child (helper) process rather than the app root. Used to
    /// pick the root of the process tree from the enumeration.
    fn is_child_process(&self, cmd: &str) -> bool;

    /// Classify a process into a role from its command line, its lineage and the live audio-session
    /// PIDs (which the OS reports independently of the command line).
    fn classify_role(
        &self,
        cmd: &str,
        pid: u32,
        root_pid: Option<u32>,
        audio_pids: &BTreeSet<u32>,
    ) -> ProcessRole;

    /// A PID-free sub-identity that disambiguates processes sharing a coarse role (e.g. two renderers,
    /// or network vs storage utilities), derived only from the already-captured command line.
    fn sub_label(&self, cmd: &str, role: ProcessRole) -> Option<String>;

    /// The persistence key for per-process caps: the role label plus the live sub-label when one exists
    /// (`Renderer:#7`, `Utility:network`), otherwise the bare role label (`Main`, `GPU`). Never a PID,
    /// which changes on every launch, so a cap survives a restart of the managed application.
    fn identity(&self, cmd: &str, role: ProcessRole) -> String {
        match self.sub_label(cmd, role) {
            Some(sub) => format!("{}:{}", role.label(), sub),
            None => role.label().to_string(),
        }
    }
}

/// Buckets a process identity by how busy it has recently been, borrowing Android's App Standby
/// Bucket model (developer.android.com/topic/performance/appstandby): four usage tiers that decide how
/// much headroom to reserve. The bucket NAMES mirror that model; the 40/20/8 percent cutoffs are this
/// project's own choice - nothing in the Android documentation fixes them - so treat them as tunable
/// heuristics. The signal here is local telemetry only (fraction of the sample window spent hot),
/// keyed by role + identity, NEVER a PID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageBucket {
    Active,
    WorkingSet,
    Frequent,
    Rare,
}

pub fn usage_bucket(busy_percent: f64) -> UsageBucket {
    if busy_percent >= 40.0 {
        UsageBucket::Active
    } else if busy_percent >= 20.0 {
        UsageBucket::WorkingSet
    } else if busy_percent >= 8.0 {
        UsageBucket::Frequent
    } else {
        UsageBucket::Rare
    }
}

impl UsageBucket {
    /// Extra MB kept above the role floor. Frequently-busy identities are protected from a full
    /// squeeze (crash-safe); rarely-busy ones keep none, so they get reclaimed to the last drop.
    pub fn floor_extra_mb(self) -> u32 {
        match self {
            UsageBucket::Active => 8,
            UsageBucket::WorkingSet => 4,
            UsageBucket::Frequent | UsageBucket::Rare => 0,
        }
    }

    /// Working-set headroom required before the engine lowers the ceiling. Rare processes tolerate a
    /// tighter margin (reclaim sooner); Active ones must show clear headroom before losing memory.
    pub fn decrease_margin_mb(self) -> f64 {
        match self {
            UsageBucket::Active => 8.0,
            UsageBucket::WorkingSet => 7.0,
            UsageBucket::Frequent => LOW_WORKING_SET_MARGIN_MB,
            UsageBucket::Rare => 3.0,
        }
    }

    /// Stable machine-readable name for surfacing the learned profile in the UI/status API. Local
    /// label only - never a PII signal, just which App-Standby-style bucket this identity fell in.
    pub fn label(self) -> &'static str {
        match self {
            UsageBucket::Active => "active",
            UsageBucket::WorkingSet => "working-set",
            UsageBucket::Frequent => "frequent",
            UsageBucket::Rare => "rare",
        }
    }
}

/// Blend a fresh per-window busy observation into a slow-moving cross-session score (0..=100), an
/// exponential weighted moving average. The weight is small so a one-off burst barely moves a
/// long-standing profile, yet a sustained habit change is tracked within a few dozen samples. The
/// first observation seeds the score directly (no history to average against). Keyed by process
/// identity, never a PID; strictly local telemetry (see PRIVACY.md).
pub fn usage_ewma(previous: Option<u32>, instant_busy: f64) -> u32 {
    const ALPHA: f64 = 0.1;
    let instant = instant_busy.clamp(0.0, 100.0);
    match previous {
        None => instant.round() as u32,
        Some(prior) => (prior as f64 * (1.0 - ALPHA) + instant * ALPHA)
            .clamp(0.0, 100.0)
            .round() as u32,
    }
}

/// Coarse installed-RAM tier. The engine scales how much headroom a process may grow into: a machine
/// with plenty of RAM can spare Spotify more (so it thrashes less), while a small machine keeps the
/// ceiling tight because every megabyte matters. Unknown total (0) is treated as the neutral Medium
/// tier so behavior is unchanged when the OS query is unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTier {
    Small,
    Medium,
    Large,
}

pub fn memory_tier(total_mb: u64) -> MemoryTier {
    if total_mb == 0 {
        MemoryTier::Medium
    } else if total_mb <= 8_192 {
        MemoryTier::Small
    } else if total_mb <= 16_384 {
        MemoryTier::Medium
    } else {
        MemoryTier::Large
    }
}

impl MemoryTier {
    /// Extra MB a process may grow above its role ceiling. Only large machines get more headroom;
    /// small and medium keep the tuned ceilings so the aggressive background floors stay crash-safe.
    pub fn maximum_bonus_mb(self) -> u32 {
        match self {
            MemoryTier::Small | MemoryTier::Medium => 0,
            MemoryTier::Large => 16,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PolicySample {
    pub role: ProcessRole,
    pub current_cap_mb: u32,
    pub working_set_mb: f64,
    pub cpu_percent: f64,
    /// Recent busy fraction (0..=100) of the sample window; the local usage-telemetry signal.
    pub busy_percent: f64,
    pub high_cpu_ms: u64,
    pub low_cpu_ms: u64,
    /// Page faults per second since the previous sample; the thrash signal for the safety valve.
    pub faults_per_sec: f64,
    /// Effective floor for this identity: the absolute minimum raised by the learned safe edge.
    pub minimum_cap_mb: u32,
    /// Total installed RAM in MB, used only to pick the headroom tier (0 == unknown -> Medium).
    pub system_total_mb: u64,
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
    ThrashRelief,
    IdleProactive,
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

/// Faults counted between two readings of the Windows counter. That counter is a 32-bit DWORD, so a
/// reading smaller than the previous one means it wrapped past its maximum; the delta is reconstructed
/// across that boundary instead of reading as a negative (and therefore huge) drop.
pub fn faults_since(previous_faults: u64, current_faults: u64) -> u64 {
    if current_faults >= previous_faults {
        current_faults - previous_faults
    } else {
        current_faults + (1u64 << 32) - previous_faults
    }
}

/// Faults per second for the window that actually separates two readings. Callers must pass the age of
/// the reading they divided against: the fast memory refresh and the slower process snapshot each
/// advance the shared baseline, so dividing by a nominal window dilutes the reading whenever the other
/// sampler moved the counter in between. `None` when the age is unknown, which callers treat as "no
/// fault pressure" rather than as zero.
pub fn estimate_faults_per_second(
    previous_faults: u64,
    current_faults: u64,
    elapsed_ms: u64,
) -> Option<f64> {
    if elapsed_ms == 0 {
        return None;
    }
    Some(faults_since(previous_faults, current_faults) as f64 * 1000.0 / elapsed_ms as f64)
}

pub fn working_set_at_ceiling(working_set_mb: f64, ceiling_mb: u32) -> bool {
    working_set_mb.is_finite() && working_set_mb >= ceiling_mb as f64
}

/// Produces an advisory next ceiling only. It does not call any Windows API.
pub fn decide_adjustment(sample: PolicySample) -> Option<Adjustment> {
    let role_min = sample.role.minimum_cap_mb()?;
    let role_base = sample.role.baseline_cap_mb().unwrap_or(MAX_CAP_MB);
    // The learned safe edge can only raise the absolute floor. Clamped to the role ceiling below so a
    // stale value cannot freeze the process.
    let floor = sample.minimum_cap_mb.max(role_min);
    // Headroom scales with installed RAM: a large machine lets a busy process grow higher (thrash less),
    // a small one keeps the ceiling tight. Unknown total -> Medium -> no bonus (unchanged behavior).
    let tier = memory_tier(sample.system_total_mb);
    let maximum = sample.role.maximum_cap_mb()?.saturating_add(tier.maximum_bonus_mb()).min(MAX_CAP_MB);

    if !sample.working_set_mb.is_finite()
        || sample.working_set_mb < 0.0
        || !sample.cpu_percent.is_finite()
        || !(0.0..=100.0).contains(&sample.cpu_percent)
        || !sample.faults_per_sec.is_finite()
        || sample.faults_per_sec < 0.0
        || !sample.busy_percent.is_finite()
        || !(0.0..=100.0).contains(&sample.busy_percent)
        || sample.current_cap_mb < role_min
        || sample.current_cap_mb > maximum
        || sample.now_ms.saturating_sub(sample.last_adjustment_ms) < ADJUSTMENT_COOLDOWN_MS
    {
        return None;
    }

    // A process pinned at its ceiling that keeps pulling pages back is being starved even when it is
    // not burning CPU. That is the exact edge of a working-set freeze, so we stop squeezing and back
    // the ceiling off. This is what lets the background floors go low without
    // crashing: the engine finds each process's own safe edge instead of us guessing a fixed number.
    let thrashing = sample.faults_per_sec >= THRASH_FAULTS_PER_SEC;
    let at_ceiling = working_set_at_ceiling(sample.working_set_mb, sample.current_cap_mb);

    // Personalize the squeeze from local usage telemetry (the four usage buckets above):
    // rarely-busy identities descend on a tighter margin toward the floor (last drop), while
    // frequently-busy ones keep headroom above the floor so they are never starved on return.
    let bucket = usage_bucket(sample.busy_percent);
    let effective_min = floor.saturating_add(bucket.floor_extra_mb()).min(role_base);
    let margin = bucket.decrease_margin_mb();

    if sample.cpu_percent >= HIGH_CPU_PERCENT
        && sample.high_cpu_ms >= HIGH_CPU_DURATION_MS
        && at_ceiling
    {
        let next = maximum.min(sample.current_cap_mb.saturating_add(INCREASE_STEP_MB));
        return (next > sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Increase,
            reason: AdjustmentReason::SustainedHighCpu,
        });
    }

    if thrashing && at_ceiling && sample.current_cap_mb < maximum {
        let next = maximum.min(sample.current_cap_mb.saturating_add(INCREASE_STEP_MB));
        return (next > sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Increase,
            reason: AdjustmentReason::ThrashRelief,
        });
    }

    if sample.cpu_percent <= LOW_CPU_PERCENT
        && sample.low_cpu_ms >= LOW_CPU_DURATION_MS
        && !thrashing
        && sample.working_set_mb <= sample.current_cap_mb as f64 - margin
    {
        let next = effective_min.max(sample.current_cap_mb.saturating_sub(DECREASE_STEP_MB));
        return (next < sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Decrease,
            reason: AdjustmentReason::LowCpuAndMemoryHeadroom,
        });
    }

    // Idle proactivity (Chrome model): a process left quiet for a long stretch is
    // squeezed toward its floor even when it does not show the usual headroom, as long as it is not
    // thrashing and is not pinned at the ceiling. This reclaims memory that backgrounded-but-untouched
    // processes keep reserved indefinitely.
    if sample.cpu_percent <= LOW_CPU_PERCENT
        && sample.low_cpu_ms >= IDLE_PROACTIVE_MS
        && !thrashing
        && !at_ceiling
    {
        let next = effective_min.max(sample.current_cap_mb.saturating_sub(DECREASE_STEP_MB));
        return (next < sample.current_cap_mb).then_some(Adjustment {
            next_cap_mb: next,
            direction: AdjustmentDirection::Decrease,
            reason: AdjustmentReason::IdleProactive,
        });
    }

    None
}

/// Maps the OS memory-load estimate + the free-RAM fraction to a coarse pressure level that drives
/// the adaptive engine (Windows has no PSI; dwMemoryLoad is the closest built-in signal, tempered by
/// available/total). Order of severity: critical > medium > low > none.
pub fn pressure_level(memory_load_percent: u32, available_mb: u64, total_mb: u64) -> &'static str {
    let load = memory_load_percent.min(100);
    let avail_frac = if total_mb > 0 {
        (available_mb as f64 / total_mb as f64) * 100.0
    } else {
        100.0
    };
    if load >= 90 || avail_frac <= 8.0 {
        "critical"
    } else if load >= 80 || avail_frac <= 15.0 {
        "medium"
    } else if load >= 65 || avail_frac <= 25.0 {
        "low"
    } else {
        "none"
    }
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
            busy_percent: 10.0,
            high_cpu_ms: 0,
            low_cpu_ms: 12_000,
            faults_per_sec: 0.0,
            minimum_cap_mb: MIN_CAP_MB,
            system_total_mb: 0,
            last_adjustment_ms: 0,
            now_ms: 20_000,
        }
    }

    #[test]
    fn every_role_starts_at_38_with_a_4_mb_absolute_floor() {
        for role in [
            ProcessRole::Main,
            ProcessRole::Audio,
            ProcessRole::Gpu,
            ProcessRole::Renderer,
            ProcessRole::Utility,
            ProcessRole::Helper,
            ProcessRole::Crashpad,
        ] {
            assert_eq!(role.baseline_cap_mb(), Some(38), "{role:?}");
            assert_eq!(role.minimum_cap_mb(), Some(4), "{role:?}");
        }
        assert_eq!(ProcessRole::Main.maximum_cap_mb(), Some(38));
        assert_eq!(ProcessRole::Audio.maximum_cap_mb(), Some(60));
        assert_eq!(ProcessRole::Renderer.maximum_cap_mb(), Some(52));
        assert_eq!(ProcessRole::Crashpad.maximum_cap_mb(), Some(40));
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
    fn fault_rate_divides_by_the_age_of_its_own_reading() {
        assert_eq!(estimate_faults_per_second(1_000, 3_000, 2_000), Some(1_000.0));
        assert_eq!(estimate_faults_per_second(0, 500, 1_000), Some(500.0));
        // The counter is 32-bit: a reading that wrapped still yields the faults that actually happened.
        assert_eq!(
            estimate_faults_per_second(u32::MAX as u64 - 99, 400, 1_000),
            Some(500.0)
        );
        assert_eq!(faults_since(7, 3), (1u64 << 32) - 4);
        assert_eq!(estimate_faults_per_second(1_000, 3_000, 0), None);
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
    fn quiet_process_with_headroom_decreases_ceiling_by_six() {
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 28;
        s.working_set_mb = 15.0;
        let decision = decide_adjustment(s).expect("expected a decrease");
        assert_eq!(decision.next_cap_mb, 22);
        assert_eq!(decision.reason, AdjustmentReason::LowCpuAndMemoryHeadroom);
    }

    #[test]
    fn a_starved_process_backs_off_even_without_high_cpu() {
        // Pinned at the ceiling and faulting heavily: the thrash valve raises the ceiling (safety).
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 12;
        s.working_set_mb = 12.0;
        s.cpu_percent = 1.0;
        s.low_cpu_ms = 12_000;
        s.faults_per_sec = 5_000.0;
        let decision = decide_adjustment(s).expect("thrash relief expected");
        assert_eq!(decision.next_cap_mb, 20);
        assert_eq!(decision.reason, AdjustmentReason::ThrashRelief);
        assert_eq!(decision.direction, AdjustmentDirection::Increase);
    }

    #[test]
    fn a_heavy_faulting_process_is_not_squeezed_further() {
        // Headroom exists, but a high fault rate means squeezing would tip it into a freeze.
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 24;
        s.working_set_mb = 15.0;
        s.cpu_percent = 1.0;
        s.low_cpu_ms = 12_000;
        s.faults_per_sec = 5_000.0;
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn usage_bucket_orders_by_recent_busyness() {
        assert_eq!(usage_bucket(55.0), UsageBucket::Active);
        assert_eq!(usage_bucket(25.0), UsageBucket::WorkingSet);
        assert_eq!(usage_bucket(10.0), UsageBucket::Frequent);
        assert_eq!(usage_bucket(2.0), UsageBucket::Rare);
    }

    #[test]
    fn a_learned_safe_floor_keeps_a_busy_process_from_descending() {
        // The learned safe edge raises the floor; with the Active headroom it can exceed the current
        // cap, so a hot process keeps its memory instead of being squeezed.
        let mut s = sample(ProcessRole::Main);
        s.current_cap_mb = 24;
        s.working_set_mb = 10.0;
        s.busy_percent = 60.0; // Active -> +8 headroom
        s.minimum_cap_mb = 20; // learned safe edge
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn a_rare_process_is_reclaimed_on_a_tighter_margin() {
        // A Frequent process leaves this sample alone; a Rare one squeezes it toward the floor.
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 20;
        s.working_set_mb = 17.0;
        s.busy_percent = 2.0; // Rare -> margin 3.0, 17 <= 20 - 3 -> descend toward floor 12
        let decision = decide_adjustment(s).expect("rare should descend");
        assert_eq!(decision.next_cap_mb, 14);
        assert_eq!(decision.direction, AdjustmentDirection::Decrease);
    }

    #[test]
    fn audio_adapts_upward_from_38_to_60_megabytes() {
        let mut s = sample(ProcessRole::Audio);
        s.working_set_mb = 38.0;
        s.cpu_percent = 50.0;
        s.high_cpu_ms = 20_000;
        assert_eq!(ProcessRole::Audio.baseline_cap_mb(), Some(38));
        assert_eq!(ProcessRole::Audio.minimum_cap_mb(), Some(4));
        assert_eq!(ProcessRole::Audio.maximum_cap_mb(), Some(60));
        let decision = decide_adjustment(s).expect("audio should adapt upward under load");
        assert_eq!(decision.next_cap_mb, 46);
        assert_eq!(decision.direction, AdjustmentDirection::Increase);
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
    fn an_idle_process_falls_to_the_absolute_floor_but_a_learned_floor_stops_it() {
        let mut s = sample(ProcessRole::Main);
        s.current_cap_mb = 10;
        s.working_set_mb = 2.0;
        assert_eq!(decide_adjustment(s).map(|d| d.next_cap_mb), Some(4));
        // The learned safe edge blocks the same descent.
        let mut learned = sample(ProcessRole::Main);
        learned.current_cap_mb = 10;
        learned.working_set_mb = 2.0;
        learned.minimum_cap_mb = 8;
        assert_eq!(decide_adjustment(learned).map(|d| d.next_cap_mb), Some(8));
    }

    #[test]
    fn cpu_estimation_rejects_invalid_intervals_and_caps_at_one_core() {
        assert_eq!(estimate_cpu_percent(10, 20, 0), None);
        assert_eq!(estimate_cpu_percent(20, 10, 100), None);
        assert_eq!(estimate_cpu_percent(0, 4_000, 1_000), Some(100.0));
        assert_eq!(estimate_cpu_percent(1_000, 1_250, 1_000), Some(25.0));
    }

    #[test]
    fn trim_priority_orders_housekeeping_first_and_visible_last() {
        // lmkd-style importance: least essential trims first, the visible/paint pipeline is protected.
        assert!(ProcessRole::Crashpad.trim_priority() < ProcessRole::Utility.trim_priority());
        assert!(ProcessRole::Utility.trim_priority() < ProcessRole::Audio.trim_priority());
        assert!(ProcessRole::Audio.trim_priority() < ProcessRole::Gpu.trim_priority());
        assert!(ProcessRole::Gpu.trim_priority() < ProcessRole::Renderer.trim_priority());
        assert!(ProcessRole::Renderer.trim_priority() < ProcessRole::Main.trim_priority());
    }

    #[test]
    fn pressure_level_rises_with_load_and_falls_with_free_ram() {
        assert_eq!(pressure_level(30, 8000, 16000), "none");
        assert_eq!(pressure_level(70, 4000, 16000), "low");
        assert_eq!(pressure_level(85, 2000, 16000), "medium");
        assert_eq!(pressure_level(95, 500, 16000), "critical");
        // Low free fraction (12.5%) escalates to medium even when the OS load estimate is modest.
        assert_eq!(pressure_level(50, 2000, 16000), "medium");
        // Unknown totals (0) are treated as no pressure rather than falsely critical.
        assert_eq!(pressure_level(0, 0, 0), "none");
    }

    #[test]
    fn memory_tier_uses_installed_ram_and_treats_unknown_as_medium() {
        assert_eq!(memory_tier(0), MemoryTier::Medium);
        assert_eq!(memory_tier(8_192), MemoryTier::Small);
        assert_eq!(memory_tier(8_193), MemoryTier::Medium);
        assert_eq!(memory_tier(16_384), MemoryTier::Medium);
        assert_eq!(memory_tier(32_768), MemoryTier::Large);
        assert_eq!(MemoryTier::Small.maximum_bonus_mb(), 0);
        assert_eq!(MemoryTier::Large.maximum_bonus_mb(), 16);
    }

    #[test]
    fn a_large_machine_gives_the_main_process_more_headroom() {
        // Main is capped at 36 normally; on a >16 GB machine it may grow to 52 under sustained load.
        let mut s = sample(ProcessRole::Main);
        s.current_cap_mb = 36;
        s.working_set_mb = 36.0;
        s.cpu_percent = 50.0;
        s.high_cpu_ms = 20_000;
        s.system_total_mb = 32_768; // Large
        let decision = decide_adjustment(s).expect("large machine should allow more headroom");
        assert_eq!(decision.next_cap_mb, 44);
        assert_eq!(decision.reason, AdjustmentReason::SustainedHighCpu);
    }

    #[test]
    fn a_deeply_idle_process_is_squeezed_without_headroom() {
        // Renderer at 20 MB using 19 MB shows no decrease margin, but after 60 s quiet it is reclaimed
        // one step toward its floor (idle proactivity), as long as it is not thrashing.
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 20;
        s.working_set_mb = 19.0;
        s.cpu_percent = 1.0;
        s.busy_percent = 2.0; // Rare
        s.low_cpu_ms = 70_000;
        let decision = decide_adjustment(s).expect("idle process should be proactively reclaimed");
        assert_eq!(decision.next_cap_mb, 14);
        assert_eq!(decision.reason, AdjustmentReason::IdleProactive);
        assert_eq!(decision.direction, AdjustmentDirection::Decrease);
    }

    #[test]
    fn a_thrashing_idle_process_is_not_proactively_squeezed() {
        let mut s = sample(ProcessRole::Renderer);
        s.current_cap_mb = 20;
        s.working_set_mb = 19.0;
        s.cpu_percent = 1.0;
        s.low_cpu_ms = 70_000;
        s.faults_per_sec = 5_000.0; // thrashing -> the safety valve wins over idle reclaim
        assert_eq!(decide_adjustment(s), None);
    }

    #[test]
    fn usage_ewma_seeds_then_moves_slowly_and_clamps() {
        // First observation seeds directly (no history to average against).
        assert_eq!(usage_ewma(None, 70.0), 70);
        // A single hot window barely moves a long-established quiet profile (alpha = 0.1).
        assert_eq!(usage_ewma(Some(10), 100.0), 19);
        // ...and a single idle window barely moves an active profile.
        assert_eq!(usage_ewma(Some(90), 0.0), 81);
        // Out-of-range instant inputs are clamped to 0..=100 before blending.
        assert_eq!(usage_ewma(Some(0), 500.0), 10);
        assert_eq!(usage_ewma(Some(100), -5.0), 90);
    }

    #[test]
    fn usage_bucket_labels_are_stable_and_monotonic() {
        assert_eq!(usage_bucket(45.0).label(), "active");
        assert_eq!(usage_bucket(25.0).label(), "working-set");
        assert_eq!(usage_bucket(12.0).label(), "frequent");
        assert_eq!(usage_bucket(3.0).label(), "rare");
    }

    #[test]
    fn role_labels_are_stable_keys_and_reject_unknown_roles() {
        // The label is the identity/cap key prefix, so it must never drift across versions.
        assert_eq!(ProcessRole::Main.label(), "Main");
        assert_eq!(ProcessRole::Gpu.label(), "GPU");
        assert_eq!(ProcessRole::Renderer.label(), "Renderer");
        assert_eq!(ProcessRole::Utility.label(), "Utility");
        assert_eq!(ProcessRole::Crashpad.label(), "Crashpad");
        assert_eq!(ProcessRole::Helper.label(), "Helper");
        assert_eq!(ProcessRole::Audio.label(), "Audio");
        for role in [
            ProcessRole::Main,
            ProcessRole::Gpu,
            ProcessRole::Renderer,
            ProcessRole::Utility,
            ProcessRole::Crashpad,
            ProcessRole::Helper,
            ProcessRole::Audio,
        ] {
            assert!(ProcessRole::is_known_label(role.label()));
        }
        assert!(!ProcessRole::is_known_label("Browser"));
        assert!(!ProcessRole::is_known_label(""));
    }
}
