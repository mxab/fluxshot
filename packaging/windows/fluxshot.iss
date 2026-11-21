; FluxShot per-user installer (Inno Setup 6). Built by build-installer.ps1:
;   iscc /DAppVersion=1.2.3 /DSourceExe=path\to\fluxshot.exe /DOutputDir=out fluxshot.iss
; No admin rights needed: installs to %LOCALAPPDATA%\Programs\FluxShot.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\release\fluxshot.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist\windows-installer"
#endif

[Setup]
; Never change AppId: it ties upgrades and the uninstaller together.
AppId={{2E00CF15-8E5F-4F8F-82C9-2E20C5625086}
AppName=FluxShot
AppVersion={#AppVersion}
AppVerName=FluxShot {#AppVersion}
AppPublisher=mxab
AppPublisherURL=https://github.com/mxab/fluxshot
AppSupportURL=https://github.com/mxab/fluxshot/issues
AppUpdatesURL=https://github.com/mxab/fluxshot/releases
PrivilegesRequired=lowest
DefaultDirName={autopf}\FluxShot
DisableProgramGroupPage=yes
DisableDirPage=auto
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10 1703, same as the app itself.
MinVersion=10.0.15063
OutputDir={#OutputDir}
OutputBaseFilename=fluxshot_{#AppVersion}_windows_amd64_setup
SetupIconFile=..\..\assets\fluxshot.ico
UninstallDisplayIcon={app}\fluxshot.exe
UninstallDisplayName=FluxShot
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; Lets upgrades close a running tray via the Restart Manager.
CloseApplications=yes
RestartApplications=no
ChangesEnvironment=yes

[Tasks]
Name: autostart; Description: "Start the FluxShot tray when I sign in"
Name: addtopath; Description: "Add fluxshot to PATH (for the command line)"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\FluxShot"; Filename: "{app}\fluxshot.exe"; Parameters: "tray"; \
  Comment: "Screenshot tray: hotkey, recent shots, cleanup"
Name: "{autoprograms}\FluxShot Capture"; Filename: "{app}\fluxshot.exe"; \
  Comment: "Select a region and annotate it"

[Registry]
; Same value the tray's "Launch at login" toggle writes (src/tray/mod.rs), so
; the two stay in sync.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
  ValueType: string; ValueName: "FluxShot"; ValueData: """{app}\fluxshot.exe"" tray"; \
  Tasks: autostart

[Run]
Filename: "{app}\fluxshot.exe"; Parameters: "tray"; Description: "Start the FluxShot tray now"; \
  Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM fluxshot.exe"; Flags: runhidden; \
  RunOnceId: "StopFluxShot"

[Code]
const
  EnvKey = 'Environment';

function PathContains(Paths, Dir: string): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

procedure AddToPath(Dir: string);
var
  Paths: string;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Paths) then
    Paths := '';
  if PathContains(Paths, Dir) then
    exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Paths + Dir);
end;

procedure RemoveFromPath(Dir: string);
var
  Paths: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Paths) then
    exit;
  P := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';');
  if P = 0 then
    exit;
  { P indexes into ';' + Paths, so it is where Dir starts in Paths. Also drop
    the separator after it, or the one before it if Dir was last. }
  if P + Length(Dir) <= Length(Paths) then
    Delete(Paths, P, Length(Dir) + 1)
  else if P > 1 then
    Delete(Paths, P - 1, Length(Dir) + 1)
  else
    Delete(Paths, P, Length(Dir));
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    { Also covers autostart enabled later from the tray menu. }
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'FluxShot');
  end;
end;
