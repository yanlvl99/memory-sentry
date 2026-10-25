// Memory Sentry Custom App. Source and default locale are English; Portuguese is selected automatically.
// The installer replaces __MSS_TOKEN__ with an internal per-install access token.
const MSSReact = Spicetify.React;
const h = MSSReact.createElement;
const MSS_API = 'http://127.0.0.1:38472';
const MSS_TOKEN = '__MSS_TOKEN__';
const MSS_GITHUB_URL = 'https://github.com/yanlvl99/spicetify-memory-sentry';
const MSS_RELEASES_URL = `${MSS_GITHUB_URL}/releases`;
const MSS_INSTALL_GUIDE_URL = `${MSS_GITHUB_URL}/blob/main/docs/INSTALLATION.md`;
const MSS_LOCALE = (typeof navigator !== 'undefined' && /^pt(?:-|$)/i.test(navigator.language || '')) ? 'pt-BR' : 'en-US';
const MSS_PT = MSS_LOCALE === 'pt-BR';

const MSS_TEXT = {
  en: {
    kicker: 'SPICETIFY · WINDOWS MEMORY TOOL · RELEASE CANDIDATE v1.0.0-rc.1', title: 'Memory Sentry',
    intro: 'Memory management for Spotify Desktop. Choose an automatic policy that applies limits when minimized, hidden, or in the background, or turn it off to set a working-set limit for each Spotify process. Original Windows limits are recorded before changes.',
    connected: 'Server connected', unavailable: 'Connection unavailable', waiting: 'Waiting for server',
    pluginEnabled: 'Memory Sentry on', pluginDisabled: 'Memory Sentry off', pluginToggleHint: 'Turn memory management on or off; disabling restores original process limits.', pluginDisabledNote: 'Memory Sentry is off. Original limits are restored and no automatic or manual caps will be applied.', pluginActivatedFlash: 'MEMORY SENTRY ACTIVATED', pluginDisabledFlash: 'MEMORY SENTRY DEACTIVATED',
    helperMissingTitle: 'Memory Sentry server is not connected',
    helperMissingBody: 'The Marketplace listing only links to this project and cannot install Memory Sentry. Use the official Setup from GitHub Releases to install or repair the Memory Sentry server.',
    helperButton: 'Download official setup', installGuideButton: 'Installation guide', safety: 'On supported Windows 10/11 x64 PCs, automatic optimization is intended to reduce Spotify resident RAM while minimized, hidden, or in the background; actual savings vary by PC and workload. Main and Audio processes have a 36 MB automatic ceiling; Audio is fixed while Main may adapt only below that ceiling. A manual limit below the process default can cause failures. A working-set ceiling does not necessarily reduce resident memory immediately; this changes resident working-set memory, not committed memory. Very low-end PCs may briefly show a black window on restore; that alone does not prove a reload if the previous UI state returns. Input may take a few seconds to respond.',
    spotifyRam: 'Spotify total working-set memory', privateWorkingSetRam: 'Spotify private working-set memory', ramNote: '{count} Spotify processes detected. Total working set includes shared pages and can exceed Task Manager memory.', privateWorkingSetNote: '{count} Spotify processes detected. Private working set excludes shared pages, matching Task Manager memory.',
    optimizedRam: 'Managed-process memory', managedNote: '{count} Spotify processes are eligible for automatic ceilings; Main is capped at 36 MB and Audio stays fixed at 36 MB.', manualManagedNote: '{count} Spotify processes are available for manual limits.',
    activeLimits: 'Managed limits active', activeNote: 'Per-process working-set limits currently applied.', manualLimitsCard: 'Manual limits active', manualActiveNote: 'Role defaults or your saved overrides apply only while Spotify is minimized, hidden, or in the background.',
    pageFaults: 'Page faults since last sample', pageFaultNote: 'Includes soft and hard faults; not equivalent to disk reads.',
    pageFaultRate: 'Page faults per second', pageFaultRateNote: 'A local diagnostic signal, not proof of disk I/O by itself.',
    restoreDuration: 'Limit restoration duration', restoreNote: 'Measures the server operation, not full UI recovery time.',
    adaptiveChanges: 'Adaptive adjustments', adaptiveNote: 'Changes made by the automatic per-process policy.',
    processes: 'Monitored processes', updated: 'Updated {time}', cpu: 'CPU {value}%', privateMemory: 'Private commit {value} MB',
    audioProtected: 'Audio fixed at 36 MB · not adaptive', adaptiveAt: 'Adaptive ceiling: {value} MB',
    willOptimize: 'Ready for automatic optimization', notIncluded: 'Outside the current process tree',
    reasonHigh: 'raised after sustained high CPU at the working-set ceiling', reasonLow: 'lowered because CPU and memory usage were low', reasonBaseline: 'using role baseline', reasonHolding: 'holding current ceiling',
    noneDetected: 'No eligible processes detected. Open Spotify and wait for process discovery. Memory Sentry will not guess at unknown process identities.',
    lastAdjustment: 'Last adjustment: {time}',
    lastAction: 'Last automatic action: adjusted {count} process(es). Audio processes use a fixed 36 MB ceiling.',
    behavior: 'Memory policy', active: 'Automatic policy enabled', autoExplanation: 'When enabled, limits apply after Spotify is minimized, hidden, or moved behind another window. Main never exceeds 36 MB; Audio stays fixed at 36 MB. Turn this off to use manual mode and set a limit per process.',
    manualMode: 'Manual mode', manualExplanation: 'Role defaults are applied automatically while Spotify is minimized, hidden, or in the background: Main/Audio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB, Crashpad 16 MB. Your per-process values override these defaults. Original limits are restored in the foreground.',
    delay: 'Delay after leaving foreground', delayExplanation: 'Used only by automatic policy; it avoids optimizing during brief background changes.',
    manualLimit: 'Manual limit (MB)', manualCurrent: 'Default for background: {value} MB', manualActive: 'Manual limit active: {value} MB', manualConfigured: 'Saved for background: {value} MB',
    lowLimitWarning: 'The requested {capMB} MB limit is below this process default ({defaultMB} MB) and may cause slowdowns, glitches, or failures. Review the risk before continuing. You can proceed in {seconds} seconds.',
    lowLimitTitle: 'Low limit warning · {role} PID {pid}', waitBeforeApply: 'Wait {seconds}s…', confirmLowLimit: 'Apply lower limit', cancel: 'Cancel', limitRange: 'Enter a whole number from 8 to 512 MB.', limitApplied: 'Manual limit applied for the background state.', limitSavedForBackground: 'Manual limit saved; it will apply when Spotify leaves the foreground.', limitCleared: 'Custom override removed; the role default will apply in the background.', saveBeforeManual: 'Manual controls activate after the automatic policy is saved as off.',
    releaseMode: 'Memory release', releaseCeiling: 'Release only down to the ceiling', releaseAggressive: 'Release as much as possible',
    releaseModeExplanation: 'Releasing only down to the ceiling frees the excess above the current limit and keeps restore smooth; releasing as much as possible frees more RAM right away but faults more pages back later.',
    debugMode: 'Debug mode', debugHint: 'Shows diagnostics: telemetry cards, activity log and per-process details.',
    seconds: '{value} seconds', saving: 'Saving…', autoSaved: 'Changes are saved automatically on this PC.',
    events: 'Recent activity', noEvents: 'No activity recorded yet.', statusError: 'Could not connect to the local Memory Sentry server. The diagnostic appears in brackets; check startup.log in %LOCALAPPDATA%\\SpicetifyMemorySentry\\logs.',
    footer: 'Supported OS: Windows 10 and Windows 11 desktop editions only. The Memory Sentry server is open source under GPL-3.0. No telemetry is uploaded. CPU and memory diagnostics stay on this PC. A separate process is required to call Win32 working-set APIs.',
    saveFailed: 'Could not save settings.', helperApiError: 'The local server returned an error.',
    window: { foreground: 'Spotify in foreground', 'background-visible': 'Spotify visible in background', minimized: 'Spotify minimized', hidden: 'Spotify window hidden', 'not-running': 'Spotify is not running', 'window-not-found': 'Spotify found; window not located', 'window-check-error': 'Window state unavailable', 'not-detected': 'Waiting for detection' }
  },
  pt: {
    kicker: 'SPICETIFY · FERRAMENTA DE RAM PARA WINDOWS · RELEASE CANDIDATE v1.0.0-rc.1', title: 'Memory Sentry',
    intro: 'Gerenciamento de memória para o Spotify Desktop. Escolha uma política automática que aplica limites quando minimizado, oculto ou em segundo plano, ou desative-a para definir um limite de working set por processo. Os limites originais do Windows são registrados antes de qualquer alteração.',
    connected: 'Servidor conectado', unavailable: 'Conexão indisponível', waiting: 'Aguardando servidor',
    pluginEnabled: 'Memory Sentry ativado', pluginDisabled: 'Memory Sentry desativado', pluginToggleHint: 'Ativa ou desativa o gerenciamento de memória; ao desativar, restaura os limites originais.', pluginDisabledNote: 'O Memory Sentry está desativado. Os limites originais foram restaurados e nenhum limite será aplicado.', pluginActivatedFlash: 'MEMORY SENTRY ATIVADO', pluginDisabledFlash: 'MEMORY SENTRY DESATIVADO',
    helperMissingTitle: 'O servidor do Memory Sentry não está conectado',
    helperMissingBody: 'A listagem do Marketplace apenas leva ao projeto e não instala o Memory Sentry. Use o instalador oficial em GitHub Releases para instalar ou reparar o servidor do Memory Sentry.',
    helperButton: 'Baixar instalador oficial', installGuideButton: 'Guia de instalação', safety: 'Em PCs compatíveis com Windows 10/11 x64, a otimização automática busca reduzir a RAM residente do Spotify enquanto minimizado, oculto ou em segundo plano; a economia varia conforme o PC e a carga. Main e Áudio têm teto automático de 36 MB; o Áudio é fixo e o Main só pode se adaptar abaixo desse teto. Um limite manual abaixo do padrão do processo pode causar falhas. Um teto de working set não necessariamente reduz a memória residente imediatamente e não altera a memória comprometida. Em PCs muito fracos, a janela pode ficar preta por alguns instantes ao restaurar; isso não prova uma recarga se a interface volta ao estado anterior. A entrada pode demorar alguns segundos para responder.',
    spotifyRam: 'Memória total do working set do Spotify', privateWorkingSetRam: 'Memória privada do working set do Spotify', ramNote: '{count} processos do Spotify detectados. O working set total inclui páginas compartilhadas e pode superar o valor do Gerenciador de Tarefas.', privateWorkingSetNote: '{count} processos do Spotify detectados. O working set privado exclui páginas compartilhadas, como no Gerenciador de Tarefas.',
    optimizedRam: 'Memória dos processos gerenciados', managedNote: '{count} processos do Spotify estão elegíveis para limites automáticos; Main tem teto de 36 MB e o Áudio fica fixo em 36 MB.', manualManagedNote: '{count} processos do Spotify estão disponíveis para limites manuais.',
    activeLimits: 'Limites gerenciados ativos', activeNote: 'Limites de working set por processo atualmente aplicados.', manualLimitsCard: 'Limites manuais ativos', manualActiveNote: 'Os padrões da função ou seus limites salvos só se aplicam quando o Spotify está minimizado, oculto ou em segundo plano.',
    pageFaults: 'Falhas de página desde a amostra', pageFaultNote: 'Inclui falhas suaves e duras; não equivale a leituras de disco.',
    pageFaultRate: 'Falhas de página por segundo', pageFaultRateNote: 'Sinal de diagnóstico local; sozinho não comprova acesso ao disco.',
    restoreDuration: 'Duração da restauração dos limites', restoreNote: 'Mede a operação do servidor, não a recuperação completa da interface.',
    adaptiveChanges: 'Ajustes adaptativos', adaptiveNote: 'Mudanças feitas pela política automática por processo.',
    processes: 'Processos monitorados', updated: 'Atualizado {time}', cpu: 'CPU {value}%', privateMemory: 'Commit privado {value} MB',
    audioProtected: 'Áudio fixo em 36 MB · sem ajuste adaptativo', adaptiveAt: 'Limite adaptativo: {value} MB',
    willOptimize: 'Pronto para otimização automática', notIncluded: 'Fora da árvore de processos atual',
    reasonHigh: 'elevado após CPU alta sustentada com o working set no teto', reasonLow: 'reduzido porque CPU e memória estavam baixas', reasonBaseline: 'usando limite inicial da função', reasonHolding: 'mantendo o limite atual',
    noneDetected: 'Nenhum processo elegível detectado. Abra o Spotify e aguarde a descoberta dos processos. O Memory Sentry não tenta adivinhar identidades de processos desconhecidos.',
    lastAdjustment: 'Último ajuste: {time}',
    lastAction: 'Última ação automática: ajustou {count} processo(s). O áudio usa um teto fixo de 36 MB.',
    behavior: 'Política de memória', active: 'Política automática ativada', autoExplanation: 'Quando ativada, os limites são aplicados após minimizar, ocultar ou colocar o Spotify em segundo plano. Main não passa de 36 MB; o Áudio fica fixo em 36 MB. Desative para usar o modo manual e definir um limite por processo.',
    manualMode: 'Modo manual', manualExplanation: 'Os padrões por função são aplicados automaticamente quando o Spotify está minimizado, oculto ou em segundo plano: Main/Áudio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB e Crashpad 16 MB. Seus valores individuais substituem esses padrões. Os limites originais são restaurados quando o Spotify volta ao primeiro plano.',
    delay: 'Atraso após sair do primeiro plano', delayExplanation: 'Usado somente pela política automática; evita otimizar em mudanças rápidas para segundo plano.',
    manualLimit: 'Limite manual (MB)', manualCurrent: 'Padrão em segundo plano: {value} MB', manualActive: 'Limite manual ativo: {value} MB', manualConfigured: 'Salvo para segundo plano: {value} MB',
    lowLimitWarning: 'O limite solicitado de {capMB} MB é menor que o padrão deste processo ({defaultMB} MB) e pode causar lentidão, falhas visuais ou erros. Revise o risco antes de continuar. Você poderá prosseguir em {seconds} segundos.',
    lowLimitTitle: 'Aviso de limite baixo · {role} PID {pid}', waitBeforeApply: 'Aguarde {seconds}s…', confirmLowLimit: 'Aplicar limite menor', cancel: 'Cancelar', limitRange: 'Digite um número inteiro entre 8 e 512 MB.', limitApplied: 'Limite manual aplicado para o estado em segundo plano.', limitSavedForBackground: 'Limite manual salvo; será aplicado quando o Spotify sair do primeiro plano.', limitCleared: 'Substituição removida; o padrão da função será aplicado em segundo plano.', saveBeforeManual: 'Os controles manuais serão liberados após salvar a política automática como desativada.',
    releaseMode: 'Liberação de memória', releaseCeiling: 'Libera somente até o teto', releaseAggressive: 'Libera o máximo possível',
    releaseModeExplanation: 'Liberar somente até o teto remove o excesso acima do limite atual e mantém a restauração fluida; liberar o máximo possível recupera mais RAM na hora, mas volta a falhar mais páginas depois.',
    debugMode: 'Modo debug', debugHint: 'Mostra diagnósticos: cartões de telemetria, log de atividade e detalhes por processo.',
    seconds: '{value} segundos', saving: 'Salvando…', autoSaved: 'As alterações são salvas automaticamente neste PC.',
    events: 'Atividade recente', noEvents: 'Ainda não há atividade registrada.', statusError: 'Não foi possível conectar ao servidor local do Memory Sentry. O diagnóstico aparece entre colchetes; consulte startup.log em %LOCALAPPDATA%\\SpicetifyMemorySentry\\logs.',
    footer: 'Compatível apenas com Windows 10 e Windows 11 (desktop). O servidor do Memory Sentry é código aberto sob GPL-3.0. Nenhuma telemetria é enviada. Os diagnósticos de CPU e memória ficam neste PC. Um processo separado é necessário para chamar as APIs Win32 de working set.',
    saveFailed: 'Não foi possível salvar as configurações.', helperApiError: 'O servidor local retornou um erro.',
    window: { foreground: 'Spotify em primeiro plano', 'background-visible': 'Spotify visível em segundo plano', minimized: 'Spotify minimizado', hidden: 'Janela do Spotify oculta', 'not-running': 'Spotify não está aberto', 'window-not-found': 'Spotify detectado; janela não localizada', 'window-check-error': 'Estado da janela indisponível', 'not-detected': 'Aguardando detecção' }
  }
};
const T = MSS_PT ? MSS_TEXT.pt : MSS_TEXT.en;
function tx(key, vars = {}) {
  let value = T[key] || MSS_TEXT.en[key] || key;
  for (const [name, replace] of Object.entries(vars)) value = value.replaceAll(`{${name}}`, String(replace));
  return value;
}
function MSSLocalizeEvent(message) {
  if (!MSS_PT || typeof message !== 'string') return message;
  const mappings = [
    [/^Não foi possível persistir o backup dos limites originais:/, 'Não foi possível salvar o arquivo local de recuperação:'],
    [/^Existem (\d+) backups de limites; a detecção falhou, então não vou descartá-los\.$/, 'Há $1 registros de recuperação, mas a detecção falhou; os registros foram preservados.'],
    [/^Recuperação de backups: (\d+) restaurados, (\d+) processos encerrados, (\d+) pendentes\.$/, 'Recuperação: $1 restaurados, $2 processos encerrados, $3 pendentes.'],
    [/^Spotify ficou (.*); o teto rígido por processo será ativado após (\d+)s\.$/, 'O Spotify ficou $1; os limites adaptativos serão ativados após $2 s.'],
    [/^Erro inesperado no monitor:/, 'Erro inesperado no monitor:'],
    [/^Settings updated:/, 'Configurações atualizadas:'],
    [/^Adaptive working-set cap applied to/, 'Limite adaptativo de working set aplicado a'],
    [/^Restored original limits for/, 'Limites originais restaurados para'],
    [/^Adaptive optimization started for/, 'Otimização adaptativa iniciada para'],
    [/^Adaptive policy raised/, 'Política adaptativa elevou'],
    [/^Adaptive policy lowered/, 'Política adaptativa reduziu'],
    [/^Adaptive cap update failed for/, 'Falha ao atualizar o limite adaptativo de'],
    [/^Adaptive cap update skipped for/, 'Atualização do limite adaptativo ignorada para'],
    [/^Could not query Spotify processes:/, 'Não foi possível consultar os processos do Spotify:'],
    [/^Could not inspect the Spotify window:/, 'Não foi possível verificar a janela do Spotify:'],
    [/^Adaptive working-set cap applied to (.*) PID (\d+): (\d+) MB; (.*)$/, 'Limite adaptativo aplicado a $1 PID $2: $3 MB; $4'],
    [/^Adaptive optimization started for (\d+) process\(es\); dedicated audio processes remain excluded\.$/, 'Otimização adaptativa iniciada para $1 processo(s); serviços de áudio continuam excluídos.'],
    [/^Adaptive policy raised (.*) PID (\d+) to (\d+) MB (.*)$/, 'A política adaptativa elevou $1 PID $2 para $3 MB $4'],
    [/^Adaptive policy lowered (.*) PID (\d+) to (\d+) MB (.*)$/, 'A política adaptativa reduziu $1 PID $2 para $3 MB $4'],
    [/^Restored original limits for (.*) PID (\d+): (.*)$/, 'Limites originais restaurados para $1 PID $2: $3'],
    [/^Discarded recovery record for PID (\d+): (.*)$/, 'Registro de recuperação descartado para PID $1: $2'],
    [/^Settings updated: (.*)$/, 'Configurações atualizadas: $1'],
    [/^Local API could not start on port (\d+): (.*)$/, 'A API local não iniciou na porta $1: $2']
  ];
  for (const [pattern, replacement] of mappings) if (pattern.test(message)) return message.replace(pattern, replacement);
  return message;
}
function MSSEnsureStyles() {
  if (document.getElementById('mss-styles')) return;
  const style = document.createElement('style');
  style.id = 'mss-styles';
  style.textContent = `
    .mss-page{max-width:1200px;margin:0 auto;padding:28px 24px 48px;color:var(--spice-text,#f5f5f5);font-family:var(--font-family,inherit)}
    .mss-hero{display:flex;gap:20px;align-items:flex-start;justify-content:space-between;margin-bottom:22px}.mss-kicker{font-size:11px;font-weight:800;letter-spacing:.14em;text-transform:uppercase;color:var(--spice-subtext,#a7a7a7);margin-bottom:8px}.mss-title{font-size:clamp(30px,4vw,42px);line-height:1.08;letter-spacing:-.04em;font-weight:800;margin:0 0 10px}.mss-copy{color:var(--spice-subtext,#a7a7a7);font-size:14px;max-width:690px;line-height:1.6}
    .mss-pill{display:inline-flex;align-items:center;gap:8px;border:1px solid var(--spice-button,#1ed760);border-radius:999px;padding:8px 11px;font-size:12px;font-weight:700;white-space:nowrap}.mss-dot{width:7px;height:7px;border-radius:50%;background:var(--spice-button,#1ed760);box-shadow:0 0 0 4px color-mix(in srgb,var(--spice-button,#1ed760) 17%,transparent)}
    .mss-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin:16px 0}.mss-card,.mss-section{background:var(--spice-card,#282828);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);border-radius:15px;padding:18px;min-width:0}.mss-card-label{font-size:12px;color:var(--spice-subtext,#a7a7a7);font-weight:700;margin-bottom:12px}.mss-number{font-size:31px;line-height:1;font-weight:800;letter-spacing:-.04em;font-variant-numeric:tabular-nums}.mss-unit{font-size:13px;color:var(--spice-subtext,#a7a7a7);font-weight:600;margin-left:5px}.mss-note{font-size:12px;color:var(--spice-subtext,#a7a7a7);line-height:1.48;margin-top:10px}
    .mss-section{margin-top:15px}.mss-section-head{display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:12px}.mss-h2{font-size:17px;font-weight:800;letter-spacing:-.02em;margin:0}.mss-small{font-size:12px;color:var(--spice-subtext,#a7a7a7);line-height:1.5}.mss-list{display:flex;flex-direction:column;gap:2px}.mss-process{display:grid;grid-template-columns:minmax(0,1fr) auto auto;gap:13px;align-items:center;padding:11px 0;border-bottom:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent)}.mss-process:last-child{border-bottom:0}.mss-process-name{font-size:13px;font-weight:700}.mss-process-meta{font-size:11px;color:var(--spice-subtext,#a7a7a7);margin-top:4px;line-height:1.4}.mss-memory{font-size:13px;font-variant-numeric:tabular-nums;font-weight:800;white-space:nowrap}.mss-tag{font-size:10px;text-transform:uppercase;letter-spacing:.06em;border-radius:6px;background:color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);padding:5px 7px;color:var(--spice-subtext,#a7a7a7);white-space:nowrap}
    .mss-settings{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}.mss-setting{display:flex;flex-direction:column;gap:8px;min-width:0}.mss-setting-label{font-weight:700;font-size:13px}.mss-select{background:var(--spice-main,#121212);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 18%,transparent);color:var(--spice-text,#fff);border-radius:9px;padding:10px 11px;font-size:13px;width:100%}.mss-check{display:flex;align-items:flex-start;gap:10px;font-size:13px;line-height:1.45;cursor:pointer}.mss-check input{accent-color:var(--spice-button,#1ed760);margin-top:3px}.mss-actions{display:flex;align-items:center;gap:9px;flex-wrap:wrap;margin-top:16px}.mss-btn{border:0;border-radius:999px;padding:10px 15px;font-size:12px;font-weight:800;cursor:pointer;color:var(--spice-text,#fff);background:color-mix(in srgb,var(--spice-text,#fff) 10%,transparent)}.mss-btn:hover{filter:brightness(1.12)}.mss-btn:disabled{opacity:.45;cursor:not-allowed}.mss-btn-primary{background:var(--spice-button,#1ed760);color:var(--spice-button-text,#061006)}.mss-btn-danger{background:#d32f2f;color:#fff;border:1px solid #ff7777}.mss-btn-secondary{background:transparent;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent)}.mss-notice{border-radius:10px;padding:12px 14px;font-size:12px;line-height:1.5;margin:12px 0;background:color-mix(in srgb,var(--spice-text,#fff) 6%,transparent);color:var(--spice-subtext,#a7a7a7)}.mss-notice-warn{background:color-mix(in srgb,#e7a83e 12%,transparent);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,#e7a83e 30%,transparent)}.mss-modal .mss-notice-warn{background:color-mix(in srgb,#d32f2f 30%,var(--spice-card,#282828));color:#fff;border:2px solid #ff5252;font-weight:700}.mss-notice-error{background:color-mix(in srgb,#e05252 12%,transparent);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,#e05252 30%,transparent)}.mss-events{display:flex;flex-direction:column;gap:0}.mss-event{display:grid;grid-template-columns:90px minmax(0,1fr);gap:12px;padding:8px 0;border-bottom:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);font-size:12px;line-height:1.45}.mss-event-time{color:var(--spice-subtext,#a7a7a7);font-variant-numeric:tabular-nums}.mss-event-msg{overflow-wrap:anywhere}.mss-empty{padding:16px 0;color:var(--spice-subtext,#a7a7a7);font-size:13px}.mss-footer{margin-top:20px;color:var(--spice-subtext,#a7a7a7);font-size:11px;line-height:1.55}.mss-manual-controls{grid-column:1/-1;display:flex;align-items:center;gap:8px;flex-wrap:wrap}.mss-input{width:96px;background:var(--spice-main,#121212);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent);color:var(--spice-text,#fff);border-radius:8px;padding:8px;font-size:13px}.mss-modal-backdrop{position:fixed;inset:0;z-index:99999;background:#000b;display:grid;place-items:center;padding:20px}.mss-modal{max-width:500px;width:100%;background:var(--spice-card,#282828);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent);border-radius:15px;padding:20px;box-shadow:0 18px 60px #0008}
    .mss-page{animation:mss-page-enter .46s ease-out both}.mss-hero{padding:24px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 10%,transparent);border-radius:18px;background:linear-gradient(135deg,color-mix(in srgb,var(--spice-card,#282828) 96%,var(--spice-button,#1ed760)),var(--spice-main,#121212))}.mss-kicker{display:inline-block;border-radius:999px;padding:5px 8px;background:color-mix(in srgb,var(--spice-button,#1ed760) 12%,transparent)}.mss-card{background:linear-gradient(155deg,var(--spice-card,#282828),color-mix(in srgb,var(--spice-card,#282828) 85%,var(--spice-main,#121212)));transition:transform .18s ease}.mss-card:hover{transform:translateY(-3px)}.mss-grid>.mss-card{animation:mss-card-enter .42s ease-out both}.mss-grid>.mss-card:nth-child(2){animation-delay:60ms}.mss-grid>.mss-card:nth-child(3){animation-delay:120ms}.mss-grid>.mss-card:nth-child(n+4){animation-delay:180ms}.mss-section{background:linear-gradient(145deg,var(--spice-card,#282828),color-mix(in srgb,var(--spice-card,#282828) 92%,var(--spice-main,#121212)))}.mss-process{transition:transform .16s ease}.mss-process:hover{transform:translateX(3px)}.mss-btn{transition:transform .15s ease,filter .15s ease}.mss-btn:hover{transform:translateY(-1px)}.mss-power{position:relative;display:inline-flex;align-items:center;gap:9px;padding:8px 12px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 18%,transparent);border-radius:999px;background:color-mix(in srgb,var(--spice-card,#282828) 86%,transparent);font-size:12px;font-weight:800;cursor:pointer;white-space:nowrap}.mss-power-on{border-color:color-mix(in srgb,var(--spice-button,#1ed760) 55%,transparent)}.mss-power-off{border-color:color-mix(in srgb,#e05252 65%,transparent)}.mss-power input{position:absolute;width:1px;height:1px;opacity:0}.mss-power-dot{width:9px;height:9px;flex:0 0 9px;border-radius:50%;background:#e05252}.mss-power-on .mss-power-dot{background:var(--spice-button,#1ed760);box-shadow:0 0 0 4px color-mix(in srgb,var(--spice-button,#1ed760) 17%,transparent)}.mss-power input:focus-visible+.mss-power-dot{outline:2px solid var(--spice-text,#fff);outline-offset:4px}.mss-screen-flash{position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;pointer-events:none;color:#fff;animation:mss-screen-transition .72s cubic-bezier(.2,.7,.2,1) both}.mss-screen-flash::before{content:"";position:absolute;inset:0}.mss-screen-flash-on::before{background:radial-gradient(ellipse at center,color-mix(in srgb,#1ed760 48%,transparent),color-mix(in srgb,#071b10 88%,transparent))}.mss-screen-flash-off::before{background:radial-gradient(ellipse at center,color-mix(in srgb,#e05252 42%,transparent),color-mix(in srgb,#1c0707 90%,transparent))}.mss-screen-flash span{position:relative;padding:14px 20px;border:1px solid #ffffff66;border-radius:999px;background:#101010aa;font-size:13px;font-weight:900;letter-spacing:.12em;text-align:center;box-shadow:0 8px 32px #0005}@keyframes mss-page-enter{from{opacity:0}to{opacity:1}}@keyframes mss-card-enter{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}}@keyframes mss-screen-transition{0%{opacity:0;transform:scale(1.025)}15%{opacity:1}65%{opacity:.88}100%{opacity:0;transform:scale(1)}}
    @media(max-width:850px){.mss-page{padding:22px 14px 36px}.mss-grid{grid-template-columns:1fr}.mss-hero{flex-direction:column}.mss-settings{grid-template-columns:1fr}.mss-process{grid-template-columns:minmax(0,1fr) auto}.mss-process .mss-memory{grid-column:2;grid-row:1}.mss-tag{display:none}.mss-power{align-self:flex-start}}
    @media(prefers-reduced-motion:reduce){.mss-page,.mss-grid>.mss-card,.mss-screen-flash{animation-duration:1ms!important}.mss-card,.mss-process,.mss-btn{transition:none!important}}
  `;
  document.head.appendChild(style);
}

