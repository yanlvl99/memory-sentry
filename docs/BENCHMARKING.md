# Benchmark protocol

The aim is to determine whether lower resident memory is worth any extra interaction delay. Do not compare runs taken under different workloads.

## Repeatable A/B test

1. Start Spotify and keep the same game, browser tabs, and background apps open throughout the test.
2. Record the dashboard power state, automatic or manual mode, selected background delay, and release mode. Enable **Debug mode** to capture per-process CPU estimates and adaptive ceilings.
3. Minimize Spotify for the selected delay plus at least 60 seconds, then restore it to the foreground. Measure from window focus until the first successful click/keyboard action. Record whether playback continued, whether the UI reloaded, whether the same page/view and state returned, and whether a temporary black screen appeared during restore. Repeat five times on each test PC.
4. Repeat with another workload and again after a fresh Spotify start. Do not change code or settings between comparable samples.
5. Compare median restore-to-interaction time, working-set reduction while minimized, page-fault deltas, CPU estimates, playback interruptions, UI reload/state loss, black-screen recovery, and whether the adaptive policy raises or lowers ceilings.

## Interpreting metrics

- **Working set:** pages currently resident in physical memory for a process. Summing process working sets may double-count shared pages. A falling working set does not mean the pages were freed or decommitted.
- **Private memory / commit charge:** private committed process memory as reported by Windows process metadata; it is not the same as resident RAM. Working-set trimming should not reduce this value, so record it separately to prove that results are not just moved memory.
- **Pagefile usage:** the commit charge attributed to a process, not the amount of data currently written to `pagefile.sys`. Trimming can leave this unchanged; dirty private pages may be written to the pagefile later when Windows reclaims their physical frames.
- **Trimmed/standby pages:** pages removed from a working set can remain cached in RAM on the standby or transition lists and may return through a soft page fault. They are not automatically "swapped out".
- **CPU percentage:** an estimate based on process CPU-time deltas normalized to one logical processor, so 100% means one logical CPU fully busy. Short bursts may be averaged or missed by the sample interval.
- **Page-fault delta/rate:** changes in cumulative Windows page-fault counters; includes soft and hard faults and is not a direct disk-read counter.
- **Restore-operation duration:** time spent restoring original Windows limits. It does not measure when Spotify accepts input, and it cannot establish that page faults caused any observed delay.
- **Restore-to-interaction time:** manually measured end-to-end recovery from window focus until the UI accepts input.

The adaptive policy raises a ceiling only when the process remains at its current working-set ceiling while also sustaining high CPU for six seconds. High CPU while working-set memory remains below the ceiling does not trigger an increase. It lowers the ceiling only when CPU is low and the process has working-set headroom. These thresholds are heuristics; data from real Windows systems should guide future changes. Any audio interruption, persistent hang, or repeated long recovery is a reason to turn **Memory Sentry off** with the dashboard power switch. Turning off only **Automatic policy** selects manual mode and does not stop background limits.

When comparing results, report working set, private/commit memory, pagefile usage, and page-fault deltas together. A lower working set with unchanged commit memory is the expected outcome of working-set limiting, not proof that the memory was released. Conversely, a large rise in hard faults or pagefile I/O suggests the limit is too aggressive for that PC or workload.
