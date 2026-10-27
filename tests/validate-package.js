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
  'CHANGELOG.md', 'docs/BENCHMARKING.md', 'docs/MARKETPLACE.md', 'docs/INSTALLATION.md', 'docs/SERVER-MIGRATION.md', 'docs/RELEASE_CHECKLIST.md', 'docs/DEVELOPMENT.md', 'docs/TROUBLESHOOTING.md', 'installer/inno/MemorySentry.iss',   'scripts/build-package.ps1', 'scripts/swap-server.ps1', 'server/src/main.rs', 'server/src/lib.rs', 'server/src/runtime.rs', 'server/src/platform/windows.rs', 'assets/marketplace-cover.svg', 'assets/readme-preview.svg', 'assets/readme-banner.svg',
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
check('root manifest is Marketplace metadata', marketplaceManifest.name === 'Memory Sentry' && marketplaceManifest.preview === 'assets/marketplace-cover.svg' && marketplaceManifest.readme === 'README.md' && !marketplaceManifest.main);
check('Marketplace metadata uses the selected discovery tags and author attribution', JSON.stringify(marketplaceManifest.tags) === JSON.stringify(['memory', 'windows', 'experimental']) && marketplaceManifest.authors?.some(author => author.url === 'https://github.com/yanlvl99'));
check('artwork uses standalone SVGs with vector elements and no external PNG links', [['marketplace-cover.svg', '1600', '1600'], ['readme-preview.svg', '1600', '1000'], ['readme-banner.svg', '1800', '700']].every(([file, width, height]) => { const svg = read(`assets/${file}`); return /^<svg\b/.test(svg) && new RegExp(`width="${width}"`).test(svg) && new RegExp(`height="${height}"`).test(svg) && /<path\b/.test(svg) && !/href=["'](?!data:)[^"']*\.png/i.test(svg); }));
check('PNG sources, archive and obsolete screenshot assets were removed', !fs.readdirSync(path.join(root, 'assets')).some(file => /\.png$/i.test(file) || /\.zip$/i.test(file)) && ['COM.png', 'SEM.png', 'assets/COM-sharp.svg', 'assets/SEM-sharp.svg', 'assets/marketplace-banner.svg', 'assets/marketplace-preview-sharp.svg', 'preview.png', 'assets/marketplace-preview.svg'].every(file => !fs.existsSync(path.join(root, file))));
check('README keeps the banner and omits the obsolete comparison preview', /assets\/readme-banner\.svg/.test(read('README.md')) && !/assets\/readme-preview\.svg/.test(read('README.md')) && !/not a controlled benchmark/i.test(read('README.md')));
check('Marketplace description explains minimized RAM savings and the restore-time trade-off', /Reduce Spotify's RAM usage while minimized/i.test(marketplaceManifest.description) && /slower interface loading when restored/i.test(marketplaceManifest.description) && marketplaceManifest.description.length <= 120);
check('official Setup declares supported Windows 10/11 desktop only', /Windows 10/.test(read('installer/inno/MemorySentry.iss')) && /Windows 11/.test(read('installer/inno/MemorySentry.iss')) && /detect_supported_client_os/.test(read('server/src/runtime.rs')));
check('local publishing helper and generated archives are ignored', /\/PUSH-TO-GITHUB\.bat/.test(read('.gitignore')) && /\*\.bundle/.test(read('.gitignore')) && /\*\.zip/.test(read('.gitignore')));
check('development guide documents Rust build and package validation without local publishing scripts', /cargo test/.test(read('docs/DEVELOPMENT.md')) && /scripts\/build-package\.ps1/.test(read('docs/DEVELOPMENT.md')) && !/PUSH-TO-GITHUB\.bat/i.test(read('docs/DEVELOPMENT.md')));
check('README explains server requirement', /Memory Sentry server/i.test(read('README.md')) && /separate per-user process/i.test(read('README.md')));
check('README documents adaptive CPU and headroom rules', /six consecutive seconds/i.test(read('README.md')) && /ten seconds/i.test(read('README.md')) && /full current working-set ceiling/i.test(read('README.md')));
check('README documents the master switch, shared delay and five-second warning', /dashboard power switch is the master on\/off control/i.test(read('README.md')) && /same configured background delay applies in both modes/i.test(read('README.md')) && /five-second warning countdown/i.test(read('README.md')));
check('changelog uses the current five-second manual warning', /five-second warning countdown/i.test(read('CHANGELOG.md')) && !/15-second warning countdown/i.test(read('CHANGELOG.md')));
check('docs distinguish the fixed automatic Audio ceiling from its manual default', /fixed 36 MB automatic ceiling/i.test(read('docs/RELEASE_CHECKLIST.md')) && /manual mode.*36 MB is the Audio default and can be overridden per process role/i.test(read('docs/SERVER-MIGRATION.md')) && /manual mode uses 36 MB as the default and supports persistent role-wide overrides/i.test(read('docs/MARKETPLACE.md')));
check('troubleshooting uses the master switch to stop all background limits', /turn \*\*Memory Sentry off\*\* with the dashboard power switch/i.test(read('docs/TROUBLESHOOTING.md')) && /Turning off only \*\*Automatic policy\*\* selects manual mode/i.test(read('docs/TROUBLESHOOTING.md')));
check('issue templates request current policy state and use the correct log files', /visible in background.*automatic\/manual mode.*selected background delay/.test(read('.github/ISSUE_TEMPLATE/server-bug.yml')) && ['setup.log', 'startup.log', 'helper.log'].every(name => read('.github/ISSUE_TEMPLATE/server-bug.yml').includes(`\`${name}\``)) && /dashboard power state, automatic\/manual mode, selected background delay/.test(read('.github/ISSUE_TEMPLATE/plugin-bug.yml')) && /Windows 11 24H2/.test(read('.github/ISSUE_TEMPLATE/plugin-bug.yml')));
check('dashboard distinguishes master power from automatic/manual policy', /dashboard power switch turns memory management on or off/i.test(read('app/index.js')) && /botão liga\/desliga do painel ativa ou desativa o gerenciamento/i.test(read('app/index.js')));
check('README documents unsigned SmartScreen limitations', /No unsigned release can guarantee zero SmartScreen/i.test(read('README.md')));
check('GPL-3.0 license is present', /GNU GENERAL PUBLIC LICENSE/i.test(read('LICENSE')) && /Version 3, 29 June 2007/i.test(read('LICENSE')));
check('privacy explicitly states no uploads', /No information is uploaded/i.test(read('PRIVACY.md')));
check('security notes never disable antivirus', /does not disable antivirus/i.test(read('SECURITY.md')));
check('Marketplace preview exists', fs.existsSync(path.join(root, marketplaceManifest.preview)));
check('release publishes only the official Setup EXE and SHA-256', /MemorySentrySetup-\*\.exe/.test(read('.github/workflows/release.yml')) && /MemorySentrySetup-\*\.exe\.sha256/.test(read('.github/workflows/release.yml')) && !/MemorySentry-\*windows-x64\.zip/.test(read('.github/workflows/release.yml')) && /action-gh-release/i.test(read('.github/workflows/release.yml')));
check('build output has one supported installer and no custom portable ZIP', /MemorySentrySetup-/.test(read('scripts/build-package.ps1')) && !/Compress-Archive/.test(read('scripts/build-package.ps1')) && !/MemorySentry-.*windows-x64\.zip/.test(read('scripts/build-package.ps1')));

const appIndex = read('app/index.js');
const app = [...appSubfiles.map(file => read(`app/${file}`)), appIndex].join('\n;\n');
check('Custom App source avoids ES module syntax because subfiles are concatenated', !/^\s*(?:import|export)\b/m.test(app));
const setupScript = read('installer/inno/MemorySentry.iss');
const buildScript = read('scripts/build-package.ps1');
const buildScriptSwap = read('scripts/swap-server.ps1');
const validation = read('.github/workflows/validate.yml');
const rustCargo = read('server/Cargo.toml');
const rustLib = read('server/src/lib.rs');
const rustMain = read('server/src/main.rs');
const rustRuntime = read('server/src/runtime.rs');
const rustPlatform = read('server/src/platform/windows.rs');
check('dashboard supports automatic Portuguese and English', /navigator\.language/i.test(app) && /pt-BR/i.test(app) && /Auto language: English/i.test(app));
check('dashboard has a GitHub release button when helper is absent', /MSSOpenGitHub/i.test(app) && /github\.com\/yanlvl99\/spicetify-memory-sentry/i.test(app));
check('dashboard offers setup/help links without a Node.js button', /MSSOpenGitHub/.test(app) && /MSSOpenInstallGuide/.test(app) && !/MSSOpenNode|nodeButton|nodejs\.org/.test(app));
check('dashboard links to official releases and a generic installation guide', /MSS_RELEASES_URL/.test(app) && /MSS_INSTALL_GUIDE_URL/.test(app) && /installGuideButton/.test(app) && !/Marketplace setup guide|Guia de instalação do Marketplace/.test(app));
check('documentation describes one install path and Marketplace linking only', /only supported installation method/i.test(read('docs/INSTALLATION.md')) && /only links to the project repository/i.test(read('docs/INSTALLATION.md')) && !/Path B/.test(read('docs/INSTALLATION.md')));
check('dashboard offers persistent role-wide manual caps only in saved manual mode', /manualReady = config\.enabled && !config\.autoOptimize && savedAutoOptimize === false/.test(app) && /setManualCap/.test(app) && /clearManualCap/.test(rustRuntime) && rustRuntime.includes('manual_role_caps') && /after Spotify restarts and their PIDs change/i.test(app));
check('dashboard automatically saves policy and release setting changes', app.includes('function setField(key, value)') && app.includes('saveSettings(nextConfig)') && app.includes("setField('autoOptimize', event.target.checked)") && app.includes("tx(saving ? 'saving' : 'autoSaved')") && !app.includes("tx('save')"));
check('manual limit inputs auto-apply, show saved role values and reset after restore', app.includes('scheduleManualLimit(process, key, value)') && app.includes('value: manualInputs[key] ?? String(process.manualCapMB ?? defaultMB)') && app.includes('await clearManualLimit(process, key)') && app.includes("windowState === 'foreground'") && app.includes('setManualInputs({}'));
check('below-default warning requires confirmation after five seconds', /Date\.now\(\) \+ 5000/.test(app) && /Spicetify\.PopupModal/.test(app) && /popup\.display\(\{ title, content: panel, isLarge: false \}\)/.test(app) && /popup\.hide\(\)/.test(app) && /tx\('waitBeforeApply', \{ seconds \}\)/.test(app) && /confirmButton\.disabled = false/.test(app) && /confirmManualLimit\(warning, countdownComplete\)/.test(app) && /if \(!countdownComplete\) return/.test(app) && /void applyManualLimit\(\{ pid: warning\.pid, creation: warning\.creation, role: warning\.role \}, warning\.capMB\)/.test(app) && !/hideModal\(\);\s*setManualWarning\(null\);\s*void applyManualLimit/.test(app));
check('dashboard relies on automatic restore and polling instead of action buttons', !app.includes("tx('applyManual')") && !app.includes("tx('restoreNow')") && !app.includes("tx('refresh')"));
check('dashboard sends process creation identity with manual limit actions', /creation: process\.creation/.test(app) && rustRuntime.includes('\\"creation\\":{},'));
check('dashboard shows per-process CPU and active ceiling', /process\.cpuPercent/i.test(app) && /process\.limitMB/i.test(app));
check('dashboard uses private working-set memory when Windows provides it', rustPlatform.includes('ProcessMemoryCountersEx2') && rustPlatform.includes('private_working_set_size') && rustRuntime.includes('privateWorkingSetMB') && rustRuntime.includes('private_working_set_available') && app.includes("memory.privateWorkingSet ? process.privateWorkingSetMB : process.workingSetMB") && app.includes("memory.privateWorkingSet ? 'privateWorkingSetRam' : 'spotifyRam'"));
check('dashboard summary avoids duplicate memory and foreground-inactive limit counts', app.includes("card(tx('spotifyProcesses'), Number(memory.targetCount || 0).toLocaleString(MSS_LOCALE), '')") && !app.includes('optimizedRam') && !app.includes('activeLimits') && !app.includes('manualLimitsCard') && !app.includes('activeCount'));
check('dashboard summary grid adapts to its remaining cards', app.includes('grid-template-columns:repeat(auto-fit,minmax(min(100%,300px),1fr))'));
check('low-limit modal warning and confirmation use high-contrast red styling', app.includes('mss-btn-danger') && app.includes('.mss-modal .mss-notice-warn') && app.includes('#ff5252'));
check('closing a low-limit warning restores the prior textbox value', app.includes('setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }))') && app.includes('modalObserver.observe(document.body, { childList: true, subtree: true })') && app.includes('closeWarning(true)') && app.includes('confirmedManualWarning.current = warning.readyAt'));
check('global plugin switch persists enabled state and runs activation transitions', /role: 'switch',[^}]*checked: Boolean\(config\.enabled\)/.test(app) && /saveSettings\(\{ \.\.\.config, enabled: Boolean\(enabled\) \}\)/.test(app) && /showScreenFlash\(Boolean\(enabled\)\)/.test(app) && /\\"enabled\\": \{\},/.test(rustRuntime) && rustRuntime.includes('if !state.settings.enabled'));
check('configuration failures do not masquerade as a lost server connection', app.includes('setSettingsError(err?.message || tx(\'saveFailed\'))') && !app.includes("setError(tx('saveFailed'))") && rustRuntime.includes('if failed > 0 && !disabling'));
check('delay and release options remain enabled for manual policy', /disabled: saving \|\| !helperRunning \|\| !config\.enabled, value: String\(config\.hiddenDelaySeconds\)/.test(app) && /disabled: saving \|\| !helperRunning \|\| !config\.enabled, value: config\.trimToCap/.test(app) && rustRuntime.includes('background_delay_elapsed') && rustRuntime.includes('one-time trim requested'));
check('plugin disabled state blocks manual controls and automatic policy settings', app.includes('disabled: busy || !config.enabled') && app.includes('disabled: saving || !helperRunning || !config.enabled') && app.includes('pluginDisabledNote') && rustRuntime.includes('Enable Memory Sentry before setting a manual process limit.'));
check('background settings are grouped and described as shared across modes', app.includes('mss-setting-group-head') && app.includes("backgroundSettingsHint: 'These options are shared by automatic and manual modes.'") && app.includes("backgroundSettingsHint: 'Estas opções são compartilhadas pelos modos automático e manual.'"));
check('dashboard header aligns status and switches in a responsive control panel', app.includes('mss-hero-controls') && app.includes('mss-debug-row') && app.includes('mss-switch-track') && app.includes('.mss-hero{display:grid;grid-template-columns') && app.includes('@media(max-width:680px)'));
check('dashboard removes entrance and hover movement and softens activation feedback', app.includes('.mss-page,.mss-grid>.mss-card{animation:none}') && app.includes('.mss-card:hover{transform:none') && app.includes('mss-screen-transition .52s ease-out') && app.includes('mss-screen-flash-on::before{background:color-mix(in srgb,var(--spice-button,#1ed760) 12%,transparent)') && app.includes('mss-screen-flash-off::before{background:color-mix(in srgb,#e05252 11%,transparent)') && app.includes('@media(prefers-reduced-motion:reduce)'));
check('dashboard explains local-only diagnostics', /No telemetry is uploaded/i.test(app));
check('Custom App replaces per-install token placeholder', app.includes('__MSS_TOKEN__'));
check('Custom App calls local status/config endpoints', app.includes('/api/status') && app.includes('/api/config'));

check('official Setup uses server preflight and recovery-aware commands', /MemorySentry\.exe/.test(setupScript) && /--check-install/.test(setupScript) && /--check-spicetify/.test(setupScript) && /--prepare-upgrade/.test(setupScript) && /--pre-uninstall/.test(setupScript));
check('Inno Setup packages the app and guards install/upgrade/uninstall', /PrivilegesRequired=lowest/.test(setupScript) && /--check-os/.test(setupScript) && /--prepare-upgrade/.test(setupScript) && /InitializeUninstall/.test(setupScript) && /--pre-uninstall/.test(setupScript));
check('build script compiles Inno setup and writes checksums', /ISCC\.exe/.test(buildScript) && /SHA256/.test(buildScript) && /\.sha256"/.test(buildScript) && /MemorySentrySetup-/.test(buildScript));
check('dashboard preserves detailed local API failures', app.includes('err && err.message') && app.includes("setError(tx('statusError') + ' [' + detail + ']')"));
check('dashboard renders actual connection diagnostics', app.includes("`${tx('helperMissingBody')} ${error}`"));
check('Rust uninstall allows Spotify to remain open and restores first', !/pub fn pre_uninstall\(\) -> Result<(), String> \{\s*ensure_spotify_closed/.test(rustRuntime) && rustRuntime.indexOf('pub fn pre_uninstall()') < rustRuntime.indexOf('unregister_startup()?;') && rustRuntime.indexOf('unregister_startup()?;') < rustRuntime.indexOf('unregister_spicetify()?;'));
check('single-instance mutex clears stale Win32 last-error state', rustPlatform.includes('SetLastError(0)'));
check('Rust launcher records early startup failures and verifies API readiness', rustRuntime.includes('startup.log') && rustRuntime.includes('request_status(settings.port, &settings.token)') && rustRuntime.includes('failed to become ready after 5 seconds'));
check('Rust configure failures are logged for the installer dialog', rustMain.includes('log_setup_failure(&error)') && rustMain.includes('setup.log') && setupScript.includes('SetupFailureDetails()'));
check('Inno Setup reads its ANSI diagnostic buffer with the required type', /LogText:\s*AnsiString/.test(setupScript) && /LoadStringFromFile\(LogFile, LogText\)/.test(setupScript));
check('installer shows a dedicated missing-Spicetify prerequisite message', rustMain.includes('"--check-spicetify"') && setupScript.includes("RunBundledServer('--check-spicetify', Code)") && setupScript.includes('SpicetifyMissing'));
check('single Setup flow does not require Spotify to be manually closed', !/ensure_spotify_closed/.test(rustRuntime) && !/Get-Process -Name Spotify/.test(rustRuntime) && /CloseApplications=no/.test(setupScript));
const configureBody = rustRuntime.slice(rustRuntime.indexOf('pub fn configure_install('), rustRuntime.indexOf('fn find_node()'));
check('local API starts and passes readiness before Spicetify apply', configureBody.indexOf('launch_monitor()') >= 0 && configureBody.indexOf('launch_monitor()') < configureBody.indexOf('apply_spicetify(&cli)'));
check('PID waits verify the actual Memory Sentry executable path', rustRuntime.includes('process_matches_executable(pid, &installed_exe)') && rustPlatform.includes('QueryFullProcessImageNameW'));
check('failed API bind removes helper.pid', rustRuntime.includes('if let Err(error) = start_api(Arc::clone(&shared))') && rustRuntime.includes('let _ = fs::remove_file(&pid_file);'));
const launcherBody = rustRuntime.slice(rustRuntime.indexOf('pub fn launch_monitor()'));
check('Server launcher creates startup diagnostics before reading settings', launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') >= 0 && launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') < launcherBody.indexOf('load_settings(&settings_path)') && launcherBody.includes('Could not load server settings'));
check('installer always launches the server after setup', setupScript.includes('Flags: runhidden') && !setupScript.includes('Flags: postinstall nowait skipifsilent runhidden'));
check('redundant batch and PowerShell installer entry points are absent', ['Install.bat','Uninstall.bat','installer/Install.ps1','installer/Uninstall.ps1'].every(file => !fs.existsSync(path.join(root, file))));
check('official Setup is per-user and does not request elevation', /PrivilegesRequired=lowest/.test(setupScript) && !/RunAs|Verb\s+RunAs/i.test(setupScript));
check('Inno uninstaller invokes recovery-aware server pre-uninstall', /InitializeUninstall/.test(setupScript) && /RunInstalledServer\('--pre-uninstall', Code\)/.test(setupScript) && /Recovery data exists but the server executable is missing/.test(setupScript));
check('source Inno installer preserves app recovery guard', /Recovery data exists but the server executable is missing/.test(setupScript) && /No program files were removed/.test(setupScript));

check('CI runs package validation', validation.includes('node tests/validate-package.js'));
check('release versions are aligned on 1.0.0-rc.2', rustCargo.includes('version = "1.0.0-rc.2"') && read('server/Cargo.lock').includes('version = "1.0.0-rc.2"') && rustRuntime.includes('const VERSION: &str = "1.0.0-rc.2";') && setupScript.includes('#define AppVersion "1.0.0-rc.2"') && setupScript.includes('VersionInfoVersion=1.0.0.2') && read('README.md').includes('1.0.0-rc.2') && app.includes('v1.0.0-rc.2'));
check('semantic prereleases are never marked as latest stable', read('.github/workflows/release.yml').includes("contains(steps.version.outputs.version, '-')") && read('.github/workflows/release.yml').includes("!contains(steps.version.outputs.version, '-')"));
check('Rust scaffold uses a minimal standard-library dependency policy', rustCargo.includes('name = "memory-sentry-server"') && !rustCargo.includes('[dependencies]'));
check('Rust policy has pure, testable adaptive logic', rustLib.includes('decide_adjustment') && rustLib.includes('working_set_at_ceiling') && rustLib.includes('#[cfg(test)]'));
check('Rust binary exposes OS, install-preflight, upgrade, configure, monitor, and uninstall commands', ['--check-os', '--check-install', '--check-spicetify', '--prepare-upgrade', '--configure', '--monitor', '--pre-uninstall', '--restore-limits'].every(command => rustMain.includes(command)));
check('server can be swapped in place while Spotify keeps running', rustMain.includes('"--swap"') && rustRuntime.includes('pub fn swap_server') && buildScriptSwap.includes('--swap'));
check('dashboard offers ceiling-only and maximum release modes', /trimToCap/.test(app) && /releaseCeiling/.test(app) && /trimToCap/.test(rustRuntime) && /trim_to_cap/.test(rustRuntime));
check('dashboard keeps diagnostics behind a debug mode toggle', /'mss\.debugMode'/.test(app) && /debugMode:/.test(app) && /debug \? card\(tx\('pageFaults'\)/.test(app) && /debug \? h\('section', \{ className: 'mss-section' \}, h\('div', \{ className: 'mss-section-head' \}, h\('h2', \{ className: 'mss-h2' \}, tx\('events'\)\)/.test(app));
check('dashboard debug switch uses the Spicetify Toggle component with a fallback', /ReactComponent\.Toggle/.test(app) && /MSSDebugToggle/.test(app) && /typeof Toggle !== 'function'/.test(app));
check('server discovers audio sessions through the Core Audio session API', rustPlatform.includes('pub fn audio_session_pids') && rustPlatform.includes('CoCreateInstance') && rustPlatform.includes('IID_IAUDIO_SESSION_CONTROL2') && rustRuntime.includes('refresh_audio_session_pids') && rustRuntime.includes('audio_pids'));
check('status payload reports the server as Server, not Rust', /"node\\":\\"Server\\",\\"runtime\\":\\"Server\\"/.test(rustRuntime));
check('Rust runtime implements local API, automatic limits and persistent role profiles', rustRuntime.includes('/api/status') && rustRuntime.includes('/api/config') && rustRuntime.includes('/api/action') && rustRuntime.includes('platform::set_working_set_limits') && rustRuntime.includes('restore_all') && rustRuntime.includes('apply_manual_cap') && rustRuntime.includes('manualCapMB') && rustRuntime.includes('manualRoleCaps'));
check('Rust automatic policy caps Main and Audio at 36 MB', rustLib.includes('Self::Main | Self::Audio => Some(36)') && rustLib.includes('Self::Main => Some(36)') && rustLib.includes('Self::Audio => None') && rustRuntime.includes('audioFixedCapMB'));
check('manual mode applies persistent per-role overrides across changing process IDs', rustRuntime.includes('manual_cap_for') && rustRuntime.includes('manual_default_cap_mb') && rustRuntime.includes('configured_manual_cap') && rustRuntime.includes('manual_role_caps'));
check('automatic and manual limits are restricted to background window states', rustRuntime.includes('should_manage_limits_in_background') && rustRuntime.includes('"background-visible"'));
check('manual caps persist process creation identity and are recorded in recovery backups', rustRuntime.includes('manualCaps') && rustRuntime.includes('manual: fields') && rustRuntime.includes('if r.manual { \"manual\" }'));
check('policy transitions restore active limits before switching modes', rustRuntime.includes('next.auto_optimize != state.settings.auto_optimize') && rustRuntime.includes('policy mode changed') && rustRuntime.includes('failed > 0'));
check('server Windows layer protects process identity before each working-set mutation', rustPlatform.includes('set_working_set_limits') && rustPlatform.includes('creation != expected_creation') && rustPlatform.includes('restore_working_set_limits'));
check('Rust startup and instance locking are per-user and single-instance', rustPlatform.includes('acquire_single_instance') && rustRuntime.includes('register_startup') && rustRuntime.includes('127.0.0.1'));
check('Rust migration stops the legacy monitor before invoking its one-shot restore command', (rustRuntime.match(/stop_legacy_node_helper\(&base\)\?;/g) || []).length === 2);
check('Rust dashboard port remains consistent with the Custom App endpoint', /const MSS_API = 'http:\/\/127\.0\.0\.1:38472'/.test(app) && /port: DEFAULT_PORT/.test(rustRuntime));


const warningSecondsSource = app.match(/function MSSManualWarningSeconds\(readyAt, now\) \{[\s\S]*?\n\}/)?.[0];
if (warningSecondsSource) {
  check('below-default warning countdown lasts five seconds',
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 0)`) === 5 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 4999)`) === 1 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(5000, 5000)`) === 0);
} else check('below-default warning countdown function exists', false);

for (const [name, source] of [['app/index.js + subfiles', app]]) {
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
  vm.runInNewContext(app, appSandbox, { filename: 'app-bundle.js' });
  check('Custom App registers render() with a stubbed Spicetify and no DOM access at load', typeof appSandbox.render === 'function');
} catch (error) {
  check(`Custom App loads with a stubbed Spicetify (${error.message})`, false);
}
if (failures) process.exitCode = 1;
else console.log('\nAll static package checks passed. Win32 behavior still requires a Windows runtime test.');
