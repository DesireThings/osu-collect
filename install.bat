@echo off
setlocal enableextensions disabledelayedexpansion

:: install.bat -- installs or updates osu-collect from the latest GitHub release
:: requires: Windows 10+ (PowerShell 5.1+ and curl.exe built-in, both preinstalled)
::
:: Delayed expansion stays off: with it on, cmd drops every "!" from a line, so a
:: user folder with a "!" in its name broke every path. Paths reach PowerShell
:: through environment variables ($env:NAME), never pasted into its code, so a
:: quote in a folder name cannot break a command. Paths in messages printed
:: inside ( ) blocks are quoted, so a ")" in a folder name cannot end the block.

set "REPO=DesireThings/osu-collect"
set "API_URL=https://api.github.com/repos/%REPO%/releases/latest"
set "ASSET_NAME=osu-collect-windows-x64.exe"
set "INSTALL_DIR=%LOCALAPPDATA%\Programs\osu-collect"
set "INSTALL_BIN=%INSTALL_DIR%\osu-collect.exe"
set "TMPDIR=%TEMP%\osu-collect-install-%RANDOM%"

:: -- fetch release metadata (one request, with retry) ------------------------

echo ==^> fetching latest release info...

:: fetch once and parse locally -- curl --retry rides out the transient 5xx /
:: timeout the GitHub API occasionally returns; three separate live calls did not
set "RELJSON=%TEMP%\osu-collect-release-%RANDOM%.json"
curl.exe -fsSL --retry 5 -A "osu-collect-installer" -o "%RELJSON%" "%API_URL%"
if errorlevel 1 (
  echo error: could not reach the GitHub API ^(try again in a minute^)
  del /f /q "%RELJSON%" 2>nul
  exit /b 1
)

:: NULL sentinels keep all three fields non-empty so for /f does not collapse the
:: "|" delimiters when an asset is missing. The sha256 comes from GitHub's inline
:: per-asset "digest" ("sha256:<hex>"), so no separate checksum request is needed.
for /f "tokens=1,2,3 delims=|" %%a in ('powershell -NoProfile -Command ^
  "$r = Get-Content -LiteralPath $env:RELJSON -Raw | ConvertFrom-Json; " ^
  "$t = $r.tag_name; if (-not $t) { $t = 'NULL' }; " ^
  "$a = $r.assets | Where-Object { $_.name -eq $env:ASSET_NAME }; " ^
  "$d = $a.browser_download_url; if (-not $d) { $d = 'NULL' }; " ^
  "$h = $a.digest; if ($h -match '^sha256:[0-9a-fA-F]{64}$') { $h = $h.Substring(7).ToLower() } else { $h = 'NULL' }; " ^
  "Write-Output ($t + '|' + $d + '|' + $h)"') do (
  set "LATEST_TAG=%%a"
  set "DOWNLOAD_URL=%%b"
  set "REMOTE_HASH=%%c"
)
del /f /q "%RELJSON%" 2>nul

if "%LATEST_TAG%"=="" (
  echo error: could not read release info
  exit /b 1
)
if "%LATEST_TAG%"=="NULL" (
  echo error: latest release has no tag
  exit /b 1
)
if "%DOWNLOAD_URL%"=="NULL" (
  echo error: asset '%ASSET_NAME%' not found in release %LATEST_TAG%
  exit /b 1
)
if "%REMOTE_HASH%"=="NULL" (
  echo error: asset '%ASSET_NAME%' has no sha256 digest in release %LATEST_TAG%
  exit /b 1
)

echo ==^> latest release: %LATEST_TAG%

:: -- create temp dir ----------------------------------------------------------

mkdir "%TMPDIR%" 2>nul
if errorlevel 1 (
  echo error: could not create temp directory "%TMPDIR%"
  exit /b 1
)

set "TMP_BIN=%TMPDIR%\%ASSET_NAME%"

:: REMOTE_HASH already came from the release JSON's per-asset digest above.

:: -- idempotency check --------------------------------------------------------

