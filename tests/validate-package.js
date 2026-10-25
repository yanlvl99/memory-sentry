'use strict';
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const policy = require('../helper/adaptive-policy');
const root = path.resolve(__dirname, '..');
let failures = 0;
function check(name, condition) {
  if (condition) console.log(`PASS ${name}`);
  else { console.error(`FAIL ${name}`); failures++; }
}
function read(relative) { return fs.readFileSync(path.join(root, relative), 'utf8'); }

const requiredFiles = [
  'app/index.js', 'app/manifest.json', 'helper/helper.js', 'helper/adaptive-policy.js',
  'helper/package.json', '.gitignore', 'manifest.json', 'README.md', 'LICENSE', 'PRIVACY.md', 'SECURITY.md',
  'CHANGELOG.md', 'docs/BENCHMARKING.md', 'docs/MARKETPLACE.md', 'docs/INSTALLATION.md', 'docs/RUST-MIGRATION.md', 'docs/RELEASE_CHECKLIST.md', 'docs/DEVELOPMENT.md', 'docs/TROUBLESHOOTING.md', 'installer/inno/MemorySentry.iss',   'scripts/build-package.ps1', 'scripts/swap-server.ps1', 'native/rust/src/main.rs', 'native/rust/src/lib.rs', 'native/rust/src/runtime.rs', 'native/rust/src/platform/windows.rs', 'COM.png', 'SEM.png', 'assets/marketplace-banner.svg', 'assets/marketplace-preview-sharp.svg', 'assets/COM-sharp.svg', 'assets/SEM-sharp.svg',
  '.github/workflows/validate.yml', '.github/workflows/release.yml'
];
for (const file of requiredFiles) check(`exists: ${file}`, fs.existsSync(path.join(root, file)));

