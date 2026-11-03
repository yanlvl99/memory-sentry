# Memory Sentry debug console

A command-line harness that drives the local server and the Windows window manager so behaviour can
be verified by a script instead of by a person clicking through the app.

It exists because two kinds of claim needed proof that static checks cannot give: whether a window
state the server reads matches what Windows actually says, and whether a memory ceiling is really
enforced and really taken back. Grepping source can assert that code exists; it cannot assert that
the OS agrees.

## How it is built

Two rules shape this tool, and they are deliberate:

**The server is the source of truth for everything the server knows.** Window candidates, which
window represents the process, the classification of window state, per-process memory figures, cap
records, and arming or releasing a ceiling all come from the server's debug API. The harness does
not reimplement those rules. A second implementation of candidate filtering here would drift from
the product's, and then a comparison would be measuring the harness rather than the product.

**There is no native layer in the harness at all.** Driving another process's window (minimize,
restore, maximize, send to back/front, focus, blur) is a POST to the server's debug API, which runs
those `PostMessage`/`SetWindowPos`/`SetForegroundWindow` calls in its own Windows crate. The OS
foreground is read from that same view. The independent witness for a cap is still real, just at the
right level: `probe.osLimits` is the kernel's answer to `GetProcessWorkingSetSizeEx`, a readback that
is a different call from the `SetProcessWorkingSetSizeEx` that sets the ceiling - so "the server
recorded a cap" and "Windows is enforcing it" remain two separate facts, without a second FFI
implementation that could only ever test itself.

The tool is a plain Node HTTP client (Node 18 or newer): no native module to install, nothing to
compile, and no experimental `node:ffi` that can break on the next Node upgrade. All of the unsafe
Win32 lives in exactly one place - the server's `Windows` crate - reached only behind the feature
gate below.

## The debug API is compiled out of shipped builds

The routes under `/api/debug/` exist only in a server built with `--features debug-api`. The feature
is off by default, no CI job passes it, and `scripts/build-package.ps1` only appends it when invoked
with `-DebugApi`. A final user's server answers those routes with `404` and cannot be asked to move a
working-set limit, even with a valid local token.

That is verified, not assumed: `validate-package.js` fails if the feature enters the default set, if
a workflow passes `--features`, if a debug route loses its `cfg` attribute, or if the debug module
starts implementing its own cap policy instead of calling the engine's functions.

To get a server the console can drive:

```powershell
scripts/build-package.ps1 -DebugApi     # produces dist\...-x64-debug.msi, host architecture only
```

Install it, or swap the running server for the freshly built binary without touching the installer:

```powershell
cargo build --release --features debug-api --locked --target x86_64-pc-windows-msvc `
  --manifest-path server\Cargo.toml
