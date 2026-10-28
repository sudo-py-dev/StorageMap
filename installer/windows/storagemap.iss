; Inno Setup script for StorageMap.
; Build: iscc /DAppVersion=0.1.3 installer\windows\storagemap.iss
; Installs per user by default (no UAC prompt), like modern TreeSize;
; the user can still choose an all-users install in the dialog.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "StorageMap"
#define AppExe "storagemap.exe"
#define AppUrl "https://github.com/sudo-py-dev/StorageMap"

[Setup]
AppId={{6E8C0B4A-5D1F-4F52-9C3B-7A2E91D4C5F3}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=StorageMap contributors
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile=..\..\assets\storagemap.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=..\..\target\installer
OutputBaseFilename=StorageMap-{#AppVersion}-windows-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
; Updates run while the app is open: let Restart Manager close it.
CloseApplications=force
RestartApplications=no
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "hebrew"; MessagesFile: "compiler:Languages\Hebrew.isl"

[CustomMessages]
english.ContextMenu=Add "Open with StorageMap" to the Explorer context menu
hebrew.ContextMenu=הוספת „פתיחה ב־StorageMap” לתפריט ההקשר של הסייר
english.OpenWith=Open with StorageMap
hebrew.OpenWith=פתיחה ב־StorageMap

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "{cm:ContextMenu}"

[Files]
Source: "..\..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; HKA = HKLM for all-users installs, HKCU for per-user installs.
Root: HKA; Subkey: "Software\Classes\Directory\shell\StorageMap"; ValueType: string; ValueData: "{cm:OpenWith}"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\StorageMap"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#AppExe}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\StorageMap\command"; ValueType: string; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\StorageMap"; ValueType: string; ValueData: "{cm:OpenWith}"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Drive\shell\StorageMap"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#AppExe}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\StorageMap\command"; ValueType: string; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\StorageMap"; ValueType: string; ValueData: "{cm:OpenWith}"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\StorageMap"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#AppExe}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\StorageMap\command"; ValueType: string; ValueData: """{app}\{#AppExe}"" ""%V"""; Tasks: contextmenu

[Run]
; Interactive install: offer to launch. Silent (auto-update): always relaunch.
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: WizardSilent