let appManifest = {};
let marketplaceManifest = {};
let packageJson = {};
try { appManifest = JSON.parse(read('app/manifest.json')); } catch (_) { check('Custom App manifest JSON', false); }
try { marketplaceManifest = JSON.parse(read('manifest.json')); } catch (_) { check('Marketplace manifest JSON', false); }
try { packageJson = JSON.parse(read('helper/package.json')); } catch (_) { check('helper package JSON', false); }
check('Spicetify Custom App manifest is valid', Boolean(appManifest.name && appManifest.icon && appManifest['active-icon']));
check('root manifest is Marketplace metadata', marketplaceManifest.name === 'Memory Sentry' && marketplaceManifest.preview === 'assets/marketplace-preview-sharp.svg' && marketplaceManifest.readme === 'README.md' && !marketplaceManifest.main);
check('Marketplace metadata has useful discovery tags and author attribution', Array.isArray(marketplaceManifest.tags) && ['spotify', 'performance', 'memory', 'windows'].every(tag => marketplaceManifest.tags.includes(tag)) && marketplaceManifest.authors?.some(author => author.url === 'https://github.com/yanlvl99'));
check('Marketplace banner uses the exact committed COM/SEM screenshots', /^<svg\s/.test(read('assets/marketplace-banner.svg')) && /data:image\/png;base64,/.test(read('assets/marketplace-banner.svg')) && /WITH MEMORY SENTRY/.test(read('assets/marketplace-banner.svg')) && /WITHOUT MEMORY SENTRY/.test(read('assets/marketplace-banner.svg')) && read('assets/marketplace-banner.svg').includes(fs.readFileSync(path.join(root, 'COM.png')).toString('base64')) && read('assets/marketplace-banner.svg').includes(fs.readFileSync(path.join(root, 'SEM.png')).toString('base64')));
check('obsolete mockup preview files were removed', !fs.existsSync(path.join(root, 'preview.png')) && !fs.existsSync(path.join(root, 'assets/marketplace-preview.svg')));
check('sharp Marketplace preview uses the exact COM.png capture', read('assets/marketplace-preview-sharp.svg').includes(fs.readFileSync(path.join(root, 'COM.png')).toString('base64')) && /feConvolveMatrix/.test(read('assets/marketplace-preview-sharp.svg')));
check('README sharp views use the exact COM/SEM captures', read('assets/COM-sharp.svg').includes(fs.readFileSync(path.join(root, 'COM.png')).toString('base64')) && read('assets/SEM-sharp.svg').includes(fs.readFileSync(path.join(root, 'SEM.png')).toString('base64')) && /feConvolveMatrix/.test(read('assets/COM-sharp.svg')) && /feConvolveMatrix/.test(read('assets/SEM-sharp.svg')));
check('README discloses that screenshot comparisons are not controlled benchmarks', /not a controlled benchmark/i.test(read('README.md')) && /COM\.png/.test(read('README.md')) && /SEM\.png/.test(read('README.md')));
check('Marketplace description discloses separate server', /server is installed separately/i.test(marketplaceManifest.description));
check('Marketplace listing explicitly limits support to Windows 10 and 11', /Windows 10 and Windows 11 only/i.test(marketplaceManifest.description));
check('official Setup declares supported Windows 10/11 desktop only', /Windows 10/.test(read('installer/inno/MemorySentry.iss')) && /Windows 11/.test(read('installer/inno/MemorySentry.iss')) && /detect_supported_client_os/.test(read('native/rust/src/runtime.rs')));
check('local publishing helper and generated archives are ignored', /\/PUSH-TO-GITHUB\.bat/.test(read('.gitignore')) && /\*\.bundle/.test(read('.gitignore')) && /\*\.zip/.test(read('.gitignore')));
check('development guide documents Rust build and package validation without local publishing scripts', /cargo test/.test(read('docs/DEVELOPMENT.md')) && /scripts\/build-package\.ps1/.test(read('docs/DEVELOPMENT.md')) && !/PUSH-TO-GITHUB\.bat/i.test(read('docs/DEVELOPMENT.md')));
check('README explains server requirement', /Memory Sentry server/i.test(read('README.md')) && /separate per-user process/i.test(read('README.md')));
check('README documents adaptive CPU and headroom rules', /six seconds/i.test(read('README.md')) && /ten seconds/i.test(read('README.md')));
check('README documents unsigned SmartScreen limitations', /No unsigned release can guarantee zero SmartScreen/i.test(read('README.md')));
check('GPL-3.0 license is present', /GNU GENERAL PUBLIC LICENSE/i.test(read('LICENSE')) && /Version 3, 29 June 2007/i.test(read('LICENSE')));
check('privacy explicitly states no uploads', /No information is uploaded/i.test(read('PRIVACY.md')));
check('security notes never disable antivirus', /does not disable antivirus/i.test(read('SECURITY.md')));
check('Marketplace preview exists', fs.existsSync(path.join(root, marketplaceManifest.preview)));
check('release publishes only the official Setup EXE and SHA-256', /MemorySentrySetup-\*\.exe/.test(read('.github/workflows/release.yml')) && /MemorySentrySetup-\*\.exe\.sha256/.test(read('.github/workflows/release.yml')) && !/MemorySentry-\*windows-x64\.zip/.test(read('.github/workflows/release.yml')) && /action-gh-release/i.test(read('.github/workflows/release.yml')));
check('build output has one supported installer and no custom portable ZIP', /MemorySentrySetup-/.test(read('scripts/build-package.ps1')) && !/Compress-Archive/.test(read('scripts/build-package.ps1')) && !/MemorySentry-.*windows-x64\.zip/.test(read('scripts/build-package.ps1')));