:: REMOTE_HASH is already lowercase hex (checked by the release-info step), so
:: the two hashes compare as plain strings
set "CURRENT_HASH="
if exist "%INSTALL_BIN%" (
  for /f "delims=" %%H in ('powershell -NoProfile -Command ^
    "(Get-FileHash -LiteralPath $env:INSTALL_BIN -Algorithm SHA256).Hash.ToLower()"') do (
    set "CURRENT_HASH=%%H"
  )
)

:: an up-to-date install still runs the PATH, shortcut and uninstaller steps, so
:: a rerun repairs any of them that went missing
if "%CURRENT_HASH%"=="%REMOTE_HASH%" (
  echo ==^> already up to date ^(%LATEST_TAG%^)
  rmdir /s /q "%TMPDIR%" 2>nul
  goto :add_path
)

:: -- download binary ----------------------------------------------------------

echo ==^> downloading osu-collect %LATEST_TAG%...
curl.exe -fsSL --retry 3 -o "%TMP_BIN%" "%DOWNLOAD_URL%"
if errorlevel 1 (
  echo error: download failed
  rmdir /s /q "%TMPDIR%" 2>nul
  exit /b 1
)

:: -- verify sha256 ------------------------------------------------------------

echo ==^> verifying checksum...
set "ACTUAL_HASH="
for /f "delims=" %%H in ('powershell -NoProfile -Command ^
  "(Get-FileHash -LiteralPath $env:TMP_BIN -Algorithm SHA256).Hash.ToLower()"') do (
  set "ACTUAL_HASH=%%H"
)

if not "%ACTUAL_HASH%"=="%REMOTE_HASH%" (
  echo error: sha256 mismatch
  echo   expected: %REMOTE_HASH%
  echo   actual:   %ACTUAL_HASH%
  del /f /q "%TMP_BIN%" 2>nul
  rmdir /s /q "%TMPDIR%" 2>nul
  exit /b 1
)

:: -- install binary -----------------------------------------------------------

if not exist "%INSTALL_DIR%" (
  mkdir "%INSTALL_DIR%"
  if errorlevel 1 (
    echo error: could not create install directory "%INSTALL_DIR%"
    rmdir /s /q "%TMPDIR%" 2>nul
    exit /b 1
  )
)

copy /y "%TMP_BIN%" "%INSTALL_BIN%" >nul
if errorlevel 1 (
  echo error: could not copy binary to "%INSTALL_BIN%"
  rmdir /s /q "%TMPDIR%" 2>nul
  exit /b 1
)
echo ==^> installed to %INSTALL_BIN%

:: -- add to user PATH if needed -----------------------------------------------

:: the steps from here on only print warnings: the binary is installed and runs,
:: so a failure here doesn't make the install fail
:add_path
:: edit the user PATH in the registry (not the process PATH) instead of with
:: setx, which cuts values off at 1024 characters. %VAR% entries stay unexpanded
:: and the value keeps its registry type. INSTALL_DIR reaches PowerShell through
:: the environment, so quotes in the path cannot break the command. Setting and
:: clearing a throwaway user variable makes Windows tell running programs that
:: the environment changed, as setx did. exit 2 = already in PATH
powershell -NoProfile -Command ^
  "try { " ^
  "  $dir = $env:INSTALL_DIR.TrimEnd('\'); " ^
  "  $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); " ^
  "  $cur = [string]$key.GetValue('Path', '', 'DoNotExpandEnvironmentNames'); " ^
  "  $parts = @($cur -split ';' | Where-Object { $_ }); " ^
  "  if ($parts | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -eq $dir }) { exit 2 } " ^
  "  $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString; " ^
  "  if ($key.GetValueNames() -contains 'Path') { $kind = $key.GetValueKind('Path') } " ^
  "  $key.SetValue('Path', (($parts + $dir) -join ';'), $kind); " ^
  "  [Environment]::SetEnvironmentVariable('OSU_COLLECT_PATH_REFRESH', '1', 'User'); " ^
  "  [Environment]::SetEnvironmentVariable('OSU_COLLECT_PATH_REFRESH', $null, 'User'); " ^
  "} catch { exit 1 }"
