'use strict';
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const root = path.resolve(__dirname, '..');
let failures = 0;
function check(name, condition) {
  if (condition) console.log(`PASS ${name}`);
  else { console.error(`FAIL ${name}`); failures++; }
}
function read(relative) { return fs.readFileSync(path.join(root, relative), 'utf8'); }

const requiredFiles = [
  'app/index.js', 'app/manifest.json',
  '.gitignore', 'manifest.json', 'README.md', 'LICENSE', 'PRIVACY.md', 'SECURITY.md',
  'CHANGELOG.md', 'docs/BENCHMARKING.md', 'docs/MARKETPLACE.md', 'docs/INSTALLATION.md', 'docs/SERVER-MIGRATION.md', 'docs/RELEASE_CHECKLIST.md', 'docs/DEVELOPMENT.md', 'docs/TROUBLESHOOTING.md',   'installer/wix/Product.wxs', 'scripts/build-package.ps1', 'scripts/swap-server.ps1', 'server/src/main.rs', 'server/crates/Core/src/lib.rs', 'server/crates/Spotify/src/lib.rs', 'server/src/Runtime.rs', 'server/src/Runtime/Setup.rs', 'server/src/Runtime/Store.rs', 'server/src/Runtime/Api.rs', 'server/src/Runtime/Monitor.rs', 'server/crates/Windows/src/lib.rs', 'assets/marketplace-cover.svg', 'assets/readme-preview.svg', 'assets/readme-banner.svg',
  '.github/ISSUE_TEMPLATE/config.yml', '.github/ISSUE_TEMPLATE/feature-request.yml', '.github/ISSUE_TEMPLATE/plugin-bug.yml', '.github/ISSUE_TEMPLATE/question.yml', '.github/ISSUE_TEMPLATE/server-bug.yml',
  '.github/workflows/validate.yml', '.github/workflows/release.yml'
];
for (const file of requiredFiles) check(`exists: ${file}`, fs.existsSync(path.join(root, file)));
check('obsolete Node helper source directory was removed', !fs.existsSync(path.join(root, 'helper')));

