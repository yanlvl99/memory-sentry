// Localization: English source strings, automatic Brazilian Portuguese, and log translation.
const MSS_TEXT = {
  en: {
    kicker: 'SPICETIFY · WINDOWS MEMORY TOOL · RELEASE CANDIDATE v1.0.0-rc.3', title: 'Memory Sentry', brandSubtitle: 'Spotify RAM tool for Windows', ramCard: 'Spotify RAM', processesCard: 'Processes',
    introShort: 'Automatic or per-process limits for Spotify in the background.',
    intro: 'Choose automatic or per-process manual limits for Spotify while it is outside the foreground, and pick which window states start optimization. The Disabled mode turns memory management off; original Windows limits are saved before changes and restored when management is disabled or Spotify returns to the foreground.',
    connected: 'Server connected', unavailable: 'Connection unavailable', waiting: 'Waiting for server',
    pluginEnabled: 'Memory Sentry on', pluginDisabled: 'Memory Sentry off', powerOn: 'On', powerOff: 'Off', pluginToggleHint: 'Turn memory management on or off; disabling restores original process limits.', pluginDisabledNote: 'Memory Sentry is off. Original limits are restored and no automatic or manual caps will be applied.', pluginActivatedFlash: 'MEMORY SENTRY ACTIVATED', pluginDisabledFlash: 'MEMORY SENTRY DEACTIVATED', pluginDisabledRestoring: 'Memory Sentry is off. It is retrying restoration of the original limits for {count} process(es); no new limits will be applied.',
    helperMissingTitle: 'Memory Sentry server is not connected',
    helperMissingBody: 'The Marketplace listing only links to this project and cannot install Memory Sentry. Use the official Setup from GitHub Releases to install or repair the Memory Sentry server.',
    helperButton: 'Download official setup', installGuideButton: 'Installation guide', safetySummary: 'How limits and risks work', safety: 'On supported Windows 10/11 x64 PCs, automatic optimization is intended to reduce Spotify resident RAM while minimized, hidden, or in the background; actual savings vary by PC and workload. Main and Audio processes have a 36 MB automatic ceiling; Audio is fixed while Main may adapt only below that ceiling. A manual limit below the process default can cause failures. A working-set ceiling does not necessarily reduce resident memory immediately; this changes resident working-set memory, not committed memory. Very low-end PCs may briefly show a black window on restore; that alone does not prove a reload if the previous UI state returns. Input may take a few seconds to respond.',
    spotifyRam: 'Spotify total working-set memory', privateWorkingSetRam: 'Spotify private working-set memory', ramNote: 'Includes shared pages and can exceed Task Manager memory.', privateWorkingSetNote: 'Excludes shared pages; comparable to Task Manager memory.',
    spotifyProcesses: 'Processes in Spotify tree', memoryLabel: 'memory', capLabel: 'ceiling', noCapLabel: 'no ceiling', hoverHint: 'Hover the bar of a process to see memory and ceiling.', stateLabel: 'State', segAuto: 'Automatic', segManual: 'Manual', segDisabled: 'Disabled', withCap: 'with ceiling', modeLabel: 'Memory mode', tagOver: 'Over ceiling', tagManual: 'Manual', tagAdaptive: 'Adaptive', tagFixed: 'Fixed', tagNoCeiling: 'No ceiling', delayShort: 'Delay', releaseShort: 'Release', autoSavedShort: 'saved automatically', whenLabel: 'Optimize when', whenBackground: 'Window behind', whenMinimized: 'Minimized', whenHidden: 'Hidden',
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
    behavior: 'Memory policy', active: 'Automatic policy enabled', autoExplanation: 'When enabled, limits apply after Spotify enters a window state you enable under Optimize when (window behind, minimized or hidden). Main never exceeds 36 MB; Audio stays fixed at 36 MB. Choose Manual to set a limit for each process role, or Disabled to stop managing memory.',
    manualMode: 'Manual mode', manualExplanation: 'Role defaults are applied after the selected delay when Spotify enters a window state you enable under Optimize when: Main/Audio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB, Crashpad 16 MB. A manual value is saved per role and applies to matching Spotify processes, even after Spotify restarts and their PIDs change. Original limits are captured and restored separately for each process instance.',
    delay: 'Delay after leaving foreground', delayExplanation: 'Wait this long before applying limits in automatic or manual mode, avoiding brief background changes.', backgroundSettings: 'Background behavior', backgroundSettingsHint: 'These options are shared by automatic and manual modes.',
    manualLimit: 'Shared role limit (MB)', manualCurrent: 'Default for background: {value} MB', manualActive: 'Manual limit active: {value} MB', manualConfigured: 'Saved for this role: {value} MB',
    lowLimitWarning: 'The requested {capMB} MB limit is below this process default ({defaultMB} MB) and may cause slowdowns, glitches, or failures. Review the risk before continuing. You can proceed in {countdown}.',
    lowLimitTitle: 'Low limit warning · {role} PID {pid}', waitBeforeApply: 'Wait {seconds}s…', confirmLowLimit: 'Apply lower limit', cancel: 'Cancel', limitRange: 'Enter a whole number from 8 to 512 MB.', limitApplied: 'Shared role limit applied for the background state.', limitSavedForBackground: 'Shared role limit saved; it will apply when Spotify leaves the foreground.', limitCleared: 'Saved role limit removed; role defaults will apply in the background.', saveBeforeManual: 'Manual controls activate after the automatic policy is saved as off.',
    releaseMode: 'Memory release', releaseCeiling: 'Release only down to the ceiling', releaseAggressive: 'Release as much as possible',
    releaseModeExplanation: 'Ceiling mode avoids a forced trim. Aggressive mode also requests a one-time trim when applying a manual limit or lowering an automatic ceiling.',
    debugMode: 'Debug mode', debugHint: 'Shows diagnostics: telemetry cards, activity log and per-process details.',
    second: '{value} second', seconds: '{value} seconds', saving: 'Saving…', autoSaved: 'Changes are saved automatically on this PC.',
    events: 'Recent activity', noEvents: 'No activity recorded yet.', statusError: 'Could not connect to the local Memory Sentry server. The diagnostic appears in brackets; check startup.log in %LOCALAPPDATA%\\SpicetifyMemorySentry\\logs.',
    footer: 'Supported OS: Windows 10 and Windows 11 desktop editions only. The Memory Sentry server is open source under GPL-3.0. No telemetry is uploaded. CPU and memory diagnostics stay on this PC. A separate process is required to call Win32 working-set APIs.',
    saveFailed: 'Could not save settings.', helperApiError: 'The local server returned an error.',
    window: { foreground: 'Spotify in foreground', 'background-visible': 'Spotify visible in background', minimized: 'Spotify minimized', hidden: 'Spotify window hidden', 'not-running': 'Spotify is not running', 'window-not-found': 'Spotify found; window not located', 'window-check-error': 'Window state unavailable', 'not-detected': 'Waiting for detection' }
  },
  pt: {
    kicker: 'SPICETIFY · FERRAMENTA DE RAM PARA WINDOWS · RELEASE CANDIDATE v1.0.0-rc.3', title: 'Memory Sentry', brandSubtitle: 'Ferramenta de RAM para o Spotify', ramCard: 'RAM do Spotify', processesCard: 'Processos',
    introShort: 'Limites automáticos ou por processo para o Spotify em segundo plano.',
    intro: 'Escolha limites automáticos ou manuais por processo para o Spotify enquanto ele está fora do primeiro plano e defina quais estados de janela iniciam a otimização. O modo Desativado desliga o gerenciamento; os limites originais do Windows são salvos antes das alterações e restaurados ao desativar o gerenciamento ou quando o Spotify volta ao primeiro plano.',
    connected: 'Servidor conectado', unavailable: 'Conexão indisponível', waiting: 'Aguardando servidor',
    pluginEnabled: 'Memory Sentry ativado', pluginDisabled: 'Memory Sentry desativado', powerOn: 'Ativado', powerOff: 'Desativado', pluginToggleHint: 'Ativa ou desativa o gerenciamento de memória; ao desativar, restaura os limites originais.', pluginDisabledNote: 'O Memory Sentry está desativado. Os limites originais foram restaurados e nenhum limite será aplicado.', pluginActivatedFlash: 'MEMORY SENTRY ATIVADO', pluginDisabledFlash: 'MEMORY SENTRY DESATIVADO', pluginDisabledRestoring: 'O Memory Sentry está desativado. Tentando restaurar os limites originais de {count} processo(s); nenhum novo limite será aplicado.',
    helperMissingTitle: 'O servidor do Memory Sentry não está conectado',
    helperMissingBody: 'A listagem do Marketplace apenas leva ao projeto e não instala o Memory Sentry. Use o instalador oficial em GitHub Releases para instalar ou reparar o servidor do Memory Sentry.',
    helperButton: 'Baixar instalador oficial', installGuideButton: 'Guia de instalação', safetySummary: 'Como os limites e riscos funcionam', safety: 'Em PCs compatíveis com Windows 10/11 x64, a otimização automática busca reduzir a RAM residente do Spotify enquanto minimizado, oculto ou em segundo plano; a economia varia conforme o PC e a carga. Main e Áudio têm teto automático de 36 MB; o Áudio é fixo e o Main só pode se adaptar abaixo desse teto. Um limite manual abaixo do padrão do processo pode causar falhas. Um teto de working set não necessariamente reduz a memória residente imediatamente e não altera a memória comprometida. Em PCs muito fracos, a janela pode ficar preta por alguns instantes ao restaurar; isso não prova uma recarga se a interface volta ao estado anterior. A entrada pode demorar alguns segundos para responder.',
    spotifyRam: 'Memória total do working set do Spotify', privateWorkingSetRam: 'Memória privada do working set do Spotify', ramNote: 'Inclui páginas compartilhadas e pode superar o valor do Gerenciador de Tarefas.', privateWorkingSetNote: 'Exclui páginas compartilhadas; comparável à memória do Gerenciador de Tarefas.',
    spotifyProcesses: 'Processos na árvore do Spotify', memoryLabel: 'memória', capLabel: 'teto', noCapLabel: 'sem teto', hoverHint: 'Passe o mouse na barra de um processo para ver memória e teto.', stateLabel: 'Estado', segAuto: 'Automático', segManual: 'Manual', segDisabled: 'Desativado', withCap: 'com teto', modeLabel: 'Modo de memória', tagOver: 'Acima do teto', tagManual: 'Manual', tagAdaptive: 'Adaptativo', tagFixed: 'Fixo', tagNoCeiling: 'Sem teto', delayShort: 'Atraso', releaseShort: 'Liberação', autoSavedShort: 'salvo automaticamente', whenLabel: 'Otimizar quando', whenBackground: 'Janela atrás', whenMinimized: 'Minimizado', whenHidden: 'Oculto',
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
    behavior: 'Política de memória', active: 'Política automática ativada', autoExplanation: 'Quando ativada, os limites são aplicados depois que o Spotify entra em um estado habilitado em Otimizar quando (janela atrás, minimizado ou oculto). Main não passa de 36 MB; o Áudio fica fixo em 36 MB. Escolha Manual para definir limites por função do processo, ou Desativado para parar de gerenciar a memória.',
    manualMode: 'Modo manual', manualExplanation: 'Os padrões por função são aplicados após o atraso selecionado quando o Spotify entra em um estado habilitado em Otimizar quando: Main/Áudio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB e Crashpad 16 MB. O limite manual é salvo por função e aplicado aos processos Spotify correspondentes, inclusive após reiniciar o Spotify e os PIDs mudarem. Os limites originais são capturados e restaurados separadamente para cada instância.',
    delay: 'Atraso após sair do primeiro plano', delayExplanation: 'Aguarda esse tempo antes de aplicar limites nos modos automático ou manual, evitando mudanças rápidas para segundo plano.', backgroundSettings: 'Comportamento em segundo plano', backgroundSettingsHint: 'Estas opções são compartilhadas pelos modos automático e manual.',
    manualLimit: 'Limite compartilhado da função (MB)', manualCurrent: 'Padrão em segundo plano: {value} MB', manualActive: 'Limite manual ativo: {value} MB', manualConfigured: 'Salvo para esta função: {value} MB',
    lowLimitWarning: 'O limite solicitado de {capMB} MB é menor que o padrão deste processo ({defaultMB} MB) e pode causar lentidão, falhas visuais ou erros. Revise o risco antes de continuar. Você poderá prosseguir em {countdown}.',
    lowLimitTitle: 'Aviso de limite baixo · {role} PID {pid}', waitBeforeApply: 'Aguarde {seconds}s…', confirmLowLimit: 'Aplicar limite menor', cancel: 'Cancelar', limitRange: 'Digite um número inteiro entre 8 e 512 MB.', limitApplied: 'Limite compartilhado da função aplicado em segundo plano.', limitSavedForBackground: 'Limite compartilhado da função salvo; será aplicado quando o Spotify sair do primeiro plano.', limitCleared: 'Limite salvo da função removido; os padrões serão aplicados em segundo plano.', saveBeforeManual: 'Os controles manuais serão liberados após salvar a política automática como desativada.',
    releaseMode: 'Liberação de memória', releaseCeiling: 'Libera somente até o teto', releaseAggressive: 'Libera o máximo possível',
    releaseModeExplanation: 'O modo teto evita uma liberação forçada. O modo agressivo também solicita uma liberação pontual ao aplicar um limite manual ou reduzir um teto automático.',
    debugMode: 'Modo debug', debugHint: 'Mostra diagnósticos: cartões de telemetria, log de atividade e detalhes por processo.',
    second: '{value} segundo', seconds: '{value} segundos', saving: 'Salvando…', autoSaved: 'As alterações são salvas automaticamente neste PC.',
    events: 'Atividade recente', noEvents: 'Ainda não há atividade registrada.', statusError: 'Não foi possível conectar ao servidor local do Memory Sentry. O diagnóstico aparece entre colchetes; consulte startup.log em %LOCALAPPDATA%\\SpicetifyMemorySentry\\logs.',
    footer: 'Compatível apenas com Windows 10 e Windows 11 (desktop). O servidor do Memory Sentry é código aberto sob GPL-3.0. Nenhuma telemetria é enviada. Os diagnósticos de CPU e memória ficam neste PC. Um processo separado é necessário para chamar as APIs Win32 de working set.',
    saveFailed: 'Não foi possível salvar as configurações.', helperApiError: 'O servidor local retornou um erro.',
    window: { foreground: 'Spotify em primeiro plano', 'background-visible': 'Spotify visível em segundo plano', minimized: 'Spotify minimizado', hidden: 'Janela do Spotify oculta', 'not-running': 'Spotify não está aberto', 'window-not-found': 'Spotify detectado; janela não localizada', 'window-check-error': 'Estado da janela indisponível', 'not-detected': 'Aguardando detecção' }
  }
};
const T = MSS_PT ? MSS_TEXT.pt : MSS_TEXT.en;
const MSS_ROLE_NAMES = MSS_PT
  ? { Main: 'principal', GPU: 'gráfico', Audio: 'áudio', Utility: 'utilitário', Renderer: 'interface', Crashpad: 'crashpad', Helper: 'auxiliar' }
  : { Main: 'primary', GPU: 'graphics', Audio: 'audio', Utility: 'utility', Renderer: 'ui', Crashpad: 'crashpad', Helper: 'helper' };
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
