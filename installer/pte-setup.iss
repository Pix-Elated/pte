; Pixelated's Tibia Editor (PTE) - InnoSetup Installer Script
; Requires InnoSetup 6.x
; Compile with: iscc installer/pte-setup.iss

#define MyAppName "Pixelated's Tibia Editor"
#define MyAppShortName "PTE"
#define MyAppPublisher "Pix-Elated"
#define MyAppURL "https://github.com/Pix-Elated/pte"
#define MyAppExeName "pte.exe"

; Version is injected by CI via /D flag: iscc /DMyAppVersion=0.1.0
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif

[Setup]
AppId={{7B2C3D4E-5F6A-7B8C-9D0E-1F2A3B4C5D6E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} v{#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppShortName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
; Output settings - overridden by CI
OutputDir=..\dist
OutputBaseFilename=pte-{#MyAppVersion}-setup
; Compression
Compression=lzma2/ultra64
SolidCompression=yes
; Privileges
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Visual
WizardStyle=modern
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
; Architecture
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Misc
VersionInfoVersion={#MyAppVersion}.0
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup
VersionInfoProductName={#MyAppShortName}
MinVersion=10.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate_otbm"; Description: "Associate .otbm files with PTE"; GroupDescription: "File associations:"
Name: "add_to_path"; Description: "Add PTE to PATH"; GroupDescription: "System integration:"; Flags: unchecked

[Files]
Source: "..\target\release\pte.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; .otbm file association
Root: HKA; Subkey: "Software\Classes\.otbm"; ValueType: string; ValueName: ""; ValueData: "PTE.OTBMFile"; Flags: uninsdeletevalue; Tasks: associate_otbm
Root: HKA; Subkey: "Software\Classes\PTE.OTBMFile"; ValueType: string; ValueName: ""; ValueData: "Open Tibia Binary Map"; Flags: uninsdeletekey; Tasks: associate_otbm
Root: HKA; Subkey: "Software\Classes\PTE.OTBMFile\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: associate_otbm
Root: HKA; Subkey: "Software\Classes\PTE.OTBMFile\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associate_otbm

; Install mode marker (so updater knows we're installed vs portable)
Root: HKCU; Subkey: "Software\PTE"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey

; Add to PATH
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: add_to_path; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsAddPath(Param: string): Boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Param + ';', ';' + OrigPath + ';') = 0;
end;