const app = read('app/index.js');
const helper = read('helper/helper.js');
const setupScript = read('installer/inno/MemorySentry.iss');
const buildScript = read('scripts/build-package.ps1');
const buildScriptSwap = read('scripts/swap-server.ps1');
const validation = read('.github/workflows/validate.yml');
const rustCargo = read('native/rust/Cargo.toml');
const rustLib = read('native/rust/src/lib.rs');
const rustMain = read('native/rust/src/main.rs');
const rustRuntime = read('native/rust/src/runtime.rs');
const rustPlatform = read('native/rust/src/platform/windows.rs');
check('dashboard supports automatic Portuguese and English', /navigator\.language/i.test(app) && /pt-BR/i.test(app) && /Auto language: English/i.test(app));
check('dashboard has a GitHub release button when helper is absent', /MSSOpenGitHub/i.test(app) && /github\.com\/yanlvl99\/spicetify-memory-sentry/i.test(app));
check('dashboard offers setup/help links without a Node.js button', /MSSOpenGitHub/.test(app) && /MSSOpenInstallGuide/.test(app) && !/MSSOpenNode|nodeButton|nodejs\.org/.test(app));
check('dashboard links to official releases and a generic installation guide', /MSS_RELEASES_URL/.test(app) && /MSS_INSTALL_GUIDE_URL/.test(app) && /installGuideButton/.test(app) && !/Marketplace setup guide|Guia de instalação do Marketplace/.test(app));
check('documentation describes one install path and Marketplace linking only', /only supported installation method/i.test(read('docs/INSTALLATION.md')) && /only links to the project repository/i.test(read('docs/INSTALLATION.md')) && !/Path B/.test(read('docs/INSTALLATION.md')));
check('dashboard offers per-process manual caps only in saved manual mode', /manualReady = !config\.autoOptimize/.test(app) && /setManualCap/.test(app) && /clearManualCap/.test(rustRuntime));
check('dashboard warns for 15 seconds before applying below-default limits', /Date\.now\(\) \+ 15000/.test(app) && /lowLimitWarning/.test(app) && /manualWarning\.seconds > 0/.test(app));
check('dashboard sends process creation identity with manual limit actions', /creation: process\.creation/.test(app) && rustRuntime.includes('\\"creation\\":{},'));
check('dashboard shows per-process CPU and active ceiling', /process\.cpuPercent/i.test(app) && /process\.limitMB/i.test(app));
check('dashboard explains local-only diagnostics', /No telemetry is uploaded/i.test(app));
check('Custom App replaces per-install token placeholder', app.includes('__MSS_TOKEN__'));
check('Custom App calls local status/config endpoints', app.includes('/api/status') && app.includes('/api/config'));
check('helper loads the testable adaptive policy module', helper.includes("require('./adaptive-policy')"));
check('helper uses native hard working-set API', helper.includes('SetProcessWorkingSetSizeEx') && helper.includes('GetProcessWorkingSetSizeEx'));
check('helper verifies process creation identity before changing limits', helper.includes('verifyProcessIdentity(opened.handle, target.creation)'));
check('helper persists original limits before applying them', helper.includes('persistBackups()') && helper.includes('makeBackupRecord'));
check('helper restores original limits when window becomes visible', helper.includes('restoreAllMeasured(`window ${state.windowState}`)'));
check('helper throttles repeated failed process-target attempts', helper.includes('TARGET_RETRY_DELAY_MS = 2500') && helper.includes('targetRetryAt.get(key)'));
check('helper refuses to apply new caps from a stale process snapshot', /if \(!state\.snapshotOk \|\| !state\.snapshotAtMs \|\| now - state\.snapshotAtMs > 6500\) return;/.test(helper));
check('helper excludes dedicated audio-service processes', helper.includes("if (p.role === 'Audio') return false") && !helper.includes('includeAudioProcess'));
check('helper polls focus at 100 ms', helper.includes('setInterval(tickSafe, 100)'));
check('helper discovers processes asynchronously on monitor path', helper.includes('refreshSnapshotAsync()') && helper.includes("execFile('powershell.exe'"));
check('helper keeps local API loopback-only and token-protected', helper.includes("server.listen(settings.port, '127.0.0.1'") && helper.includes('authorized(req)') && helper.includes('trustedOrigin(origin)'));
check('helper has read-only recovery and self-test commands', helper.includes('--restore-limits') && helper.includes('--self-test'));
check('helper does not use fixed manual cap configuration', !/settings\.workingSetLimitMB/.test(helper));
check('official Setup uses native preflight and recovery-aware commands', /MemorySentry\.exe/.test(setupScript) && /--check-install/.test(setupScript) && /--check-spicetify/.test(setupScript) && /--prepare-upgrade/.test(setupScript) && /--pre-uninstall/.test(setupScript));
check('Inno Setup packages the app and guards install/upgrade/uninstall', /PrivilegesRequired=lowest/.test(setupScript) && /--check-os/.test(setupScript) && /--prepare-upgrade/.test(setupScript) && /InitializeUninstall/.test(setupScript) && /--pre-uninstall/.test(setupScript));
check('build script compiles Inno setup and writes checksums', /ISCC\.exe/.test(buildScript) && /SHA256/.test(buildScript) && /\.sha256"/.test(buildScript) && /MemorySentrySetup-/.test(buildScript));
check('dashboard preserves detailed local API failures', app.includes('err && err.message') && app.includes("setError(tx('statusError') + ' [' + detail + ']')"));
check('dashboard renders actual connection diagnostics', app.includes("`${tx('helperMissingBody')} ${error}`"));
check('Rust uninstall allows Spotify to remain open and restores first', !/pub fn pre_uninstall\(\) -> Result<(), String> \{\s*ensure_spotify_closed/.test(rustRuntime) && rustRuntime.indexOf('pub fn pre_uninstall()') < rustRuntime.indexOf('unregister_startup()?;') && rustRuntime.indexOf('unregister_startup()?;') < rustRuntime.indexOf('unregister_spicetify()?;'));
check('single-instance mutex clears stale Win32 last-error state', rustPlatform.includes('SetLastError(0)'));
check('Rust launcher records early startup failures and verifies API readiness', rustRuntime.includes('startup.log') && rustRuntime.includes('request_status(settings.port, &settings.token)') && rustRuntime.includes('failed to become ready after 5 seconds'));
check('Rust configure failures are logged for the installer dialog', rustMain.includes('log_setup_failure(&error)') && rustMain.includes('setup.log') && setupScript.includes('SetupFailureDetails()'));
check('Inno Setup reads its ANSI diagnostic buffer with the required type', /LogText:\s*AnsiString/.test(setupScript) && /LoadStringFromFile\(LogFile, LogText\)/.test(setupScript));
check('installer shows a dedicated missing-Spicetify prerequisite message', rustMain.includes('"--check-spicetify"') && setupScript.includes("RunBundledCompanion('--check-spicetify', Code)") && setupScript.includes('SpicetifyMissing'));
check('single Setup flow does not require Spotify to be manually closed', !/ensure_spotify_closed/.test(rustRuntime) && !/Get-Process -Name Spotify/.test(rustRuntime) && /CloseApplications=no/.test(setupScript));
const configureBody = rustRuntime.slice(rustRuntime.indexOf('pub fn configure_install('), rustRuntime.indexOf('fn find_node()'));
check('local API starts and passes readiness before Spicetify apply', configureBody.indexOf('launch_monitor()') >= 0 && configureBody.indexOf('launch_monitor()') < configureBody.indexOf('apply_spicetify(&cli)'));
check('PID waits verify the actual Memory Sentry executable path', rustRuntime.includes('process_matches_executable(pid, &installed_exe)') && rustPlatform.includes('QueryFullProcessImageNameW'));
check('failed API bind removes helper.pid', rustRuntime.includes('if let Err(error) = start_api(Arc::clone(&shared))') && rustRuntime.includes('let _ = fs::remove_file(&pid_file);'));
const launcherBody = rustRuntime.slice(rustRuntime.indexOf('pub fn launch_monitor()'));
check('Rust launcher creates startup diagnostics before reading settings', launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') >= 0 && launcherBody.indexOf('append_startup_log(&startup_path, "Launcher invoked.")') < launcherBody.indexOf('load_settings(&settings_path)') && launcherBody.includes('Could not load companion settings'));
check('installer always launches the companion after setup', setupScript.includes('Flags: runhidden') && !setupScript.includes('Flags: postinstall nowait skipifsilent runhidden'));
check('redundant batch and PowerShell installer entry points are absent', ['Install.bat','Uninstall.bat','installer/Install.ps1','installer/Uninstall.ps1'].every(file => !fs.existsSync(path.join(root, file))));
check('official Setup is per-user and does not request elevation', /PrivilegesRequired=lowest/.test(setupScript) && !/RunAs|Verb\s+RunAs/i.test(setupScript));
check('Inno uninstaller invokes recovery-aware native pre-uninstall', /InitializeUninstall/.test(setupScript) && /RunInstalledCompanion\('--pre-uninstall', Code\)/.test(setupScript) && /Recovery data exists but the companion executable is missing/.test(setupScript));
check('source Inno installer preserves app recovery guard', /Recovery data exists but the companion executable is missing/.test(setupScript) && /No program files were removed/.test(setupScript));
check('package uses pinned Koffi and correct version', packageJson.version === '0.2.0' && packageJson.dependencies?.koffi === '3.3.3' && packageJson.engines?.node === '>=22');
check('CI runs package validation', validation.includes('node tests/validate-package.js'));
check('release versions are aligned on 1.0.0-rc.1', rustCargo.includes('version = "1.0.0-rc.1"') && read('native/rust/Cargo.lock').includes('version = "1.0.0-rc.1"') && rustRuntime.includes('const VERSION: &str = "1.0.0-rc.1";') && setupScript.includes('#define AppVersion "1.0.0-rc.1"') && read('README.md').includes('1.0.0-rc.1'));
check('semantic prereleases are never marked as latest stable', read('.github/workflows/release.yml').includes("contains(steps.version.outputs.version, '-')") && read('.github/workflows/release.yml').includes("!contains(steps.version.outputs.version, '-')"));
check('Rust scaffold uses a minimal standard-library dependency policy', rustCargo.includes('name = "memory-sentry-native"') && !rustCargo.includes('[dependencies]'));
check('Rust policy has pure, testable adaptive logic', rustLib.includes('decide_adjustment') && rustLib.includes('#[cfg(test)]'));
check('Rust binary exposes OS, install-preflight, upgrade, configure, monitor, and uninstall commands', ['--check-os', '--check-install', '--check-spicetify', '--prepare-upgrade', '--configure', '--monitor', '--pre-uninstall', '--restore-limits'].every(command => rustMain.includes(command)));
check('server can be swapped in place while Spotify keeps running', rustMain.includes('"--swap"') && rustRuntime.includes('pub fn swap_server') && buildScriptSwap.includes('--swap'));
check('dashboard offers ceiling-only and maximum release modes', /trimToCap/.test(app) && /releaseCeiling/.test(app) && /trimToCap/.test(rustRuntime) && /trim_to_cap/.test(rustRuntime));
check('dashboard keeps diagnostics behind a debug mode toggle', /'mss\.debugMode'/.test(app) && /debugMode:/.test(app) && /debug \? card\(tx\('pageFaults'\)/.test(app) && /debug \? h\('section', \{ className: 'mss-section' \}, h\('div', \{ className: 'mss-section-head' \}, h\('h2', \{ className: 'mss-h2' \}, tx\('events'\)\)/.test(app));
check('dashboard debug switch uses the Spicetify Toggle component with a fallback', /ReactComponent\.Toggle/.test(app) && /MSSDebugToggle/.test(app) && /typeof Toggle !== 'function'/.test(app));
check('server discovers audio sessions through the Core Audio session API', rustPlatform.includes('pub fn audio_session_pids') && rustPlatform.includes('CoCreateInstance') && rustPlatform.includes('IID_IAUDIO_SESSION_CONTROL2') && rustRuntime.includes('refresh_audio_session_pids') && rustRuntime.includes('audio_pids'));
check('status payload reports the server as Server, not Rust', /"node\\":\\"Server\\",\\"runtime\\":\\"Server\\"/.test(rustRuntime));
check('Rust runtime implements local API, automatic limits and manual per-process limits', rustRuntime.includes('/api/status') && rustRuntime.includes('/api/config') && rustRuntime.includes('/api/action') && rustRuntime.includes('platform::set_working_set_limits') && rustRuntime.includes('restore_all') && rustRuntime.includes('apply_manual_cap') && rustRuntime.includes('manualCapMB'));
check('Rust manual defaults set Main and Audio to 36 MB without changing automatic audio protection', rustLib.includes('Self::Main | Self::Audio => 36') && rustRuntime.includes('if role == ProcessRole::Audio') && rustLib.includes('audio_processes_never_receive_an_automatic_ceiling'));
check('manual caps persist process creation identity and are recorded in recovery backups', rustRuntime.includes('manualCaps') && rustRuntime.includes('manual: fields') && rustRuntime.includes('if r.manual { \"manual\" }'));
check('policy transitions restore active limits before switching modes', rustRuntime.includes('next.auto_optimize != state.settings.auto_optimize') && rustRuntime.includes('policy mode changed') && rustRuntime.includes('failed > 0'));
check('Rust native layer protects process identity before each working-set mutation', rustPlatform.includes('set_working_set_limits') && rustPlatform.includes('creation != expected_creation') && rustPlatform.includes('restore_working_set_limits'));
check('Rust startup and instance locking are per-user and single-instance', rustPlatform.includes('acquire_single_instance') && rustRuntime.includes('register_startup') && rustRuntime.includes('127.0.0.1'));
check('Rust migration stops the legacy monitor before invoking its one-shot restore command', (rustRuntime.match(/stop_legacy_node_helper\(&base\)\?;/g) || []).length === 2);
check('Rust dashboard port remains consistent with the Custom App endpoint', /const MSS_API = 'http:\/\/127\.0\.0\.1:38472'/.test(app) && /port: DEFAULT_PORT/.test(rustRuntime));

// Exercise the pure adaptive policy without Windows APIs.
check('main process starts with its role baseline', policy.baseCapMB('Main') === 36);
check('CPU estimate is normalized to one logical processor', policy.estimateCpuPercent(1000, 1250, 1000) === 25);
check('CPU estimate rejects invalid sample intervals', policy.estimateCpuPercent(1000, 1250, 0) === null);
check('sustained high CPU increases the ceiling by 8 MB', policy.decideAdjustment({ role: 'Renderer', currentCapMB: 28, workingSetMB: 24, cpuPercent: 18, highCpuMs: 7000, lowCpuMs: 0, lastAdjustmentAt: 0, now: 20000 })?.nextCapMB === 36);
check('high CPU without enough duration does not increase ceiling', policy.decideAdjustment({ role: 'Renderer', currentCapMB: 28, workingSetMB: 24, cpuPercent: 18, highCpuMs: 3000, now: 20000 }) === null);
check('low CPU and headroom lowers the ceiling by 4 MB', policy.decideAdjustment({ role: 'Renderer', currentCapMB: 28, workingSetMB: 15, cpuPercent: 1, lowCpuMs: 12000, lastAdjustmentAt: 0, now: 20000 })?.nextCapMB === 24);
check('main process keeps a 24 MB floor to improve restore responsiveness', policy.minimumCapMB('Main') === 24 && policy.decideAdjustment({ role: 'Main', currentCapMB: 28, workingSetMB: 15, cpuPercent: 1, lowCpuMs: 12000, lastAdjustmentAt: 0, now: 20000 })?.nextCapMB === 24);
check('renderers can still shrink toward the 12 MB floor', policy.minimumCapMB('Renderer') === 12 && policy.decideAdjustment({ role: 'Renderer', currentCapMB: 16, workingSetMB: 7, cpuPercent: 1, lowCpuMs: 12000, lastAdjustmentAt: 0, now: 20000 })?.nextCapMB === 12);
check('low CPU near the ceiling does not reduce the limit', policy.decideAdjustment({ role: 'Renderer', currentCapMB: 28, workingSetMB: 25, cpuPercent: 1, lowCpuMs: 12000, lastAdjustmentAt: 0, now: 20000 }) === null);
check('adjustment cooldown prevents rapid oscillation', policy.decideAdjustment({ role: 'Renderer', currentCapMB: 28, workingSetMB: 15, cpuPercent: 1, lowCpuMs: 12000, lastAdjustmentAt: 17000, now: 20000 }) === null);
check('adaptive floor is 12 MB and global ceiling is 64 MB', policy.MIN_CAP_MB === 12 && policy.MAX_CAP_MB === 64);
check('per-role upper bounds remain below the global safety limit', policy.maximumCapMB('Main') === 60 && policy.maximumCapMB('Renderer') === 52);
check('a process at its role maximum cannot grow further', policy.decideAdjustment({ role: 'Main', currentCapMB: 60, workingSetMB: 50, cpuPercent: 90, highCpuMs: 12000, lastAdjustmentAt: 0, now: 20000 }) === null);
check('CPU estimate is capped at one logical processor', policy.estimateCpuPercent(0, 4000, 1000) === 100);

const warningSecondsSource = app.match(/function MSSManualWarningSeconds\(readyAt, now\) \{[\s\S]*?\n\}/)?.[0];
if (warningSecondsSource) {
  check('below-default warning cannot finish before 15 seconds',
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(15000, 0)`) === 15 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(15000, 14999)`) === 1 &&
    vm.runInNewContext(`${warningSecondsSource}; MSSManualWarningSeconds(15000, 15000)`) === 0);
} else check('below-default warning countdown function exists', false);

for (const [name, source] of [['app/index.js', app], ['helper/helper.js', helper], ['helper/adaptive-policy.js', read('helper/adaptive-policy.js')]]) {
  try { new vm.Script(source, { filename: name }); check(`JavaScript syntax: ${name}`, true); }
  catch (error) { check(`JavaScript syntax: ${name} (${error.message})`, false); }
}
if (failures) process.exitCode = 1;
else console.log('\nAll static checks and pure policy tests passed. Native Win32 behavior still requires a Windows runtime test.');