let appManifest = {};
let marketplaceManifest = {};
try { appManifest = JSON.parse(read('app/manifest.json')); } catch (_) { check('Custom App manifest JSON', false); }
try { marketplaceManifest = JSON.parse(read('manifest.json')); } catch (_) { check('Marketplace manifest JSON', false); }
check('Spicetify Custom App manifest is valid', Boolean(appManifest.name && appManifest.icon && appManifest['active-icon']));
const appSubfiles = Array.isArray(appManifest.subfiles) ? appManifest.subfiles : [];
for (const file of appSubfiles) check(`exists: app/${file} (subfile)`, fs.existsSync(path.join(root, 'app', file)));
check('Custom App manifest does not list index.js as a subfile', !appSubfiles.includes('index.js'));
const appIndex = read('app/index.js');
const appScript = [...appSubfiles.map(file => read(`app/${file}`)), appIndex].join('\n;\n');
const appStyles = fs.existsSync(path.join(root, 'app/style.css')) ? read('app/style.css') : '';
const app = appScript + '\n' + appStyles;
check('Custom App source avoids ES module syntax because subfiles are concatenated', !/^\s*(?:import|export)\b/m.test(appScript));
check('root manifest is Marketplace metadata', marketplaceManifest.name === 'Memory Sentry' && marketplaceManifest.preview === 'assets/marketplace-cover.svg' && marketplaceManifest.readme === 'README.md' && !marketplaceManifest.main);
check('Marketplace metadata uses the selected discovery tags and author attribution', JSON.stringify(marketplaceManifest.tags) === JSON.stringify(['memory', 'windows', 'experimental']) && marketplaceManifest.authors?.some(author => author.url === 'https://github.com/yanlvl99'));
check('artwork uses standalone SVGs with vector elements and no external PNG links', [['marketplace-cover.svg', '1600', '1600'], ['readme-preview.svg', '1600', '1000'], ['readme-banner.svg', '1800', '700']].every(([file, width, height]) => { const svg = read(`assets/${file}`); return /^<svg\b/.test(svg) && new RegExp(`width="${width}"`).test(svg) && new RegExp(`height="${height}"`).test(svg) && /<path\b/.test(svg) && !/href=["'](?!data:)[^"']*\.png/i.test(svg); }));
check('PNG sources, archive and obsolete screenshot assets were removed', !fs.readdirSync(path.join(root, 'assets')).some(file => (/\.png$/i.test(file) || /\.zip$/i.test(file)) && file !== 'readme-banner.png') && ['COM.png', 'SEM.png', 'assets/COM-sharp.svg', 'assets/SEM-sharp.svg', 'assets/marketplace-banner.svg', 'assets/marketplace-preview-sharp.svg', 'preview.png', 'assets/marketplace-preview.svg'].every(file => !fs.existsSync(path.join(root, file))));
check('README keeps the banner and omits the obsolete comparison preview', /assets\/readme-banner\.(svg|png)/.test(read('README.md')) && !/assets\/readme-preview\.svg/.test(read('README.md')) && !/not a controlled benchmark/i.test(read('README.md')));
check('Marketplace description explains minimized RAM savings and the restore-time trade-off', /Reduce Spotify's RAM usage while minimized/i.test(marketplaceManifest.description) && /slower interface loading when restored/i.test(marketplaceManifest.description) && marketplaceManifest.description.length <= 120);
check('the supported OS is gated by the server and stated in the dashboard footer', /Windows 10/.test(app) && /Windows 11/.test(app) && /detect_supported_client_os/.test(read('server/src/Runtime/Setup.rs')));
check('local publishing helper and generated archives are ignored', /\/PUSH-TO-GITHUB\.bat/.test(read('.gitignore')) && /\*\.bundle/.test(read('.gitignore')) && /\*\.zip/.test(read('.gitignore')));
check('development guide documents Rust build and package validation without local publishing scripts', /cargo test/.test(read('docs/DEVELOPMENT.md')) && /scripts\/build-package\.ps1/.test(read('docs/DEVELOPMENT.md')) && !/PUSH-TO-GITHUB\.bat/i.test(read('docs/DEVELOPMENT.md')));
check('README explains server requirement', /Memory Sentry server/i.test(read('README.md')) && /separate per-user process/i.test(read('README.md')));
check('README documents adaptive CPU and headroom rules', /six consecutive seconds/i.test(read('README.md')) && /ten seconds/i.test(read('README.md')) && /full current working-set ceiling/i.test(read('README.md')));
check('README documents the disabled mode, shared delay and five-second warning', /Disabled\*\*? mode is the master on\/off control/i.test(read('README.md')) && /same configured delay applies whenever Custom is enabled/i.test(read('README.md')) && /five-second warning countdown/i.test(read('README.md')) && /Optimize when/.test(read('README.md')));
check('changelog uses the current five-second manual warning', /five-second warning countdown/i.test(read('CHANGELOG.md')) && !/15-second warning countdown/i.test(read('CHANGELOG.md')));
check('docs describe the uniform 38 MB start, the 4 MB floor and the adaptive Audio ceiling', /starts at a 38 MB automatic ceiling with a 4 MB absolute floor, and that Audio adapts upward to 60 MB/i.test(read('docs/RELEASE_CHECKLIST.md')) && /In Custom mode a pinned process holds a fixed ceiling/i.test(read('docs/SERVER-MIGRATION.md')) && /38 MB ceiling with a 4 MB absolute floor/i.test(read('docs/MARKETPLACE.md')));
check('troubleshooting uses the disabled mode to stop all limits', /selecting \*\*Disabled\*\* in the memory mode/i.test(read('docs/TROUBLESHOOTING.md')) && /Custom mode is the only active mode/i.test(read('docs/TROUBLESHOOTING.md')));
check('issue templates request current policy state and use the correct log files', /visible in background.*automatic\/manual mode.*selected background delay/.test(read('.github/ISSUE_TEMPLATE/server-bug.yml')) && ['setup.log', 'startup.log', 'helper.log'].every(name => read('.github/ISSUE_TEMPLATE/server-bug.yml').includes(`\`${name}\``)) && /dashboard memory mode, the enabled trigger states/.test(read('.github/ISSUE_TEMPLATE/plugin-bug.yml')) && /Windows 11 24H2/.test(read('.github/ISSUE_TEMPLATE/plugin-bug.yml')));
check('dashboard offers a Custom/Disabled mode picker and per-process Automatic/Manual limit segments', app.includes("tx('segAuto')") && app.includes("tx('segManual')") && app.includes("tx('segCustom')") && app.includes("tx('segDisabled')") && /optimi[sz]e when/i.test(app) && app.includes('pluginDisabledNote'));
check('README documents unsigned SmartScreen limitations', /No unsigned release can guarantee zero SmartScreen/i.test(read('README.md')));
check('GPL-3.0 license is present', /GNU GENERAL PUBLIC LICENSE/i.test(read('LICENSE')) && /Version 3, 29 June 2007/i.test(read('LICENSE')));
check('privacy explicitly states no uploads', /No information is uploaded/i.test(read('PRIVACY.md')));
check('security notes never disable antivirus', /does not disable antivirus/i.test(read('SECURITY.md')));
check('Marketplace preview exists', fs.existsSync(path.join(root, marketplaceManifest.preview)));
check('release publishes only the official per-user MSI and its SHA-256', /MemorySentryForSpicetify-\*\.msi/.test(read('.github/workflows/release.yml')) && /MemorySentryForSpicetify-\*\.msi\.sha256/.test(read('.github/workflows/release.yml')) && !/MemorySentry-\*windows-x64\.zip/.test(read('.github/workflows/release.yml')) && /action-gh-release/i.test(read('.github/workflows/release.yml')));
check('build output uses the MSI packaging script and no custom portable ZIP', /MemorySentryForSpicetify-.*\.msi/.test(read('scripts/build-package.ps1')) && !/MemorySentrySetup-/.test(read('scripts/build-package.ps1')) && !/Compress-Archive/.test(read('scripts/build-package.ps1')) && !/MemorySentry-.*windows-x64\.zip/.test(read('scripts/build-package.ps1')));

const wixScript = read('installer/wix/Product.wxs');
// Every Custom App subfile declared in app/manifest.json must ALSO be copied by an installed <File>
// entry in Product.wxs. Spicetify concatenates the manifest subfiles at load, so if a ui/*.js is in
// the manifest but missing from the installer the bundle references a component that was never
// shipped and the whole page throws on load while the server looks perfectly healthy.
// This check turns that class of regression into a red validator run instead of a blank dashboard.
const wixMissingUi = appSubfiles.filter(sf => !wixScript.includes(sf.replace(/\//g, '\\')));
check(`WiX Product.wxs installs every manifest subfile (missing: ${wixMissingUi.join(', ') || 'none'})`, wixMissingUi.length === 0);
const buildScript = read('scripts/build-package.ps1');
const buildScriptSwap = read('scripts/swap-server.ps1');
const validation = read('.github/workflows/validate.yml');
const rustCargo = read('server/Cargo.toml');
const rustLib = read('server/crates/Core/src/lib.rs');
const rustMain = read('server/src/main.rs');
const rustRuntime = read('server/src/Runtime.rs');
const rustWindows = read('server/crates/Windows/src/lib.rs');
const rustSpotify = read('server/crates/Spotify/src/lib.rs');
const rustSetup = read('server/src/Runtime/Setup.rs');
const rustStore = read('server/src/Runtime/Store.rs');
const rustApi = read('server/src/Runtime/Api.rs');
const rustMonitor = read('server/src/Runtime/Monitor.rs');
// The server bin is split into modules; guard behaviour across all of its source, not one file.
const rustServer = [rustRuntime, rustSetup, rustStore, rustApi, rustMonitor].join('\n');
check('dashboard supports automatic Portuguese and English', /navigator\.language/i.test(app) && /pt-BR/i.test(app) && /Auto language: English/i.test(app));
check('dashboard has a GitHub release button when helper is absent', /MSSOpenGitHub/i.test(app) && /github\.com\/yanlvl99\/memory-sentry/i.test(app));
check('dashboard offers setup/help links without a Node.js button', /MSSOpenGitHub/.test(app) && /MSSOpenInstallGuide/.test(app) && !/MSSOpenNode|nodeButton|nodejs\.org/.test(app));
check('dashboard links to official releases and a generic installation guide', /MSS_RELEASES_URL/.test(app) && /MSS_INSTALL_GUIDE_URL/.test(app) && /installGuideButton/.test(app) && !/Marketplace setup guide|Guia de instalação do Marketplace/.test(app));
check('documentation describes one install path and Marketplace linking only', /only supported installation method/i.test(read('docs/INSTALLATION.md')) && /only links to the project repository/i.test(read('docs/INSTALLATION.md')) && !/Path B/.test(read('docs/INSTALLATION.md')));
check('dashboard offers persistent per-identity manual caps whenever enabled (Custom is the only active mode)', /capsEditable = config\.enabled === true/.test(app) && /setManualCap/.test(app) && /clearManualCap/.test(rustServer) && rustServer.includes('manual_role_caps'));
check('dashboard automatically saves policy and release setting changes', app.includes('function setField(key, value)') && app.includes('saveSettings(nextConfig)') && app.includes("setMode('custom')") && app.includes("setMode('disabled')") && app.includes("tx(saving ? 'saving' : 'autoSavedShort')") && !app.includes("tx('save')"));
check('manual limit inputs auto-apply, show saved role values and reset after restore', app.includes('scheduleManualLimit(process, key, value)') && app.includes('value: manualInputs[key] ?? String(process.manualCapMB ?? defaultMB)') && app.includes('await clearManualLimit(process, key)') && app.includes("windowState === 'foreground'") && app.includes('setManualInputs({}'));
// The limit textbox is a controlled input: its value comes from manualInputs, so every change event must
// also store the typed draft. Debouncing the server write alone makes React snap the DOM back to the old
// state value on the next render and the field becomes untypeable. Guard the whole App -> Hero -> RolePanel
// prop chain so removing any link fails here instead of shipping an editable-looking but frozen field.
check('manual limit field stores the typed draft before debouncing the server write', app.includes('onInput(key, value); onSchedule(process, key, value)') && app.includes("onInput: (key, value) => setManualInputs(previous => ({ ...previous, [key]: value }))") && app.includes('onCopy: onCopy, onInput: onInput') && app.includes('setMode, onInput, onSchedule'));
// Now that every keystroke lands in manualInputs, the pre-edit origin and the post-rejection draft
// must key off SERVER truth (the saved pin), never the in-progress draft, or an abandoned number can
// be restored by the warning modal or applied by the pin action.
check('manual-limit origin seeds from server truth and rejected drafts roll back', app.includes("manualInputOrigins.current.set(key, String(process.manualCapMB ?? process.defaultLimitMB))") && app.includes("setManualInputs(previous => ({ ...previous, [key]: String(origin ?? process.manualCapMB ?? process.defaultLimitMB) }))") && !app.includes('String(manualInputs[key] ?? process.manualCapMB'));
check('below-default warning requires confirmation after five seconds', /performance\.now\(\) \+ 5000/.test(app) && /Spicetify\.PopupModal/.test(app) && /popup\.display\(\{ title, content: panel, isLarge: false \}\)/.test(app) && /popup\.hide\(\)/.test(app) && /tx\('waitBeforeApply', \{ seconds \}\)/.test(app) && /confirmButton\.disabled = false/.test(app) && /confirmManualLimit\(warning, countdownComplete\)/.test(app) && /if \(!countdownComplete\) return/.test(app) && /void applyManualLimit\(\{ identity: warning\.key, role: warning\.role \}, warning\.capMB\)/.test(app) && !/hideModal\(\);\s*setManualWarning\(null\);\s*void applyManualLimit/.test(app));
check('dashboard relies on automatic restore and polling instead of action buttons', !app.includes("tx('applyManual')") && !app.includes("tx('restoreNow')") && !app.includes("tx('refresh')"));
check('manual limit actions address a process by identity, not PID', /action: 'setManualCap', identity: process\.identity/.test(app) && /action: 'clearManualCap', identity:/.test(app) && rustServer.includes('parse_json_string(body, "identity")') && rustServer.includes('fn live_target_for_identity') && !rustServer.includes('fn manual_target'));
check('dashboard shows per-process CPU and active ceiling', /process\.cpuPercent/i.test(app) && /process\.limitMB/i.test(app));
// The pin acts on one process identity, so the confirmation messages must not call it a shared role
// limit. The foreground-capping strings were removed with that feature and must not creep back.
check('manual-limit messages name the process and the removed foreground strings stay gone', app.includes("limitApplied: 'Fixed limit applied to this process.'") && app.includes("limitSavedForBackground: 'Fixed limit saved for this process; it will apply when Spotify leaves the foreground.'") && app.includes("limitCleared: 'Process limit removed; the adaptive engine manages it again.'") && app.includes("limitApplied: 'Limite fixo aplicado a este processo.'") && app.includes("limitCleared: 'Limite do processo removido; o motor adaptativo volta a gerenciá-lo.'") && !app.includes('whenForeground:') && !app.includes('whenForegroundHint:') && !app.includes('foregroundShort:'));
// Both snapshot writers (accept_snapshot and refresh_memory) update cpu time, so each percent delta must
// be divided by the exact age of the shared timestamped baseline it was read from. Dividing by a nominal
// window instead dilutes every reading whenever the other writer advanced the counter in between, which
// silently biases the adaptive engine toward "low CPU". Guard the baseline contract end to end.
check('cpu percent divides deltas by the shared timestamped baseline, never by a nominal window', rustRuntime.includes('cpu_baselines: BTreeMap<(u32, u64), (u64, u64)>') && rustMonitor.includes('cpu_baselines: BTreeMap::new()') && rustWindows.includes('pub captured_at_ms: u64') && rustMonitor.includes('snapshot.captured_at_ms = now_ms()') && /cpu_baselines[\s\S]{0,40}\.insert\(key,\s*\(process\.cpu_time_ms as u64,\s*observed_at\)\)/.test(rustMonitor) && /cpu_baselines[\s\S]{0,40}\.insert\(key,\s*\(stats\.cpu_time_ms as u64,\s*now\)\)/.test(rustMonitor) && rustMonitor.includes('observed_at.saturating_sub(previous_at)') && rustMonitor.includes('now.saturating_sub(previous_at)') && /cpu_baselines[\s\S]{0,40}\.retain/.test(rustMonitor) && !rustMonitor.includes('elapsed_ms'));
// Page faults carry the same contract: the baseline stores the count WITH the instant it was captured,
// each writer stamps the rate for its own window, and the adaptive engine consumes that rate. Dividing
// a delta by the snapshot interval would dilute the thrash signal exactly as it once diluted cpu.
check('page-fault rate divides deltas by the shared timestamped baseline, never by a nominal window', rustRuntime.includes('page_fault_baselines: BTreeMap<(u32, u64), (u64, u64)>') && rustMonitor.includes('page_fault_baselines: BTreeMap::new()') && rustLib.includes('pub fn estimate_faults_per_second') && rustLib.includes('pub fn faults_since') && rustWindows.includes('pub page_faults_per_sec: Option<f64>') && /page_fault_baselines[\s\S]{0,60}\.insert\(key,\s*\(current,\s*observed_at\)\)/.test(rustMonitor) && /page_fault_baselines[\s\S]{0,60}\.insert\(key,\s*\(stats\.page_fault_count,\s*now\)\)/.test(rustMonitor) && rustMonitor.includes('estimate_faults_per_second(') && rustMonitor.includes('process.page_faults_per_sec.unwrap_or(0.0)') && !rustMonitor.includes('delta as f64 / (interval as f64 / 1000.0)'));
// The per-process rate is the value the engine actually consumed, and it is what a real-machine
// calibration of the thrash threshold has to read. The debug surface must expose it (not only the
// raw delta) so the thresholds can be measured instead of guessed.
check('debug console exposes the per-process page-fault rate behind the thrash guard', rustApi.includes('pageFaultsPerSec\\":{}') && rustServer.includes('json_optional_number(p.page_faults_per_sec)') && app.includes("tx('colFaultRate')") && app.includes('t.pageFaultsPerSec') && app.includes("colFaultRate: 'Faults/s'") && app.includes("colFaultRate: 'Falhas/s'"));
// The floorless policy: every role starts at 38 MB, the absolute floor is 4 MB, and a ceiling that
// caused thrashing becomes that identity's learned safe floor instead of a per-role clamp. Guard the
// constants, the effective floor in the policy sample, the persistence key and the runtime wiring.
check('the adaptive policy starts at 38 MB, floors at 4 MB and learns a safe floor on thrash relief', rustLib.includes('pub const INITIAL_CAP_MB: u32 = 38;') && rustLib.includes('pub const MIN_CAP_MB: u32 = 4;') && /fn baseline_cap_mb[\s\S]{0,160}Some\(INITIAL_CAP_MB\)/.test(rustLib) && /fn minimum_cap_mb[\s\S]{0,160}Some\(MIN_CAP_MB\)/.test(rustLib) && rustLib.includes('minimum_cap_mb: u32') && rustRuntime.includes('safe_floors: BTreeMap<String, u32>') && rustStore.includes('fn parse_safe_floors') && rustStore.includes('safeFloors') && rustMonitor.includes('minimum_cap_mb:') && rustMonitor.includes('AdjustmentReason::ThrashRelief') && rustMonitor.includes('safe_floors') && rustMonitor.includes('role-rebased') && read('README.md').includes('38 MB') && read('README.md').includes('4 MB'));
// CPU is a second starvation signal, and a few roles keep a state-aware floor: Audio never drops
// below 25 MB, a renderer holds 38 MB while the window is visible in the background. Guard the
// constants, the pure floor helper and the runtime wiring end to end.
check('CPU starvation and the role/state floors are wired end to end', rustLib.includes('pub const STARVATION_CPU_PERCENT: f64 = 50.0;') && rustLib.includes('pub const STARVATION_WORKING_SET_MB: f64 = 8.0;') && rustLib.includes('fn state_floor_mb') && rustLib.includes('Self::Audio => 25') && rustLib.includes('Self::Renderer if visible_background => 38') && rustLib.includes('CpuStarvation') && rustMonitor.includes('state_floor_mb(') && rustMonitor.includes('cpu-starvation') && rustMonitor.includes('AdjustmentReason::CpuStarvation') && read('README.md').includes('25 MB') && read('README.md').includes('visible in the background'));
check('dashboard uses private working-set memory when Windows provides it', rustWindows.includes('ProcessMemoryCountersEx2') && rustWindows.includes('private_working_set_size') && rustServer.includes('privateWorkingSetMB') && rustServer.includes('private_working_set_available') && app.includes("memory.privateWorkingSet ? process.privateWorkingSetMB : process.workingSetMB") && app.includes("memory.privateWorkingSet ? 'privateWorkingSetNote' : 'ramNote'"));
check('dashboard shows a memory composition bar instead of a Spotify total card', !app.includes('mss-summary-value') && !app.includes('mss-kpi-grid') && !app.includes('optimizedRam') && !app.includes('activeLimits') && !app.includes('activeCount') && app.includes('mss-composition') && app.includes('memory.targetCount'));
check('dashboard renders no chart or card grid', !app.includes('mss-kpi-grid') && !app.includes('.mss-card') && !app.includes('mss-chart') && !app.includes('mss-heatmap'));
check('dashboard source is modularised with PascalCase subfiles and no numeric prefixes', appSubfiles.length >= 10 && appSubfiles.every(file => /^ui\/[A-Z][A-Za-z0-9]*\.js$/.test(file)) && appSubfiles.includes('ui/Icons.js') && appSubfiles.includes('ui/Widgets.js') && appSubfiles.includes('ui/TopBar.js') && appSubfiles.includes('ui/Hero.js') && appSubfiles.includes('ui/RolePanel.js') && appSubfiles.includes('ui/Settings.js') && appSubfiles.includes('ui/Diagnostics.js') && appSubfiles.includes('ui/Footer.js') && appSubfiles.includes('ui/App.js'));
check('mode picker uses inline icons and exposes live Custom + Disabled tiles', app.includes('function MSSIcon(') && app.includes('function MSSSegButton(') && app.includes("iconName: 'custom'") && app.includes("iconName: 'disabled'") && app.includes("tx('segCustom')") && app.includes("setMode('custom')") && !app.includes("iconName: 'auto'") && !app.includes("iconName: 'manual'") && !app.includes('comingSoon: true'));
check('role group panel is clickable and each process row carries its own per-identity limit field', (app.match(/className: 'mss-identity-limit'/g) || []).length === 1 && app.includes('mss-role-panel-head') && app.includes('processes.map(process') && app.includes('const key = process.identity'));
check('low-limit modal warning and confirmation use high-contrast red styling', app.includes('mss-btn-danger') && app.includes('.mss-modal .mss-notice-warn') && app.includes('#ff5252'));
check('closing a low-limit warning restores the prior textbox value', app.includes('setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }))') && app.includes('modalObserver.observe(document.body, { childList: true, subtree: true })') && app.includes('closeWarning(true)') && app.includes('confirmedManualWarning.current = warning.readyAt'));
check('global plugin switch persists enabled state and runs activation transitions', /saveSettings\(nextConfig\)/.test(app) && /enabled: true, autoOptimize: true, customMode: true/.test(app) && /showActivationToast\(nextConfig\.enabled\)/.test(app) && /\\"enabled\\": \{\},/.test(rustServer) && rustServer.includes('if !state.settings.enabled'));
check('configuration failures do not masquerade as a lost server connection', app.includes('setSettingsError(err?.message || tx(\'saveFailed\'))') && !app.includes("setError(tx('saveFailed'))") && rustServer.includes('if failed > 0 && !disabling'));
check('dashboard lets the user pick which hidden states trigger optimization', app.includes("key: 'optimizeWhenBackground'") && app.includes("key: 'optimizeWhenMinimized'") && app.includes("key: 'optimizeWhenHidden'") && app.includes('function setTrigger(key, value)') && app.includes('if (!value && activeTriggers() <= 1) return;') && rustServer.includes('fn should_manage_limits(settings: &Settings, window_state: &str)') && rustServer.includes('"background-visible" => settings.optimize_when_background') && rustServer.includes('optimizeWhenBackground') && rustServer.includes('optimizeWhenMinimized') && rustServer.includes('optimizeWhenHidden'));
check('server refuses to disable every optimization trigger', rustServer.includes('if !next.optimize_when_background') && rustServer.includes('!next.optimize_when_minimized') && rustServer.includes('!next.optimize_when_hidden') && rustServer.includes('next.optimize_when_background = true;'));
check('foreground is never capped; optimization is background-only and pressure-aware', !rustServer.includes('optimize_when_foreground') && !rustServer.includes('foreground_cap_mb') && !app.includes('optimizeWhenForeground') && rustServer.includes('pressure_level(') && rustWindows.includes('fn system_memory') && rustServer.includes('pressureLevel'));
check('delay and refresh are exposed as sliders (not dropdowns), commit only on release, and the server honors the refresh cadence', app.includes('function MSSSlider') && app.includes("h(MSSSlider, {") && app.includes("style: { '--pct'") && app.includes('onPointerUp: commit') && app.includes('onKeyUp: commit') && app.includes('onBlur: commit') && /onChange: event => setDraft\(Number\(event\.target\.value\)\)/.test(app) && /onChange: v => onField\('hiddenDelaySeconds', v\)/.test(app) && /onChange: v => onField\('refreshIntervalMs', v\)/.test(app) && !app.includes("[1, 2, 3, 5, 8, 10, 15, 20, 30]") && rustServer.includes('background_delay_elapsed') && rustServer.includes('refresh_interval_ms') && rustServer.includes('s.settings.refresh_interval_ms') && rustServer.includes('"refreshIntervalMs"'));
check('settings card is composed from reusable widgets (row, chip, slider, select) with centered rows', app.includes('function MSSSettingRow') && app.includes('function MSSSelect') && app.includes('function MSSChip') && app.includes('function MSSSlider') && /h\(MSSSettingRow, \{ label: tx\('delayShort'\), description:/.test(app) && /h\(MSSSettingRow, \{ label: tx\('releaseShort'\), description:/.test(app) && /h\(MSSSelect, \{[\s\S]*?value: config\.trimToCap !== false \? 'ceiling' : 'aggressive'/.test(app) && /\.mss-setting\s*\{[\s\S]*?align-items:\s*center/.test(appStyles));
check('learned usage profile is a per-identity busy EWMA persisted across sessions, seeds the squeeze, and is surfaced local-only', rustLib.includes('pub fn usage_ewma') && rustLib.includes('fn label(self)') && rustServer.includes('usage_scores: BTreeMap<String, u32>') && rustServer.includes('fn parse_usage_scores') && rustServer.includes('"usageScores"') && rustServer.includes('usage_ewma(state.settings.usage_scores.get(&identity).copied(), cpu)') && rustServer.includes('state.settings.usage_scores.get(&identity)') && rustServer.includes('Could not persist usage profile at shutdown') && rustServer.includes('usageScore\\":') && rustServer.includes('usageBucket\\":') && app.includes('learnedBadge') && app.includes("tx('colLearned')") && app.includes('t.usageBucket'));
check('the learned profile and telemetry are documented as local-only and never uploaded', fs.existsSync(path.join(root, 'PRIVACY.md')) && read('PRIVACY.md').toLowerCase().includes('learned usage profile') && read('PRIVACY.md').includes('never a PID') && app.includes("learnedHint: 'How busy this process usually is") && app.includes('nothing is uploaded'));
check('each background option carries an M3 supporting-text explanation mapped to its server setting', /description: tx\('whenDesc'\)/.test(app) && /description: tx\('delayDesc'\)/.test(app) && /description: tx\('refreshDesc'\)/.test(app) && /description: tx\('releaseDesc'\)/.test(app) && app.includes("whenDesc: 'Which window states start optimization") && app.includes("refreshDesc: 'How often the dashboard re-reads memory and CPU") && app.includes("whenDesc: 'Quais estados da janela iniciam a otimização") && app.includes("refreshDesc: 'Com que frequência o painel rele memória e CPU") && /\.mss-setting-desc\s*\{/.test(appStyles));
check('plugin disabled state blocks manual controls and automatic policy settings', app.includes('disabled: busy || !config.enabled') && app.includes('const controlDisabled = saving || !helperRunning || !config.enabled;') && app.includes('disabled: controlDisabled') && app.includes('pluginDisabledNote') && rustServer.includes('Enable Memory Sentry before setting a manual process limit.'));
check('background settings are grouped and described as applying while active', app.includes('mss-toolbar') && app.includes("backgroundSettingsHint: 'These background options apply whenever Memory Sentry is active.'") && app.includes("backgroundSettingsHint: 'Estas opções de segundo plano se aplicam sempre que o Memory Sentry está ativo.'"));
check('dashboard keeps status and switches in a single-row topbar', /\.mss-topbar\s*\{[^}]*flex-wrap:\s*nowrap/.test(appStyles) && app.includes('mss-topbar-controls') && app.includes('mss-pill') && app.includes('mss-switch-knob') && /@media \(max-width: 559px\)/.test(appStyles));
check('the process section shows a single memory-composition bar with one coloured segment per process', app.includes('mss-composition') && app.includes('mss-composition-seg') && app.includes('function mssPidColor') && app.includes('sortedTargets') && /\.mss-composition\s*\{[^}]*display:\s*flex/.test(appStyles));
check('the composition bar and legend are interactive: clicking either expands the role group panel with per-process copy and limit actions', app.includes('onClick: () => setExpandedRole(active ? null : p.role)') && app.includes('onClick: () => setExpandedRole(active ? null : role)') && app.includes('mss-role-panel') && app.includes('function mssCopy(') && app.includes("tx('copyPid')") && app.includes("tx('copyMemory')"));
check('caps are keyed by PID-free process identity even inside the role group panel', rustServer.includes('fn process_identity') && rustServer.includes('let identity = identity_from_parts(*role, sub_label.as_deref())') && rustServer.includes('fn manual_cap_for_identity') && app.includes('process.identity') && app.includes('process.subLabel'));
check('Custom mode applies an identity pin to already-adaptive processes so the fixed ceiling takes effect', rustServer.includes('.then(|| identity_pin_cap(&state.settings, &process, role))') && rustServer.includes('if let Some(cap) = pin') && rustServer.includes('apply_manual_cap(state, &process, role, cap)') && rustServer.includes('r.manual && r.applied_maximum_mb == Some(cap)'));
check('the per-state ceiling feature is fully removed with no dead code in server, policy or UI', !rustServer.includes('state_role_caps') && !rustServer.includes('stateRoleCaps') && !rustServer.includes('state_mode_for') && !rustServer.includes('state_key_for_window') && !rustServer.includes('parse_state_mode') && !rustServer.includes('StateMode') && !rustLib.includes('StateMode') && !app.includes('StateCeilingEditor') && !app.includes('MSS_STATE_TABS') && !app.includes('stateRoleCaps') && !app.includes('mssStateCapsToArray') && !app.includes('segAdaptive'));
// Native discovery is the dominant RAM/CPU win, but safety-first: the PowerShell/WMI path must stay
// wired as a per-pass fallback (a broad PEB-read failure falls back rather than collapsing identities),
// and the x64 PEB layout must be pinned by a compile-time size assert so a wrong ABI cannot silently
// mis-read command lines.
check('native Win32 process discovery replaces the per-tick subprocess yet keeps PowerShell as a fallback', rustWindows.includes('pub fn native_process_snapshot(targets: &[&str])') && rustWindows.includes('CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)') && rustWindows.includes('fn native_command_line') && rustWindows.includes('NtQueryInformationProcess') && rustWindows.includes('ReadProcessMemory') && rustWindows.includes('PROCESS_VM_READ') && rustWindows.includes('assert!(std::mem::size_of::<ProcessBasicInformation>() == 48)') && rustWindows.includes("pub fn powershell_snapshot(image_name: &str)") && rustWindows.includes("Name='__MSS_IMAGE__'") && rustServer.includes('windows::native_process_snapshot(app_profile().target_executables())') && rustServer.includes('windows::powershell_snapshot(app_profile().process_image_name())') && rustServer.includes('peb_failed_broadly'));
// The generic/Spotify seam: the monitor, the policy engine and the Win32 layer hold no application
// knowledge of their own. Everything Spotify-specific (executable name, command-line role tokens,
// identity sub-labels) lives in the crates/Spotify profile behind the Core AppProfile trait, wired in
// exactly one place in the runtime. This is what lets a new desktop app be supported by adding one
// crate instead of editing the engine, so a regression that re-hardcodes Spotify into the generic
// layers must fail the validator.
check('app-specific targeting is isolated in the Spotify profile behind a generic AppProfile trait', rustLib.includes('pub trait AppProfile') && rustLib.includes('fn classify_role') && rustLib.includes('fn sub_label') && rustSpotify.includes('impl AppProfile for SpotifyProfile') && rustSpotify.includes('["spotify.exe"]') && rustSpotify.includes('"Spotify.exe"') && rustCargo.includes('members = ["crates/Core", "crates/Windows", "crates/Spotify"]') && rustCargo.includes('memory_sentry_spotify = { path = "crates/Spotify"') && rustServer.includes('use memory_sentry_spotify::SpotifyProfile') && rustServer.includes('static APP_PROFILE: SpotifyProfile') && rustServer.includes("fn app_profile() -> &'static dyn AppProfile") && rustWindows.includes('targets.iter().any(|target| entry_exe_matches') && rustWindows.includes('.replace("__MSS_IMAGE__", image_name)'));
check('same-role processes carry a live sub-label parsed from the command line', rustServer.includes('fn process_sub_label') && rustServer.includes('subLabel') && app.includes('process.subLabel') && app.includes('p.subLabel'));
check('Custom mode exposes a per-process Automatic/Manual control keyed on the SAVED pin (survives foreground restore)', app.includes('onClear: clearManualLimit') && app.includes('onPin: pinCurrentCeiling') && app.includes('mss-seg-proc') && app.includes('const pinned = process.manualCapMB != null;') && app.includes("onClick: () => { if (pinned) onClear(process, key); }") && app.includes("onClick: () => { if (!pinned) onPin(process); }") && app.includes("tx('procModeLabel')") && rustServer.includes('restore_one_record(state, &record, "custom pin cleared")'));
check('server keeps the background wait through an unreadable window state', rustServer.includes('fn window_state_is_unknown(window_state: &str) -> bool') && rustServer.includes('if window_state_is_unknown(&state.window_state) {') && app.includes('const visibleMs = Math.max(250, Math.min(10000, Number(config.refreshIntervalMs)') && app.includes("addEventListener('visibilitychange'") && app.includes('timer = window.setTimeout(tick, delay)'));
check('server detects window states by enumerating real top-level windows', rustWindows.includes('fn EnumWindows(callback:') && rustWindows.includes('pub fn window_candidates(pids: &[u32])') && rustWindows.includes('pub fn classify_windows(') && rustWindows.includes('DWMWA_CLOAKED') && rustWindows.includes('WS_EX_TOOLWINDOW') && rustWindows.includes('MINIMUM_WINDOW_AREA') && !rustWindows.includes("'W|{0}|{1}|{2}'"));
check('server classifies a tray-hidden window as hidden rather than minimized', rustWindows.includes('"hidden", largest_window(candidates)') && rustWindows.includes('window.on_screen && !window.iconic') && rustWindows.includes('classify_windows(&[window(false, false, 1_920_000)], false).0,') && rustWindows.includes('"window-not-found", None'));
check('server requires a visible Spotify window before reporting foreground', rustWindows.includes('if is_foreground && candidates.iter().any(|window| window.on_screen)') && rustWindows.includes('classify_windows(&[window(false, false, 1_920_000)], true).0,'));
check('window state transitions log the raw evidence', rustServer.includes('candidates={} onScreen={} iconic={} foreground={}') && rustServer.includes('window_state_reason: String'));
check('runtime re-reads the window list on every pass', rustServer.includes('fn observe_window_state(state: &mut AppState, is_foreground: bool)') && rustServer.includes('windows::window_candidates(&pids)') && !rustServer.includes('window_confirmed') && !rustServer.includes('trusted_window_handle'));
check('status rows stop reporting the original working-set ceiling read on every poll', !rustServer.includes('\\"originalMaximumMB\\":{}') && !rustServer.includes('\\"originalCeilingUnlimited\\":{}') && !rustServer.includes('usize::MAX / 2') && rustServer.includes('windows::read_original_limits(process.pid, process.creation)'));
check('dashboard names the uncapped state as limitless instead of free memory', app.includes("tx('tagNoCeiling')") && !app.includes("'Livre'") && !app.includes("'Free'") && app.includes("tagNoCeiling: 'Sem limite'") && app.includes("tagNoCeiling: 'Limitless'") && !app.includes('originalCeilingLabel') && !app.includes('originalCeilingNone'));
check('dashboard respects reduced motion', /@media \(prefers-reduced-motion: reduce\)/.test(appStyles) && /transition: none !important/.test(appStyles) && app.includes('mss-toast-on') && app.includes('mss-toast-off'));
check('dashboard explains local-only diagnostics', /No telemetry is uploaded/i.test(app));
check('Custom App replaces per-install token placeholder', app.includes('__MSS_TOKEN__'));
check('Custom App calls local status/config endpoints', app.includes('/api/status') && app.includes('/api/config'));

check('dashboard preserves detailed local API failures', app.includes('err && err.message') && app.includes("setError(tx('statusError') + ' [' + detail + ']')"));
check('dashboard renders a dedicated server-offline screen with the live diagnostic and a retry probe', /function MSSServerOffline\(/.test(app) && app.includes("className: 'mss-offline-diag'") && /h\('pre', \{ className: 'mss-offline-diag' \}, error\)/.test(app) && app.includes('onRetry: retryNow') && app.includes("tx('serverOfflineWhy')") && app.includes("MSSIcon('server')"));
check('dashboard distinguishes a never-started server from a dropped connection and backs off retries', app.includes("if (everConnected.current) setConnection('lost');") && app.includes("failCount.current >= 3 ? 'missing' : 'unknown'") && app.includes("connection === 'missing'") && app.includes("connection === 'lost'") && app.includes('tx(\'connectionLost\')') && app.includes('const delay = failCount.current > 0 ? Math.min(10000, base * 2 ** Math.min(failCount.current, 4)) : base') && app.includes("state === 'lost' ? tx('statusLost')"));
check('native memory refresh keeps the displayed private working set live', rustServer.includes('process.private_working_set_mb = stats.private_working_set_mb;'));
check('Rust uninstall allows Spotify to remain open and restores first', !/pub fn pre_uninstall\(\) -> Result<(), String> \{\s*ensure_spotify_closed/.test(rustServer) && rustServer.indexOf('pub fn pre_uninstall()') < rustServer.indexOf('unregister_startup()?;') && rustServer.indexOf('unregister_startup()?;') < rustServer.indexOf('unregister_spicetify()?;'));
check('single-instance mutex clears stale Win32 last-error state', rustWindows.includes('SetLastError(0)'));
check('Rust launcher records early startup failures and verifies API readiness', rustServer.includes('startup.log') && rustServer.includes('request_status(settings.port, &settings.token)') && rustServer.includes('failed to become ready after 5 seconds'));
check('Rust configure failures are logged for setup diagnostics', rustMain.includes('log_setup_failure(&error)') && rustMain.includes('setup.log'));
check('setup never requires Spotify to be closed manually', !/ensure_spotify_closed/.test(rustServer) && !/Get-Process -Name Spotify/.test(rustServer) && !/<util:CloseApplication/.test(wixScript));
const configureBody = rustServer.slice(rustServer.indexOf('pub fn configure_install('), rustServer.indexOf('fn find_node()'));
check('local API starts and passes readiness before Spicetify apply', configureBody.indexOf('launch_monitor()') >= 0 && configureBody.indexOf('launch_monitor()') < configureBody.indexOf('apply_spicetify(&cli)'));
check('PID waits verify the actual Memory Sentry executable path', rustServer.includes('process_matches_executable(pid, &installed_exe)') && rustWindows.includes('QueryFullProcessImageNameW'));
check('failed API bind removes helper.pid', rustServer.includes('if let Err(error) = start_api(Arc::clone(&shared))') && rustServer.includes('let _ = fs::remove_file(&pid_file);'));
const launcherBody = rustServer.slice(rustServer.indexOf('pub fn launch_monitor()'));
check('Server launcher creates startup diagnostics before reading settings', launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') >= 0 && launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') < launcherBody.indexOf('load_settings(&settings_path)') && launcherBody.includes('bootstrap_default_settings'));
check('redundant batch, PowerShell and Inno installer entry points are absent', ['Install.bat','Uninstall.bat','installer/Install.ps1','installer/Uninstall.ps1','installer/inno'].every(file => !fs.existsSync(path.join(root, file))));
// --- WiX MSI installer guards (per-user MSI is the primary shipped installer) ---
check('MSI is per-user and uses MajorUpgrade', /Scope="perUser"/.test(wixScript) && /<MajorUpgrade/.test(wixScript));
check('MSI shows a folder-selection wizard (WixUI_InstallDir)', /WixUI_InstallDir/.test(wixScript) && !/WixUI_Minimal/.test(wixScript));
check('MSI forces overwrite on install/repair (REINSTALLMODE=amus)', /REINSTALLMODE.*amus/.test(wixScript));
check('MSI disables RestartManager prompt (no "close applications" dialog)', /MSIRESTARTMANAGERCONTROL.*Disable/.test(wixScript));
check('MSI silently stops MemorySentry before file ops (taskkill, no CloseApplication element)', /StopMemorySentry/.test(wixScript) && /taskkill/.test(wixScript) && !/<util:CloseApplication/.test(wixScript));
check('MSI uses hidden WixQuietExec for all deferred actions (no console flash)', /WixQuietExec/.test(wixScript) && !/ExeCommand/.test(wixScript));
check('MSI passes CustomAppsFolder+name (avoids trailing-backslash CA bug)', wixScript.includes('[CustomAppsFolder]MemorySentryForSpicetify'));
check('MSI runs --configure and --pre-uninstall via custom actions', /--configure/.test(wixScript) && /--pre-uninstall/.test(wixScript));
check('MSI shows the real GPL-3.0 license, not the WiX placeholder', /WixUILicenseRtf/.test(wixScript) && fs.existsSync(path.join(root, 'installer/wix/license-gpl3.rtf')) && read('installer/wix/license-gpl3.rtf').includes('GNU GENERAL PUBLIC LICENSE') && !/Lorem ipsum/i.test(read('installer/wix/license-gpl3.rtf')));
check('MSI uninstall cleanup CAs guard against MajorUpgrade collision', (wixScript.match(/NOT UPGRADINGPRODUCTCODE/g) || []).length >= 3);
check('MSI removes legacy Inno folder and registry key on install', /RemoveLegacyFolder/.test(wixScript) && /RemoveLegacyRegistry/.test(wixScript));
check('server detaches stdio before spawning the monitor (prevents QuietExec hang)', rustServer.includes('detach_stdio_from_children'));
const sbbBody = rustServer.slice(rustServer.indexOf('fn start_background_copy()'), rustServer.indexOf('fn start_background_copy') + 1500);
check('server writes the monitor log to data_dir, not the program folder', sbbBody.includes('data_dir()'));
check('configure migrates legacy layout before reading saved settings', configureBody.includes('migrate_legacy_layout()') && configureBody.indexOf('migrate_legacy_layout()') < configureBody.indexOf('let existing_raw'));
check('build script compiles WiX MSI', /\$WixCommand.*build/.test(buildScript) && /WixToolset\.UI\.wixext/.test(buildScript) && /\.msi/.test(buildScript));
check('packaging script builds native x64 and arm64 servers and MSIs', /x86_64-pc-windows-msvc/.test(buildScript) && /aarch64-pc-windows-msvc/.test(buildScript) && /WixArch/.test(buildScript) && /-arch \$arch\.WixArch/.test(buildScript));
check('CI installs the aarch64 Windows target in both workflows', /rustup target add aarch64-pc-windows-msvc/.test(validation) && /rustup target add aarch64-pc-windows-msvc/.test(read('.github/workflows/release.yml')));

check('CI runs package validation', validation.includes('node tests/validate-package.js'));
check('release versions are aligned on 1.0.0-rc.6', rustCargo.includes('version = "1.0.0-rc.6"') && read('server/Cargo.lock').includes('version = "1.0.0-rc.6"') && rustServer.includes('const VERSION: &str = "1.0.0-rc.6";') && read('README.md').includes('1.0.0-rc.6'));
check('no doc still claims the Audio ceiling is fixed in automatic mode', ['README.md','SECURITY.md','docs/TROUBLESHOOTING.md','docs/SERVER-MIGRATION.md','docs/MARKETPLACE.md'].every(f => !/not adaptively adjusted|excluded from adaptive adjustments|without adaptive adjustments|excludes them from adaptive cap adjustments/i.test(read(f))));
check('server rejects a mismatched CPU architecture at configure time', read('server/crates/Windows/src/lib.rs').includes('GetNativeSystemInfo') && rustServer.includes('windows::native_arch()'));
check('semantic prereleases are never marked as latest stable', read('.github/workflows/release.yml').includes("contains(steps.version.outputs.version, '-')") && read('.github/workflows/release.yml').includes("!contains(steps.version.outputs.version, '-')"));
check('Rust scaffold uses a minimal standard-library dependency policy (only internal workspace crates, no third-party)', rustCargo.includes('name = "memory-sentry-server"') && (read('server/Cargo.lock').match(/^name = "[^"]+"/gm) || []).every(n => n === 'name = "memory-sentry-server"' || n === 'name = "memory-sentry-core"' || n === 'name = "memory-sentry-windows"' || n === 'name = "memory-sentry-spotify"'));
check('Rust policy has pure, testable adaptive logic', rustLib.includes('decide_adjustment') && rustLib.includes('working_set_at_ceiling') && rustLib.includes('#[cfg(test)]'));
check('Rust binary exposes the live configure, monitor, launch, swap and uninstall commands', ['--check-os', '--configure', '--monitor', '--launch', '--swap', '--pre-uninstall', '--restore-limits'].every(command => rustMain.includes(command)));
check('server can be swapped in place while Spotify keeps running', rustMain.includes('"--swap"') && rustServer.includes('pub fn swap_server') && buildScriptSwap.includes('--swap'));
check('dashboard offers ceiling-only and maximum release modes', /trimToCap/.test(app) && /releaseCeiling/.test(app) && /trimToCap/.test(rustServer) && /trim_to_cap/.test(rustServer));
check('dashboard keeps diagnostics behind a debug mode toggle', /'mss\.debugMode'/.test(app) && /debugMode:/.test(app) && /mss-diag/.test(app) && /debug \? h\('div', \{ className: 'mss-diag-body' \}/.test(app) && app.includes("tx('events')"));
check('dashboard debug switch is a self-contained accessible switch', /MSSDebugToggle/.test(app) && /className: 'mss-switch'/.test(app) && /role: 'switch'/.test(app) && /aria-checked/.test(app) && !/Spicetify\.ReactComponent\.Toggle/.test(app));
check('server discovers audio sessions through the Core Audio session API', rustWindows.includes('pub fn audio_session_pids') && rustWindows.includes('CoCreateInstance') && rustWindows.includes('IID_IAUDIO_SESSION_CONTROL2') && rustServer.includes('refresh_audio_session_pids') && rustServer.includes('audio_pids'));
check('status payload reports the server as Server, not Rust', /"node\\":\\"Server\\",\\"runtime\\":\\"Server\\"/.test(rustServer));
check('Rust runtime implements local API, automatic limits and persistent role profiles', rustServer.includes('/api/status') && rustServer.includes('/api/config') && rustServer.includes('/api/action') && rustServer.includes('windows::set_working_set_limits') && rustServer.includes('restore_all') && rustServer.includes('apply_manual_cap') && rustServer.includes('manualCapMB') && rustServer.includes('manualRoleCaps'));
check('Rust policy keeps the Audio ceiling adaptive from the uniform 38 MB start up to 60 MB', rustLib.includes('Self::Audio => Some(60)') && rustLib.includes('pub const INITIAL_CAP_MB: u32 = 38;') && !rustLib.includes('Self::Audio => None') && !rustServer.includes('audioFixedCapMB') && !rustServer.includes('enforce_fixed_audio_cap') && !rustServer.includes('protectedAudio'));
check('manual mode applies persistent per-role overrides across changing process IDs', rustServer.includes('manual_cap_for') && rustServer.includes('manual_default_cap_mb') && rustServer.includes('configured_manual_cap') && rustServer.includes('manual_role_caps'));
check('server resolves split program and data folders, migrates the old layout and survives a corrupt settings file', rustServer.includes('fn data_dir()') && rustServer.includes('fn program_dir()') && rustServer.includes('fn migrate_legacy_layout()') && rustServer.includes('fn bootstrap_default_settings'));
const prepareUpgradeBody = rustServer.slice(rustServer.indexOf('pub fn prepare_upgrade()'), rustServer.indexOf('pub fn swap_server('));
check('upgrade stops the running server before migrating data', prepareUpgradeBody.indexOf('force_terminate_installed_process') >= 0 && prepareUpgradeBody.indexOf('force_terminate_installed_process') < prepareUpgradeBody.indexOf('migrate_legacy_layout'));
check('migration copies data without deleting the old folder', prepareUpgradeBody.includes('migrate_legacy_layout()') && !rustServer.slice(rustServer.indexOf('fn migrate_legacy_layout()'), rustServer.indexOf('fn parse_json_string')).includes('remove_dir_all'));
check('the MSI lays the program under Programs and the Custom App under spicetify\\CustomApps', /Directory Id="ProgramsFolder" Name="Programs"/.test(wixScript) && /Id="INSTALLFOLDER" Name="MemorySentryForSpicetify"/.test(wixScript) && /Id="CUSTOMAPPFOLDER" Name="MemorySentryForSpicetify"/.test(wixScript));
check('automatic and manual limits are restricted to background window states', rustServer.includes('fn should_manage_limits(settings: &Settings, window_state: &str)') && rustServer.includes('"background-visible"') && rustServer.includes('should_manage_limits(&state.settings, &state.window_state)'));
check('recovery backups record process creation identity and whether each cap was manual', rustServer.includes('records.insert((pid, creation), record)') && rustServer.includes('manual: fields') && rustServer.includes('if r.manual { "manual" }'));
check('settings no longer persist PID-keyed manual caps and the schema is bumped to v4', !rustServer.includes('settings.manual_caps') && !rustServer.includes('manual_caps: BTreeMap') && !rustServer.includes('parse_manual_caps') && rustServer.includes('const CURRENT_SCHEMA_VERSION: u32 = 4') && !/"manualCaps"/.test(rustServer.split('fn write_settings')[1] || ''));
check('stale restore records are pruned once instead of retried every poll (no PID restore flood)', rustServer.includes('let mut pruned = false') && rustServer.includes('state.active_caps.remove(&record.pid)') && rustServer.includes('Dropped no-longer-eligible PID'));
check('policy transitions restore active limits before switching modes', rustServer.includes('next.auto_optimize != state.settings.auto_optimize') && rustServer.includes('policy mode changed') && rustServer.includes('failed > 0'));
check('server Windows layer protects process identity before each working-set mutation', rustWindows.includes('set_working_set_limits') && rustWindows.includes('creation != expected_creation') && rustWindows.includes('restore_working_set_limits'));
check('Rust startup and instance locking are per-user and single-instance', rustWindows.includes('acquire_single_instance') && rustServer.includes('register_startup') && rustServer.includes('127.0.0.1'));
check('Rust migration stops the legacy monitor before invoking its one-shot restore command', (rustServer.match(/stop_legacy_node_helper\(&\w+\)\?;/g) || []).length === 2);
check('Rust dashboard port remains consistent with the Custom App endpoint', /const MSS_API = 'http:\/\/127\.0\.0\.1:38472'/.test(app) && /port: DEFAULT_PORT/.test(rustServer));


const warningSecondsSource = app.match(/function MSSManualWarningSeconds\(readyAt, now\) \{[\s\S]*?\n\}/)?.[0];
if (warningSecondsSource) {
  check('below-default warning countdown lasts five seconds',
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 0)`) === 5 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 4999)`) === 1 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 5000)`) === 0);
} else check('below-default warning countdown function exists', false);

for (const [name, source] of [['app/index.js + subfiles', appScript]]) {
  try { new vm.Script(source, { filename: name }); check(`JavaScript syntax: ${name}`, true); }
  catch (error) { check(`JavaScript syntax: ${name} (${error.message})`, false); }
}
const appSandbox = {
  Spicetify: {
    React: { createElement: () => null, useState: value => [value, () => {}], useEffect: () => {}, useRef: value => ({ current: value }), useCallback: fn => fn },
    ReactComponent: {}, PopupModal: {}, Platform: {}, SVGIcons: {}, Config: {},
    LocalStorage: { get: () => null, set: () => {} },
    showNotification: () => {}
  },
  document: {
    getElementById: () => null, querySelector: () => null, head: { appendChild: () => {} }, body: { appendChild: () => {} },
    createElement: () => ({ style: {}, setAttribute: () => {}, appendChild: () => {}, append: () => {}, addEventListener: () => {}, remove: () => {} })
  },
  window: { open: () => {}, addEventListener: () => {}, localStorage: { getItem: () => null, setItem: () => {} }, setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {} },
  navigator: { language: 'en-US' },
  fetch: () => Promise.reject(new Error('no network during validation')),
  setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
  console
};
try {
  vm.runInNewContext(appScript, appSandbox, { filename: 'app-bundle.js' });
  check('Custom App registers render() with a stubbed Spicetify and no DOM access at load', typeof appSandbox.render === 'function');
} catch (error) {
  check(`Custom App loads with a stubbed Spicetify (${error.message})`, false);
}
// Comment hygiene (a shipped-source invariant, not a one-off cleanup): product comments must
// explain WHY an optimization holds or what invariant a value protects - never cite the internal
// workflow ledger (learning/pitfall/decision/research idNN) or transient issue numbers (#NN),
// which are meaningless outside this repo and go stale the moment they are written. Public
// references (Chrome, Android lmkd / App Standby Buckets, PRIVACY.md) are fine and kept.
const shippedSource = [appScript, appStyles, rustLib, rustSpotify, rustMain, rustWindows, rustServer].join('\n');
const ledgerRef = /\b(?:learning|pitfall|decision|research)\s+id\d+\b|\bid\d{2,4}\b/;
const issueRef = /(?<!:)#\d{1,3}\b/;
const ledgerHits = shippedSource.split('\n').filter(line => ledgerRef.test(line));
// Only flag a bare #NN inside a comment (never the `Role:#N` identity sub-label form, hence the
// (?<!:) lookbehind) so CSS hex colors (#1ed760) and Rust attributes (#[...]) stay clean.
const issueHits = shippedSource.split('\n').filter(line => {
  const t = line.trim();
  const isComment = t.startsWith('//') || t.startsWith('///') || t.startsWith('*') || t.startsWith('/*') || /\s\/\/\s/.test(line);
  return isComment && issueRef.test(line);
});
check(`shipped comments cite no internal ledger ids (found: ${ledgerHits.map(l => l.trim()).join(' | ') || 'none'})`, ledgerHits.length === 0);
check(`shipped comments cite no transient issue numbers (#NN) (found: ${issueHits.map(l => l.trim()).join(' | ') || 'none'})`, issueHits.length === 0);

if (failures) process.exitCode = 1;
else console.log('\nAll static package checks passed. Win32 behavior still requires a Windows runtime test.');