if errorlevel 2 (
  echo ==^> "%INSTALL_DIR%" already in user PATH
) else if errorlevel 1 (
  echo warning: could not update user PATH
) else (
  echo ==^> added "%INSTALL_DIR%" to user PATH
  echo     restart your terminal for PATH changes to take effect
)

:: -- shortcuts ----------------------------------------------------------------

echo ==^> creating shortcuts...
:: if Windows Terminal is present, launch through it so the app opens in wt instead
:: of the legacy console; the wt.exe alias is a 0-byte stub with no icon, so resolve
:: the real WindowsTerminal.exe from the package and set it as the shortcut icon
powershell -NoProfile -Command ^
  "$ws = New-Object -ComObject WScript.Shell; " ^
  "$q = [char]34; " ^
  "$bang = [char]33; " ^
  "$name = 'osu-collect (terminal).lnk'; " ^
  "$wt = [System.IO.Path]::Combine($env:LOCALAPPDATA, 'Microsoft\WindowsApps\wt.exe'); " ^
  "$useWt = Test-Path $wt; " ^
  "$icon = ''; " ^
  "$pkg = Get-AppxPackage -Name Microsoft.WindowsTerminal | Select-Object -First 1; " ^
  "if ($pkg) { $wtExe = Join-Path $pkg.InstallLocation 'WindowsTerminal.exe'; if (Test-Path $wtExe) { $icon = $wtExe + ',0'; } } " ^
  "$paths = @( " ^
  "  [System.Environment]::GetFolderPath('Desktop'), " ^
  "  [System.IO.Path]::Combine($env:APPDATA, 'Microsoft\Windows\Start Menu\Programs') " ^
  "); " ^
  "foreach ($dir in $paths) { " ^
  "  Remove-Item -LiteralPath ([System.IO.Path]::Combine($dir, 'osu' + $bang + 'collect.lnk')) -Force -ErrorAction SilentlyContinue; " ^
  "  $lnk = $ws.CreateShortcut([System.IO.Path]::Combine($dir, $name)); " ^
  "  if ($useWt) { " ^
  "    $lnk.TargetPath = $wt; " ^
  "    $lnk.Arguments = $q + $env:INSTALL_BIN + $q; " ^
  "  } else { " ^
  "    $lnk.TargetPath = $env:INSTALL_BIN; " ^
  "  } " ^
  "  if ($icon) { $lnk.IconLocation = $icon; } " ^
  "  $lnk.WorkingDirectory = $env:USERPROFILE; " ^
  "  $lnk.Description = 'osu-collect terminal app'; " ^
  "  $lnk.Save() " ^
  "}"
if errorlevel 1 (
  echo warning: could not create shortcuts
) else (
  echo ==^> shortcuts created in Desktop and Start Menu
)

:: -- register uninstaller -----------------------------------------------------

echo ==^> registering uninstaller...

:: 1. extract the uninstaller payload (plain PowerShell at the bottom of this file)
::    into the install dir. kept separate from the registry writes below so a
::    throw here cannot abort registration
set "INSTALLER_SELF=%~f0"
powershell -NoProfile -Command ^
  "try { " ^
  "  $marker = '# ::OSU-COLLECT' + '-UNINSTALLER::'; " ^
  "  $txt = Get-Content -LiteralPath $env:INSTALLER_SELF -Raw; " ^
  "  $i = $txt.IndexOf($marker); " ^
  "  if ($i -ge 0) { Set-Content -LiteralPath (Join-Path $env:INSTALL_DIR 'uninstall.ps1') -Value $txt.Substring($i + $marker.Length).TrimStart() -Encoding ASCII; } " ^
  "} catch { exit 1 }"

if not exist "%INSTALL_DIR%\uninstall.ps1" (
  echo warning: could not write uninstall.ps1 -- skipping uninstaller registration
  goto :after_register
)

