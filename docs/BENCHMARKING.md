# Benchmark protocol

The aim is to determine whether lower resident memory is worth any extra interaction delay. Do not compare runs taken under different workloads.

## Repeatable A/B test

1. Start Spotify and keep the same game, browser tabs, and background apps open throughout the test.
2. Record the dashboard mode (Custom or Disabled), selected background delay, and release mode. Enable **Debug mode** to capture per-process CPU estimates and adaptive ceilings.
3. Minimize Spotify for the selected delay plus at least 60 seconds, then restore it to the foreground. Measure from window focus until the first successful click/keyboard action. Record whether playback continued, whether the UI reloaded, whether the same page/view and state returned, and whether a temporary black screen appeared during restore. Repeat five times on each test PC.
4. Repeat with another workload and again after a fresh Spotify start. Do not change code or settings between comparable samples.
5. Compare median restore-to-interaction time, working-set reduction while minimized, page-fault deltas, CPU estimates, playback interruptions, UI reload/state loss, black-screen recovery, and whether the adaptive policy raises or lowers ceilings.

## Interpreting metrics

- **Working set:** pages currently resident in physical memory for a process. Summing process working sets may double-count shared pages. A falling working set does not mean the pages were freed or decommitted.
- **Private working set:** private pages currently resident in physical RAM. This differs from private committed bytes and is shown by the dashboard when Windows provides the counter.
- **Commit size / commit charge:** committed virtual memory attributed to the process. Working-set trimming should not directly reduce this value; record it separately from resident memory.
- **Pagefile activity:** commit size does not indicate how many bytes are currently written to `pagefile.sys`. Under memory pressure, Windows may write modified private pages to the pagefile; use separate Windows performance counters or tools to measure pagefile I/O.
- **Trimmed/standby pages:** pages removed from a working set can remain cached in RAM on the standby or transition lists and may return through a soft page fault. They are not automatically "swapped out".
- **CPU percentage:** an estimate based on process CPU-time deltas normalized to one logical processor, so 100% means one logical CPU fully busy. Short bursts may be averaged or missed by the sample interval.
- **Page-fault delta/rate:** changes in cumulative Windows page-fault counters; includes soft and hard faults and is not a direct disk-read counter.
- **Restore-operation duration:** time spent restoring original Windows limits. It does not measure when Spotify accepts input, and it cannot establish that page faults caused any observed delay.
- **Restore-to-interaction time:** manually measured end-to-end recovery from window focus until the UI accepts input.

The engine sizes each process from a decaying histogram of its recent working set: the target is a high percentile of that demand plus a margin set by the learned usage bucket and by the system memory pressure Windows reports. A change happens only when the target passes a dead band, at most once every eight seconds. A capped process that faults heavily per resident megabyte, or that burns CPU while holding a small share of its ceiling, gets a relief increase and that ceiling becomes the identity's learned safe floor. Every role starts at 38 MB with a 4 MB absolute floor; Audio holds 25 MB or more and a renderer holds 38 MB while the window is visible in the background. Role maxima gain headroom on machines with more than 16 GB of RAM. These thresholds are heuristics; data from real Windows systems should guide future changes. Any audio interruption, persistent hang, or repeated long recovery is a reason to switch the dashboard to **Disabled**, which stops new limits and restores the recorded originals. Pinning a single process to **Manual** only fixes that process's ceiling and does not stop the rest of the engine.

When comparing results, report working set, private working set, commit size, pagefile I/O, and page-fault deltas as separate metrics. A lower working set with unchanged commit memory is the expected outcome of working-set limiting, not proof that the memory was released. Conversely, a large rise in hard faults or pagefile I/O suggests the limit is too aggressive for that PC or workload.