server\target\x86_64-pc-windows-msvc\release\memory-sentry-server.exe --swap <same path>
```

Swap back to a plain build afterwards. A binary that can move memory limits should not stay
installed on a machine that is meant to be running the released product.

Every command refuses to continue with a clear message when the running server lacks the API, and
`GET /api/status` carries a `debugApi` flag so the difference is visible without probing.

## Commands

```
node Index.js status          one snapshot: stored state, live state, OS focus, cap counts
node Index.js windows         the server's window candidates and classification, as JSON
node Index.js compare         stored vs live vs OS truth, plus every disagreement between them
node Index.js ram             per-process memory: server figure beside the enforced-limit readback
node Index.js probe [--pid N] raw per-process figures, enforced limits, recovery records
node Index.js system          system totals, pressure level and the settings that gate them
node Index.js watch --seconds 30 [--interval 1000]
node Index.js force --identity NAME --mb 24    arm a ceiling through the product's own apply path
node Index.js release [--pid N | --all]        release it the same way
node Index.js empty --pid N                    trim that working set now
node Index.js beacon --focused true --visible true    post what the dashboard would post
node Index.js min | restore | max | focus | back | top | blur   [--hwnd 0x.. | --pid N]
node Index.js test smoke | bug2 | all          the automated scenarios
node Index.js benchmark [--pid N|--identity T] [--mb C] [--seconds N] [--interval MS]
```

The automated commands (`test`, `benchmark`) drive real windows and real caps on this machine, so run
them against a debug build you can restore, not a build you intend to keep installed.

`--settings <path>` points at a different server instance; everything else is read from the
`settings.json` the server owns (port and token are not duplicated here). Exit status is 0 when the
work finished, 1 when a step failed, 2 for an unknown command.

`force` arms a ceiling through the engine's own apply path, and the engine is allowed to hand it back:
in a window state this configuration does not manage (check `optimizeWhenBackground`,
`optimizeWhenMinimized` and `optimizeWhenHidden` with `node Index.js system`), the next pass restores
the original limits and `ram` shows the process uncapped again. Minimize first (`node Index.js min`)
when the point is to watch a ceiling hold. This was observed, not inferred: the same `force` command
left no ceiling behind while the window was visible-but-unfocused, and armed a durable one - reported
by Windows as a 24 MB enforced maximum - once it was minimized.

`compare` prints three readings of the same moment, all from the server. `stored` is what it last
decided, `live` is the server's rule applied to fresh candidates, and the per-process block is the
enforced limits read straight back from the kernel. A gap between the first two means a stale or
skipped read; a process whose recorded cap and kernel ceiling disagree means the limit never bound.

`ram` prints an `enforced==applied` column that answers the only question worth asking about a cap:
whether the ceiling the server recorded is the maximum Windows reports it is enforcing right now (a
separate readback call), and that an uncapped process has no hard maximum set.

## Scenarios

`node Index.js test all` runs both, and prints PASS / FAIL / INCONCLUSIVE per step with the values it
observed. Exit code is non-zero only on a FAIL. A timeout that depends on the machine reports
INCONCLUSIVE rather than failing, because "this environment could not produce the condition" is a
different fact from "the product is wrong" — and conflating them is how a suite starts being ignored.

### smoke — is the instrument trustworthy?

Before judging the product, the harness checks itself: it reaches the server and reads the OS
foreground it reports, confirms the candidate list accepts an ordinary window, confirms the server
can read back an enforced working-set limit for every managed process, then proves that minimizing
and restoring through the server's window API actually changes the state the server reads. Finally it
arms a ceiling through the server and confirms the kernel readback reports it, releases it, and
confirms the readback no longer does.

A cap is armed while the app is in a background state on purpose: with the window in front the engine
hands every limit straight back, which is the product rule rather than a test failure.

### bug2 — does a Spotify the user returned to get its limits back?

The regression was that a foreground Spotify could keep a ceiling because the restore was gated on the
server's own window reading, and that reading is a transient enumeration. The scenario takes the
window out of the foreground, arms a real ceiling, then tests two separate things:

- **The claim releases a ceiling that the window reading would keep.** The harness posts a
  "focused and on-screen" beacon while the server's window state still says background, and the
  ceiling must go. That path exists only in the fixed build, and it is the part a static check could
  never distinguish.
- **A real focus change releases it, with nothing posted by the harness.** Focus is moved away from
  Spotify (the server's `blur` hands it to the shell's desktop window), the ceiling is armed again,
  and then focus goes back to Spotify through the server's window API. The step asserts the ceiling
  came off, and a separate step observes whether the dashboard itself reported focus - those are two
  facts and are reported as two steps.

The scenario also asserts the other half of the rule: in a background state **that this installation's
configuration actually manages**, the ceiling must survive for several seconds. Which states those are
is read from the live settings, because `optimizeWhenBackground`, `optimizeWhenMinimized` and
`optimizeWhenHidden` differ per machine; assuming a hold in an unmanaged state tested a rule the
product does not have.

Cleanup releases every ceiling the scenario armed and restores the window, hands focus back to
whatever held it before, then verifies the OS reports no enforced ceiling left. The verification is
the point: "we tried to clean up" is not the same claim.

### benchmark — the measurable half of a memory A/B

`node Index.js benchmark` is different from the pass/fail scenarios: it returns numbers. For one
process it samples, through the debug API only, a baseline with no explicit ceiling, then arms a
ceiling and samples under it, then releases it and samples the rebound - reporting the median working
set, private working set, page-fault rate and CPU for each phase, plus the engine's restore telemetry
(processes restored, failures, duration). It ends with a verdict and a machine-readable JSON block.

The ceiling binds, and the run shows whether it bound: an observed run capped a 14.7 MB network
utility at 8 MB and read the working set settle to 8.0 MB (a 45% reduction, restore 48.8 ms, zero
failures). Two things keep that honest. Where a ceiling sits above the process's own working set
nothing binds and the reduction is small. And because Windows trims a working set lazily, a short
window on an idle process can show a near-zero reduction even with a genuinely enforced ceiling below
it - the tool reports the 0% it measured instead of waiting or guessing, so raise `--seconds` or pick
a busier process when you want the trim to show. Where the process is idle the fault rate is genuinely
zero, and the verdict says "not measurable" rather than inventing a cost.

What it does **not** claim, on purpose: a lower working set is not proof memory was freed (trimmed
pages can sit in the standby list and return through a soft fault), and the human-only half of
`docs/BENCHMARKING.md` - the time from window focus until the UI accepts a click - is not observable
from the API, so the verdict leaves it out instead of guessing it. Use `--pid` to target one process
exactly (`--identity` is a substring and picks the first match); pick a ceiling a little below the
process's own baseline to see a binding reduction.

Both the ceiling arming and the cleanup travel the engine's real apply and restore paths, so the
figures are the shipped engine's behaviour, not the harness's. Cleanup releases every ceiling, restores
the window, and hands focus back to whatever held it before.

## Limits of what is proven here

- The transient window-enumeration gap that originally stranded a ceiling is not reproduced
  organically; the scenario proves the server acts on a fresh client claim while its window reading
  says otherwise, which is the mechanism that covers that gap.
- The app-side beacon is proven to arrive on a real focus change, but a renderer that was just
  un-hidden can deliver its own event late, so that observation step is allowed to report
  INCONCLUSIVE on a slow machine.
- Numbers come from one machine with one Spotify session. `watch` and `ram` are the commands to use
  when a report needs the actual figures rather than the verdict.
