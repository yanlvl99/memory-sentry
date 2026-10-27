#ifndef AppVersion
  #define AppVersion "1.0.0-rc.3"
#endif
#ifndef StageDir
  #define StageDir "..\..\dist\stage"
#endif

[Setup]
AppId={{C0C9DB49-DBD6-4985-84A1-C4D0C1AA98EC}
AppName=Memory Sentry
AppVersion={#AppVersion}
AppVerName=Memory Sentry {#AppVersion}
AppPublisher=Memory Sentry Contributors
AppPublisherURL=https://github.com/yanlvl99/memory-sentry
AppSupportURL=https://github.com/yanlvl99/memory-sentry/issues
AppUpdatesURL=https://github.com/yanlvl99/memory-sentry/releases
DefaultDirName={localappdata}\SpicetifyMemorySentry
DisableDirPage=yes
DisableProgramGroupPage=yes
DefaultGroupName=Memory Sentry
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64
WizardStyle=modern
LicenseFile={#StageDir}\LICENSE
UninstallDisplayIcon={app}\MemorySentry.exe
OutputDir=..\..\dist
OutputBaseFilename=MemorySentrySetup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
CloseApplications=no
RestartApplications=no
UsePreviousLanguage=yes
LanguageDetectionMethod=uilanguage
ShowLanguageDialog=no
SetupLogging=yes
VersionInfoVersion=1.0.0.3
Uninstallable=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
english.OSCheckFailed=Memory Sentry supports Windows 10 and Windows 11 desktop editions only. No files were changed.
brazilianportuguese.OSCheckFailed=O Memory Sentry é compatível apenas com as edições desktop do Windows 10 e do Windows 11. Nenhum arquivo foi alterado.
english.InstallCheckFailed=Setup could not verify all prerequisites. Read the diagnostic shown by Setup and try again. No program files have been replaced.
brazilianportuguese.InstallCheckFailed=O instalador não conseguiu confirmar todos os requisitos. Leia o diagnóstico exibido pelo instalador e tente novamente. Nenhum arquivo do programa foi substituído.
english.SpicetifyMissing=Spicetify CLI was not found for this Windows user. Install Spicetify, reopen Setup, and try again. No program files have been replaced.
brazilianportuguese.SpicetifyMissing=O Spicetify CLI não foi encontrado para este usuário do Windows. Instale o Spicetify, abra o instalador novamente e tente de novo. Nenhum arquivo do programa foi substituído.
english.UpgradeCheckFailed=The previous server did not confirm that its recovery state was safe. Setup has stopped; the existing files and recovery data were preserved.
brazilianportuguese.UpgradeCheckFailed=O servidor anterior não confirmou que a recuperação estava segura. A instalação foi interrompida; os arquivos existentes e os dados de recuperação foram preservados.
english.ConfigureFailed=Files were copied, but Memory Sentry could not register its Custom App. Setup tried to roll back the Custom App if recovery completed. Review the setup output and rerun Setup to repair the installation.
brazilianportuguese.ConfigureFailed=Os arquivos foram copiados, mas o Memory Sentry não conseguiu registrar seu Custom App. O instalador tentou reverter o Custom App se a recuperação foi concluída. Verifique a saída e execute o instalador novamente para reparar a instalação.
english.ServerMissing=The server executable is missing from this setup package. Download the complete official release package.
brazilianportuguese.ServerMissing=O executável do servidor não está neste pacote de instalação. Baixe o pacote oficial completo.
english.UninstallStopped=Uninstall stopped because recovery or unregistering did not complete. Review the error and retry. No program files were removed.
brazilianportuguese.UninstallStopped=A desinstalação foi interrompida porque a recuperação ou a remoção do registro não foi concluída. Confira o erro e tente novamente. Nenhum arquivo do programa foi removido.
english.RecoveryDataExists=Recovery data exists but the server executable is missing. Uninstall stopped to avoid losing recovery records. Reinstall the matching package first.
brazilianportuguese.RecoveryDataExists=Há dados de recuperação, mas o executável do servidor está ausente. A desinstalação foi interrompida para evitar perder os registros. Reinstale primeiro o pacote correspondente.
english.StartMemorySentry=Start Memory Sentry now
brazilianportuguese.StartMemorySentry=Iniciar o Memory Sentry agora
english.InstallationGuide=Installation guide
brazilianportuguese.InstallationGuide=Guia de instalação

[Files]
Source: "{#StageDir}\MemorySentry.exe"; DestDir: "{tmp}"; Flags: dontcopy
Source: "{#StageDir}\MemorySentry.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\app\*"; DestDir: "{userappdata}\spicetify\CustomApps\SpicetifyMemorySentry"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#StageDir}\README.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\PRIVACY.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\SECURITY.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\docs\INSTALLATION.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\docs\TROUBLESHOOTING.md"; DestDir: "{app}\docs"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Memory Sentry\{cm:InstallationGuide}"; Filename: "{app}\docs\INSTALLATION.md"
Name: "{autoprograms}\Memory Sentry\Uninstall Memory Sentry"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\MemorySentry.exe"; Parameters: "--launch"; Description: "{cm:StartMemorySentry}"; Flags: runhidden

[UninstallDelete]
Type: filesandordirs; Name: "{userappdata}\spicetify\CustomApps\SpicetifyMemorySentry"
Type: filesandordirs; Name: "{app}\logs"
Type: files; Name: "{app}\settings.json"
Type: files; Name: "{app}\helper.pid"
Type: files; Name: "{app}\working-set-backup.tsv"
Type: files; Name: "{app}\working-set-backup.previous"
Type: files; Name: "{app}\working-set-backup.tsv.tmp"
Type: files; Name: "{app}\working-set-backup.json"
Type: files; Name: "{app}\helper.js"
Type: filesandordirs; Name: "{app}\node_modules"

[Code]
function RunBundledServer(const Params: String; var Code: Integer): Boolean;
begin
  Result := Exec(ExpandConstant('{tmp}\MemorySentry.exe'), Params, '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function RunInstalledServer(const Params: String; var Code: Integer): Boolean;
begin
  Result := Exec(ExpandConstant('{app}\MemorySentry.exe'), Params, '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function SetupFailureDetails(): String;
var
  LogText: AnsiString;
  LogFile: String;
begin
  Result := '';
  LogFile := ExpandConstant('{localappdata}\SpicetifyMemorySentry\logs\setup.log');
  if FileExists(LogFile) and LoadStringFromFile(LogFile, LogText) then begin
    if Length(LogText) > 1800 then
      LogText := Copy(LogText, Length(LogText) - 1799, 1800);
    Result := Trim(LogText);
  end;
end;

function InitializeSetup(): Boolean;
var
  Code: Integer;
begin
  Result := False;
  try
    ExtractTemporaryFile('MemorySentry.exe');
  except
    MsgBox(CustomMessage('ServerMissing'), mbError, MB_OK);
    Exit;
  end;
  if (not RunBundledServer('--check-os', Code)) or (Code <> 0) then begin
    MsgBox(CustomMessage('OSCheckFailed'), mbError, MB_OK);
    Exit;
  end;
  Result := True;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Result := '';
  if not RunBundledServer('--check-spicetify', Code) then begin
    Result := CustomMessage('InstallCheckFailed');
    Exit;
  end;
  if Code <> 0 then begin
    Result := CustomMessage('SpicetifyMissing');
    Exit;
  end;
  if (not RunBundledServer('--check-install', Code)) or (Code <> 0) then begin
    Result := CustomMessage('InstallCheckFailed');
    Exit;
  end;
  if (not RunBundledServer('--prepare-upgrade', Code)) or (Code <> 0) then begin
    Result := CustomMessage('UpgradeCheckFailed');
    Exit;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
  CleanupCode: Integer;
  AppTarget: String;
  Diagnostic: String;
begin
  if CurStep = ssPostInstall then begin
    AppTarget := ExpandConstant('{userappdata}\spicetify\CustomApps\SpicetifyMemorySentry');
    if (not Exec(ExpandConstant('{app}\MemorySentry.exe'), '--configure "' + AppTarget + '"', '', SW_HIDE, ewWaitUntilTerminated, Code)) or (Code <> 0) then begin
      // Only roll back the external Custom App if the server confirms recovery/unregistration.
      if RunInstalledServer('--pre-uninstall', CleanupCode) and (CleanupCode = 0) then
        DelTree(AppTarget, True, True, True);
      Diagnostic := SetupFailureDetails();
      if Diagnostic <> '' then begin
        MsgBox(CustomMessage('ConfigureFailed') + #13#10#13#10 + Diagnostic, mbError, MB_OK);
        RaiseException(CustomMessage('ConfigureFailed') + #13#10 + Diagnostic);
      end else begin
        MsgBox(CustomMessage('ConfigureFailed'), mbError, MB_OK);
        RaiseException(CustomMessage('ConfigureFailed'));
      end;
    end;
  end;
end;

function InitializeUninstall(): Boolean;
var
  Code: Integer;
  RecoveryTsv: String;
  RecoveryJson: String;
begin
  Result := False;
  RecoveryTsv := ExpandConstant('{app}\working-set-backup.tsv');
  RecoveryJson := ExpandConstant('{app}\working-set-backup.json');
  if FileExists(ExpandConstant('{app}\MemorySentry.exe')) then begin
    if (not RunInstalledServer('--pre-uninstall', Code)) or (Code <> 0) then begin
      MsgBox(CustomMessage('UninstallStopped'), mbError, MB_OK);
      Exit;
    end;
    Result := True;
    Exit;
  end;
  if FileExists(RecoveryTsv) or FileExists(RecoveryJson) then begin
    MsgBox(CustomMessage('RecoveryDataExists'), mbError, MB_OK);
    Exit;
  end;
  Result := True;
end;