async function MSSRequest(path, options = {}) {
  const response = await fetch(`${MSS_API}${path}`, {
    method: options.method || 'GET', cache: 'no-store', mode: 'cors', credentials: 'omit',
    headers: { Authorization: `Bearer ${MSS_TOKEN}`, ...(options.body ? { 'Content-Type': 'application/json' } : {}) },
    body: options.body ? JSON.stringify(options.body) : undefined
  });
  let payload = {};
  try { payload = await response.json(); } catch (_) { }
  if (!response.ok) throw new Error(payload.error || `${tx('helperApiError')} (HTTP ${response.status}).`);
  return payload;
}
function MSSFormatMB(value) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return '—';
  return Number(value).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1, minimumFractionDigits: 1 });
}
function MSSManualWarningSeconds(readyAt, now) {
  return Math.max(0, Math.ceil((readyAt - now) / 1000));
}
function MSSWindowLabel(value) {
  return T.window[value] || (MSS_TEXT.en.window[value] || 'Unknown state');
}
function MSSRoleLabel(role) {
  if (!MSS_PT) return role;
  return ({ Main: 'Principal', GPU: 'GPU', Renderer: 'Renderização', Utility: 'Utilitário', Crashpad: 'Relatório de falhas', Helper: 'Auxiliar', Audio: 'Áudio' })[role] || role;
}
function MSSOpenGitHub() {
  window.open(MSS_RELEASES_URL, '_blank', 'noopener,noreferrer');
}
function MSSDebugToggle(value, onChange) {
  const Toggle = Spicetify.ReactComponent && Spicetify.ReactComponent.Toggle;
  if (typeof Toggle !== 'function') {
    return h('button', { className: `mss-btn mss-btn-secondary${value ? ' mss-btn-primary' : ''}`, onClick: () => onChange(!value), 'aria-pressed': value }, tx('debugMode'));
  }
  return h(Toggle, {
    value: Boolean(value),
    onSelected: next => onChange(typeof next === 'boolean' ? next : !value),
    id: 'mss-debug-mode',
    'aria-label': tx('debugMode')
  });
}
function MSSOpenInstallGuide() {
  window.open(MSS_INSTALL_GUIDE_URL, '_blank', 'noopener,noreferrer');
}
function MSSDate(value) {
  return value ? new Date(value).toLocaleTimeString(MSS_LOCALE) : '—';
}
function MSSApp() {
  const R = MSSReact;
  const [status, setStatus] = R.useState(null);
  const [error, setError] = R.useState('');
  const [busy, setBusy] = R.useState(false);
  const [saving, setSaving] = R.useState(false);
  const [settingsLoaded, setSettingsLoaded] = R.useState(false);
  const [savedAutoOptimize, setSavedAutoOptimize] = R.useState(null);
  const [debug, setDebug] = R.useState(() => { try { return window.localStorage.getItem('mss.debugMode') === '1'; } catch (_) { return false; } });
  const [config, setConfig] = R.useState({ enabled: true, autoOptimize: true, hiddenDelaySeconds: 2, trimToCap: true });
  const [manualInputs, setManualInputs] = R.useState({});
  const [manualWarning, setManualWarning] = R.useState(null);
  const manualApplyTimers = R.useRef(new Map());
  const manualInputOrigins = R.useRef(new Map());
  const confirmedManualWarning = R.useRef(null);
  const previousWindowState = R.useRef(null);
  const screenFlashNode = R.useRef(null);
  const screenFlashTimer = R.useRef(null);
  const [manualMessage, setManualMessage] = R.useState(null);

  function applyDebug(next) {
    const enabled = Boolean(next);
    setDebug(enabled);
    try { window.localStorage.setItem('mss.debugMode', enabled ? '1' : '0'); } catch (_) { }
  }
  function showScreenFlash(enabled) {
    if (screenFlashTimer.current) window.clearTimeout(screenFlashTimer.current);
    screenFlashNode.current?.remove();
    const overlay = document.createElement('div');
    overlay.className = `mss-screen-flash ${enabled ? 'mss-screen-flash-on' : 'mss-screen-flash-off'}`;
    overlay.setAttribute('role', 'status');
    overlay.setAttribute('aria-live', 'polite');
    const label = document.createElement('span');
    label.textContent = tx(enabled ? 'pluginActivatedFlash' : 'pluginDisabledFlash');
    overlay.appendChild(label);
    document.body.appendChild(overlay);
    screenFlashNode.current = overlay;
    screenFlashTimer.current = window.setTimeout(() => {
      overlay.remove();
      if (screenFlashNode.current === overlay) screenFlashNode.current = null;
      screenFlashTimer.current = null;
    }, 760);
  }
  async function togglePluginEnabled(enabled) {
    if (saving || !helperRunning || typeof status?.config?.enabled !== 'boolean') return;
    const previousConfig = config;
    setManualWarning(null);
    setManualMessage(null);
    const saved = await saveSettings({ ...config, enabled: Boolean(enabled) });
    if (!saved) {
      setConfig(previousConfig);
      return;
    }
    showScreenFlash(Boolean(enabled));
  }

  R.useEffect(() => {
    let alive = true;
    async function refresh() {
      try {
        const data = await MSSRequest('/api/status');
        if (!alive) return;
        setStatus(data); setError('');
        if (!settingsLoaded && data.config) {
          const loadedConfig = { enabled: data.config.enabled !== false, autoOptimize: data.config.autoOptimize !== false, hiddenDelaySeconds: data.config.hiddenDelaySeconds || 2, trimToCap: data.config.trimToCap !== false };
          setConfig(loadedConfig);
          setSavedAutoOptimize(loadedConfig.autoOptimize);
          setSettingsLoaded(true);
        }
      } catch (err) {
        if (alive) {
          const detail = err && err.message ? String(err.message) : String(err || 'Unknown error');
          setError(tx('statusError') + ' [' + detail + ']');
        }
      }
    }
    refresh();
    const timer = window.setInterval(refresh, 2500);
    return () => { alive = false; window.clearInterval(timer); };
  }, [settingsLoaded]);

  R.useEffect(() => () => {
    for (const timer of manualApplyTimers.current.values()) window.clearTimeout(timer);
    manualApplyTimers.current.clear();
    manualInputOrigins.current.clear();
    if (screenFlashTimer.current) window.clearTimeout(screenFlashTimer.current);
    screenFlashNode.current?.remove();
  }, []);

  R.useEffect(() => {
    const windowState = status?.spotify?.windowState;
    if (!windowState) return;
    if (windowState === 'foreground' && previousWindowState.current && previousWindowState.current !== 'foreground') {
      setManualInputs({});
      manualInputOrigins.current.clear();
    }
    previousWindowState.current = windowState;
  }, [status?.spotify?.windowState]);

  R.useEffect(() => {
    if (!manualWarning) return undefined;
    const popup = Spicetify.PopupModal;
    const usePopupModal = typeof popup?.display === 'function' && typeof popup?.hide === 'function';
    const warning = manualWarning;
    const title = tx('lowLimitTitle', { role: MSSRoleLabel(warning.role), pid: warning.pid });
    const panel = document.createElement('section');
    panel.className = 'mss-modal';
    panel.setAttribute('role', 'alertdialog');
    panel.setAttribute('aria-modal', 'true');
    const warningCopy = document.createElement('p');
    warningCopy.className = 'mss-notice mss-notice-warn';
    warningCopy.setAttribute('aria-live', 'polite');
    const actions = document.createElement('div');
    actions.className = 'mss-actions';
    const confirmButton = document.createElement('button');
    confirmButton.type = 'button';
    confirmButton.className = 'mss-btn mss-btn-danger';
    confirmButton.textContent = tx('waitBeforeApply', { seconds: MSSManualWarningSeconds(warning.readyAt, Date.now()) });
    confirmButton.disabled = true;
    const cancelButton = document.createElement('button');
    cancelButton.type = 'button';
    cancelButton.className = 'mss-btn mss-btn-secondary';
    cancelButton.textContent = tx('cancel');
    actions.append(confirmButton, cancelButton);
    panel.append(warningCopy, actions);

    let fallbackOverlay = null;
    let modalOpen = true;
    let dismissed = false;
    let modalObserver = null;
    let popupWasConnected = false;
    const hideModal = () => {
      if (!modalOpen) return;
      modalOpen = false;
      if (usePopupModal) popup.hide();
      else fallbackOverlay?.remove();
    };
    if (usePopupModal) {
      popup.display({ title, content: panel, isLarge: false });
      popupWasConnected = panel.isConnected;
    } else {
      fallbackOverlay = document.createElement('div');
      fallbackOverlay.className = 'mss-modal-backdrop';
      const fallbackTitle = document.createElement('h2');
      fallbackTitle.className = 'mss-h2';
      fallbackTitle.textContent = title;
      panel.prepend(fallbackTitle);
      fallbackOverlay.appendChild(panel);
      document.body.appendChild(fallbackOverlay);
    }

    let countdownComplete = false;
    const timer = window.setInterval(() => {
      const seconds = MSSManualWarningSeconds(warning.readyAt, Date.now());
      warningCopy.textContent = tx('lowLimitWarning', {
        capMB: warning.capMB,
        defaultMB: warning.defaultMB,
        seconds
      });
      if (countdownComplete) return;
      if (seconds > 0) {
        confirmButton.textContent = tx('waitBeforeApply', { seconds });
        return;
      }
      countdownComplete = true;
      window.clearInterval(timer);
      confirmButton.disabled = false;
      confirmButton.textContent = tx('confirmLowLimit');
    }, 250);
    const closeWarning = modalAlreadyClosed => {
      if (dismissed) return;
      dismissed = true;
      window.clearInterval(timer);
      modalObserver?.disconnect();
      if (modalAlreadyClosed) modalOpen = false;
      else hideModal();
      setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }));
      setManualWarning(null);
    };
    const onConfirm = () => confirmManualLimit(warning, countdownComplete);
    const onCancel = () => closeWarning(false);
    const onBackdropClick = event => {
      if (event.target === fallbackOverlay) closeWarning(false);
    };
    confirmButton.addEventListener('click', onConfirm);
    cancelButton.addEventListener('click', onCancel);
    fallbackOverlay?.addEventListener('click', onBackdropClick);

    if (usePopupModal && typeof MutationObserver === 'function') {
      modalObserver = new MutationObserver(() => {
        if (panel.isConnected) {
          popupWasConnected = true;
        } else if (popupWasConnected) {
          closeWarning(true);
        }
      });
      modalObserver.observe(document.body, { childList: true, subtree: true });
    }
    return () => {
      window.clearInterval(timer);
      modalObserver?.disconnect();
      confirmButton.removeEventListener('click', onConfirm);
      cancelButton.removeEventListener('click', onCancel);
      fallbackOverlay?.removeEventListener('click', onBackdropClick);
      const wasConfirmed = confirmedManualWarning.current === warning.readyAt;
      if (wasConfirmed) confirmedManualWarning.current = null;
      else if (!dismissed) {
        setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }));
      }
      hideModal();
    };
  }, [manualWarning?.readyAt]);

  function setField(key, value) {
    const nextConfig = { ...config, [key]: value };
    setConfig(nextConfig);
    saveSettings(nextConfig);
  }
  async function saveSettings(nextConfig = config) {
    setSaving(true);
    try {
      const result = await MSSRequest('/api/config', { method: 'POST', body: nextConfig });
      const savedConfig = { enabled: typeof result.config?.enabled === 'boolean' ? result.config.enabled : nextConfig.enabled !== false, autoOptimize: result.config.autoOptimize, hiddenDelaySeconds: result.config.hiddenDelaySeconds, trimToCap: result.config.trimToCap !== false };
      setConfig(savedConfig);
      setSavedAutoOptimize(savedConfig.autoOptimize);
      setError('');
      try { setStatus(await MSSRequest('/api/status')); } catch (_) { }
      return true;
    } catch (_) {
      setError(tx('saveFailed'));
      return false;
    } finally { setSaving(false); }
  }

  async function applyManualLimit(process, capMB) {
    setBusy(true);
    setManualMessage(null);
    try {
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'setManualCap', pid: process.pid, creation: process.creation, capMB } });
      const updatedStatus = await MSSRequest('/api/status');
      setStatus(updatedStatus);
      const background = ['minimized', 'hidden', 'background-visible'].includes(updatedStatus.spotify?.windowState);
      setManualMessage({ text: tx(background ? 'limitApplied' : 'limitSavedForBackground'), error: false });
    } catch (err) {
      setManualMessage({ text: err?.message || tx('helperApiError'), error: true });
    } finally { setBusy(false); }
  }
  function scheduleManualLimit(process, key, value) {
    const timers = manualApplyTimers.current;
    if (!manualInputOrigins.current.has(key)) {
      manualInputOrigins.current.set(key, String(manualInputs[key] ?? process.manualCapMB ?? process.defaultLimitMB));
    }
    if (timers.has(key)) window.clearTimeout(timers.get(key));
    const input = String(value);
    if (!input.trim()) return;
    const timer = window.setTimeout(() => {
      timers.delete(key);
      const capMB = Number(input);
      if (!Number.isInteger(capMB) || capMB < 8 || capMB > 512) {
        manualInputOrigins.current.delete(key);
        setManualMessage({ text: tx('limitRange'), error: true });
        return;
      }
      const previousValue = manualInputOrigins.current.get(key);
      manualInputOrigins.current.delete(key);
      void requestManualLimit(process, key, capMB, previousValue);
    }, 700);
    timers.set(key, timer);
  }
  function confirmManualLimit(warning, countdownComplete) {
    if (!countdownComplete) return;
    confirmedManualWarning.current = warning.readyAt;
    setManualWarning(null);
    void applyManualLimit({ pid: warning.pid, creation: warning.creation }, warning.capMB);
  }
  async function requestManualLimit(process, key, capMB, previousValue) {
    const defaultMB = Number(process.defaultLimitMB);
    setManualMessage(null);
    if (!Number.isInteger(capMB) || capMB < 8 || capMB > 512) {
      setManualMessage({ text: tx('limitRange'), error: true });
      return;
    }
    if (capMB === process.manualCapMB) return;
    if (capMB === defaultMB) {
      if (process.manualCapMB != null) await clearManualLimit(process, key);
      return;
    }
    if (capMB < defaultMB) {
      const readyAt = Date.now() + 5000;
      setManualWarning({ key, pid: process.pid, creation: process.creation, role: process.role, capMB, defaultMB, previousValue: String(previousValue ?? process.manualCapMB ?? defaultMB), readyAt });
      return;
    }
    await applyManualLimit(process, capMB);
  }
  async function clearManualLimit(process, key) {
    setBusy(true);
    setManualMessage(null);
    try {
      const timer = manualApplyTimers.current.get(key);
      if (timer) window.clearTimeout(timer);
      manualApplyTimers.current.delete(key);
      manualInputOrigins.current.delete(key);
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'clearManualCap', pid: process.pid, creation: process.creation } });
      setStatus(await MSSRequest('/api/status'));
      setManualInputs(previous => ({ ...previous, [key]: String(process.defaultLimitMB) }));
      setManualMessage({ text: tx('limitCleared'), error: false });
    } catch (err) {
      setManualMessage({ text: err?.message || tx('helperApiError'), error: true });
    } finally { setBusy(false); }
  }

  const spotify = status?.spotify || {};
  const memory = status?.memory || {};
  const telemetry = status?.telemetry || {};
  const helperRunning = Boolean(status?.helper?.running);
  const targets = Array.isArray(memory.targets) ? memory.targets : [];
  const events = Array.isArray(status?.recentEvents) ? status.recentEvents : [];
  const activeCount = status?.cap?.cappedProcesses || 0;
  const manualReady = config.enabled && !config.autoOptimize && savedAutoOptimize === false;
  const card = (label, value, unit, note) => h('section', { className: 'mss-card' }, h('div', { className: 'mss-card-label' }, label), h('div', { className: 'mss-number' }, value, unit ? h('span', { className: 'mss-unit' }, unit) : null), debug && note ? h('div', { className: 'mss-note' }, note) : null);

  return h('main', { className: 'mss-page' },
    h('header', { className: 'mss-hero' },
      h('div', null, h('div', { className: 'mss-kicker' }, tx('kicker')), h('h1', { className: 'mss-title' }, tx('title')), h('div', { className: 'mss-copy' }, tx('intro')),
        h('div', { className: 'mss-small', style: { marginTop: '10px' } }, `${MSSWindowLabel(spotify.windowState || 'not-detected')} · ${MSS_PT ? 'Idioma automático: português' : 'Auto language: English'}`)),
      h('div', { style: { display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' } },
        h('div', { className: 'mss-pill' }, h('span', { className: 'mss-dot', style: { background: error ? '#e05252' : (helperRunning ? 'var(--spice-button,#1ed760)' : '#d6a03d') } }), error ? tx('unavailable') : (helperRunning ? tx('connected') : tx('waiting'))),
        h('label', { className: `mss-power ${config.enabled ? 'mss-power-on' : 'mss-power-off'}`, title: tx('pluginToggleHint') },
          h('input', { type: 'checkbox', role: 'switch', 'aria-label': tx('pluginToggleHint'), checked: Boolean(config.enabled), disabled: saving || !helperRunning || typeof status?.config?.enabled !== 'boolean', onChange: event => { void togglePluginEnabled(event.target.checked); } }),
          h('span', { className: 'mss-power-dot', 'aria-hidden': 'true' }),
          h('span', null, tx(config.enabled ? 'pluginEnabled' : 'pluginDisabled'))
        ),
        h('div', { style: { display: 'flex', gap: '9px', alignItems: 'center' }, title: tx('debugHint') },
          h('span', { className: 'mss-small' }, tx('debugMode')),
          MSSDebugToggle(debug, applyDebug)
        )
      )
    ),
    error ? h('section', { className: 'mss-notice mss-notice-error' }, h('strong', null, tx('helperMissingTitle')), h('p', { style: { margin: '7px 0 0' } }, `${tx('helperMissingBody')} ${error}`), h('div', { className: 'mss-actions' }, h('button', { className: 'mss-btn mss-btn-primary', onClick: MSSOpenGitHub }, tx('helperButton')), h('button', { className: 'mss-btn mss-btn-secondary', onClick: MSSOpenInstallGuide }, tx('installGuideButton')))) : null,
    !config.enabled ? h('div', { className: 'mss-notice mss-notice-warn', role: 'status' }, tx('pluginDisabledNote')) : null,
    h('div', { className: 'mss-notice mss-notice-warn' }, tx('safety')),
    h('div', { className: 'mss-grid' },
      card(tx(memory.privateWorkingSet ? 'privateWorkingSetRam' : 'spotifyRam'), MSSFormatMB(memory.spotifyTotalMB), 'MB', tx(memory.privateWorkingSet ? 'privateWorkingSetNote' : 'ramNote', { count: memory.processCount || 0 })),
      card(tx('optimizedRam'), MSSFormatMB(memory.targetTotalMB), 'MB', tx(config.autoOptimize ? 'managedNote' : 'manualManagedNote', { count: memory.targetCount || 0 })),
      card(tx(config.autoOptimize ? 'activeLimits' : 'manualLimitsCard'), activeCount.toLocaleString(MSS_LOCALE), '', tx(config.autoOptimize ? 'activeNote' : 'manualActiveNote')),
      debug ? card(tx('pageFaults'), telemetry.pageFaultDelta == null ? '—' : Number(telemetry.pageFaultDelta).toLocaleString(MSS_LOCALE), '', tx('pageFaultNote')) : null,
      debug ? card(tx('pageFaultRate'), telemetry.pageFaultsPerSecond == null ? '—' : Number(telemetry.pageFaultsPerSecond).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }), '/s', tx('pageFaultRateNote')) : null,
      debug ? card(tx('adaptiveChanges'), Number(telemetry.adaptiveAdjustments || 0).toLocaleString(MSS_LOCALE), '', tx('adaptiveNote')) : null,
      debug ? card(tx('restoreDuration'), telemetry.lastRestoreDurationMs == null ? '—' : Number(telemetry.lastRestoreDurationMs).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }), telemetry.lastRestoreDurationMs == null ? '' : 'ms', tx('restoreNote')) : null
    ),
    h('section', { className: 'mss-section' },
      h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('processes')), h('span', { className: 'mss-small' }, tx('updated', { time: MSSDate(memory.sampledAt) }))),
      targets.length ? h('div', { className: 'mss-list' }, targets.map(process => {
        const key = `${process.pid}:${process.creation}`;
        const displayedMemoryMB = memory.privateWorkingSet ? process.privateWorkingSetMB : process.workingSetMB;
        const statusText = process.protectedAudio
          ? (config.autoOptimize ? tx('audioProtected') : (process.capped ? tx('manualActive', { value: process.limitMB }) : tx('manualCurrent', { value: process.defaultLimitMB })))
          : (process.capped ? tx(process.adaptiveStatus === 'sustained-high-cpu' ? 'reasonHigh' : (process.adaptiveStatus === 'low-cpu-and-memory-headroom' ? 'reasonLow' : (process.adaptiveStatus === 'baseline' ? 'reasonBaseline' : 'reasonHolding'))) : (process.included ? tx('willOptimize') : tx('notIncluded')));
        const meta = [debug && Number.isFinite(process.cpuPercent) ? tx('cpu', { value: Number(process.cpuPercent).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }) }) : null, debug && process.privateMB != null ? tx('privateMemory', { value: MSSFormatMB(process.privateMB) }) : null, debug ? statusText : null].filter(Boolean).join(' · ');
        const defaultMB = Number(process.defaultLimitMB || 36);
        const capText = process.capped && process.manual
          ? tx('manualActive', { value: process.limitMB ?? defaultMB })
          : process.manualCapMB != null
            ? tx('manualConfigured', { value: process.manualCapMB })
            : tx('manualCurrent', { value: defaultMB });
        return h('div', { className: 'mss-process', key },
          h('div', null, h('div', { className: 'mss-process-name' }, `${MSSRoleLabel(process.role)} · PID ${process.pid}`), meta ? h('div', { className: 'mss-process-meta' }, meta) : null),
          h('span', { className: 'mss-tag' }, process.manual ? (MSS_PT ? 'Manual' : 'Manual') : (process.protectedAudio ? (MSS_PT ? 'Áudio' : 'Audio') : (process.capped ? (MSS_PT ? 'Adaptativo' : 'Adaptive') : process.role))),
          h('div', { className: 'mss-memory' }, displayedMemoryMB == null ? '—' : `${MSSFormatMB(displayedMemoryMB)} MB${process.capped ? ` / ${process.limitMB} MB` : ''}`),
          manualReady ? h('div', { className: 'mss-manual-controls' },
            h('label', { className: 'mss-small', htmlFor: `mss-limit-${key}` }, tx('manualLimit')),
            h('input', { id: `mss-limit-${key}`, className: 'mss-input', type: 'number', min: 8, max: 512, step: 1, value: manualInputs[key] ?? String(defaultMB), disabled: busy || !config.enabled, onChange: event => { const value = event.target.value; setManualInputs(previous => ({ ...previous, [key]: value })); scheduleManualLimit(process, key, value); } }),
            h('span', { className: 'mss-small' }, capText)
          ) : null
        );
      })) : h('div', { className: 'mss-empty' }, tx('noneDetected')),
      config.enabled && !config.autoOptimize && !manualReady ? h('div', { className: 'mss-notice mss-notice-warn' }, tx('saveBeforeManual')) : null,
      manualMessage ? h('div', { className: `mss-notice${manualMessage.error ? ' mss-notice-error' : ''}` }, manualMessage.text) : null,
      debug && telemetry.lastAdaptiveAdjustmentAt ? h('div', { className: 'mss-actions' }, h('span', { className: 'mss-small' }, tx('lastAdjustment', { time: MSSDate(telemetry.lastAdaptiveAdjustmentAt) }))) : null,
      debug && status?.lastAction ? h('div', { className: 'mss-notice' }, tx('lastAction', { count: status.lastAction.applied || 0 })) : null
    ),
    h('section', { className: 'mss-section' },
      h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('behavior')), h('span', { className: 'mss-small' }, debug && helperRunning ? `${status?.helper?.runtime || 'Server'} ${status?.helper?.version || ''}` : (MSS_PT ? 'Servidor' : 'Server'))),
      h('label', { className: 'mss-check', style: { marginBottom: '18px' } }, h('input', { type: 'checkbox', role: 'switch', disabled: saving || !helperRunning || !config.enabled, checked: Boolean(config.autoOptimize), onChange: event => { setManualWarning(null); setField('autoOptimize', event.target.checked); } }), h('span', null, h('strong', null, tx('active')), h('br'), h('span', { className: 'mss-small' }, tx('autoExplanation')))),
      !config.autoOptimize ? h('div', { className: 'mss-notice' }, h('strong', null, tx('manualMode')), ' · ', tx('manualExplanation')) : null,
      h('div', { className: 'mss-settings' }, h('label', { className: 'mss-setting' }, h('span', { className: 'mss-setting-label' }, tx('delay')), h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled || !config.autoOptimize, value: String(config.hiddenDelaySeconds), onChange: event => setField('hiddenDelaySeconds', Number(event.target.value)) }, [1, 2, 3, 5, 8, 10, 15, 20, 30].map(n => h('option', { key: n, value: String(n) }, tx('seconds', { value: n })))), h('span', { className: 'mss-small' }, tx('delayExplanation'))),
        h('label', { className: 'mss-setting' }, h('span', { className: 'mss-setting-label' }, tx('releaseMode')), h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled || !config.autoOptimize, value: config.trimToCap !== false ? 'ceiling' : 'aggressive', onChange: event => setField('trimToCap', event.target.value === 'ceiling') }, h('option', { value: 'ceiling' }, tx('releaseCeiling')), h('option', { value: 'aggressive' }, tx('releaseAggressive'))), h('span', { className: 'mss-small' }, tx('releaseModeExplanation')))),
      h('div', { className: 'mss-actions' }, h('span', { className: 'mss-small' }, tx(saving ? 'saving' : 'autoSaved')))
    ),
    debug ? h('section', { className: 'mss-section' }, h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('events')), h('span', { className: 'mss-small' }, status?.helper ? `${status.helper.runtime || 'Server'} ${status.helper.version || ''}` : (MSS_PT ? 'Servidor' : 'Server'))),
      events.length ? h('div', { className: 'mss-events' }, events.map((event, i) => h('div', { className: 'mss-event', key: `${event.time}-${i}` }, h('span', { className: 'mss-event-time' }, MSSDate(event.time)), h('span', { className: 'mss-event-msg' }, MSSLocalizeEvent(event.message))))) : h('div', { className: 'mss-empty' }, tx('noEvents')),
      status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null
    ) : (status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null),

    h('footer', { className: 'mss-footer' }, tx('footer'))
  );
}
function render() { MSSEnsureStyles(); return h(MSSApp, {}); }