:: 2. register under HKCU with reg.exe (native, reliable). The key and the
::    shortcut names say "terminal": the desktop app's installer registers
::    "osu-collect" and makes osu-collect.lnk, and the two must not overwrite
::    or uninstall each other.
set "UNKEY=HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\osu-collect-terminal"
set "UNVER=%LATEST_TAG%"
if "%UNVER:~0,1%"=="v" set "UNVER=%UNVER:~1%"
reg add "%UNKEY%" /v DisplayName          /t REG_SZ   /d "osu-collect (terminal app)" /f >nul
reg add "%UNKEY%" /v DisplayVersion        /t REG_SZ   /d "%UNVER%" /f >nul
reg add "%UNKEY%" /v DisplayIcon           /t REG_SZ   /d "%INSTALL_BIN%" /f >nul
reg add "%UNKEY%" /v Publisher             /t REG_SZ   /d "DesireThings" /f >nul
reg add "%UNKEY%" /v InstallLocation       /t REG_SZ   /d "%INSTALL_DIR%" /f >nul
reg add "%UNKEY%" /v URLInfoAbout          /t REG_SZ   /d "https://github.com/DesireThings/osu-collect" /f >nul
reg add "%UNKEY%" /v UninstallString       /t REG_SZ   /d "powershell -NoProfile -ExecutionPolicy Bypass -File \"%INSTALL_DIR%\uninstall.ps1\"" /f >nul
reg add "%UNKEY%" /v QuietUninstallString  /t REG_SZ   /d "powershell -NoProfile -ExecutionPolicy Bypass -File \"%INSTALL_DIR%\uninstall.ps1\"" /f >nul
reg add "%UNKEY%" /v NoModify              /t REG_DWORD /d 1 /f >nul
reg add "%UNKEY%" /v NoRepair              /t REG_DWORD /d 1 /f >nul

reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\osu-collect-terminal" /v UninstallString >nul 2>&1
if errorlevel 1 (
  echo warning: could not register uninstaller in the Installed apps list
) else (
  echo ==^> uninstaller registered; see Settings - Apps - Installed apps
)

:after_register

:: -- cleanup ------------------------------------------------------------------

rmdir /s /q "%TMPDIR%" 2>nul
echo ==^> done -- open a new terminal and run 'osu-collect' to start
endlocal
exit /b 0

:: Everything below runs only when extracted to uninstall.ps1 (PowerShell), never
:: by cmd -- it sits past `exit /b 0`. The installer copies it verbatim into the
:: install dir and registers it as the Windows uninstaller.
# ::OSU-COLLECT-UNINSTALLER::
$ErrorActionPreference = 'SilentlyContinue'
Write-Host ''
Write-Host 'Uninstalling osu-collect ...'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\osu-collect'
$desktop    = [Environment]::GetFolderPath('Desktop')
$startMenu  = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
# only the terminal app's own shortcuts: osu-collect.lnk is the desktop app's
$shortcuts  = @(
    Join-Path $desktop   'osu-collect (terminal).lnk'
    Join-Path $startMenu 'osu-collect (terminal).lnk'
)
foreach ($lnk in $shortcuts) { Remove-Item -LiteralPath $lnk -Force }

# strip the install dir from the user PATH; read and write the registry value
# directly so other %VAR% entries stay unexpanded and the value keeps its type
$envKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
if ($envKey -and ($envKey.GetValueNames() -contains 'Path')) {
    $userPath = [string]$envKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    $kept = @($userPath -split ';' | Where-Object { $_ -and ([Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ne $installDir) })
    $envKey.SetValue('Path', ($kept -join ';'), $envKey.GetValueKind('Path'))
    # setting and clearing a throwaway variable tells running programs about the change
    [Environment]::SetEnvironmentVariable('OSU_COLLECT_PATH_REFRESH', '1', 'User')
    [Environment]::SetEnvironmentVariable('OSU_COLLECT_PATH_REFRESH', $null, 'User')
}

# drop the "Installed apps" registry entry
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\osu-collect-terminal' -Recurse -Force

Write-Host 'osu-collect has been uninstalled.'
Start-Sleep -Seconds 2

# remove the install dir (incl. this script) from a detached process so the file
# isn't locked while it deletes itself; the delay lets this script exit first
Start-Process cmd -WindowStyle Hidden -ArgumentList '/c', ('timeout /t 3 > nul & rmdir /s /q "' + $installDir + '"')
