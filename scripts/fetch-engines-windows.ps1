# RecovMe — Copyright © 2026 Dhan Denvert Asuncion
# SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
# Downloads the official Windows builds of the two recovery engines into
# src-tauri/resources/engines so the installer carries them.
#   PhotoRec 7.2 (CGSecurity, 64-bit)   The Sleuth Kit (tsk_recover)
#   adb (Android platform tools, for Android phones)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'src-tauri/resources/engines'
$tmp = Join-Path ($env:RUNNER_TEMP ?? $env:TEMP) 'recovme-engines'
$testdiskVersion = $env:TESTDISK_WIN_VERSION ?? '7.2'
$tskVersion = $env:TSK_VERSION ?? '4.15.0'

Remove-Item -Recurse -Force $tmp, (Join-Path $out 'photorec'), (Join-Path $out 'tsk'), (Join-Path $out 'adb') -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $tmp, (Join-Path $out 'photorec'), (Join-Path $out 'tsk'), (Join-Path $out 'adb') | Out-Null

Write-Host "── PhotoRec $testdiskVersion ──"
$zip = Join-Path $tmp 'testdisk.zip'
$urls = @(
  "https://www.cgsecurity.org/testdisk-$testdiskVersion.win64.zip",
  "https://www.cgsecurity.org/Download_and_donate.php/testdisk-$testdiskVersion.win64.zip"
)
$ok = $false
foreach ($u in $urls) {
  try {
    Invoke-WebRequest -Uri $u -OutFile $zip -UseBasicParsing
    if ((Get-Item $zip).Length -gt 1MB) { $ok = $true; break }
  } catch { Write-Host "  $u failed: $_" }
}
if (-not $ok) { throw 'Could not download PhotoRec for Windows' }
Expand-Archive $zip (Join-Path $tmp 'testdisk') -Force
$exe = Get-ChildItem (Join-Path $tmp 'testdisk') -Recurse -Filter 'photorec_win.exe' | Select-Object -First 1
if (-not $exe) { throw 'photorec_win.exe not found in the download' }
# Keep PhotoRec and its Cygwin runtime; leave out the Qt GUI (large, unused).
Get-ChildItem $exe.DirectoryName | Where-Object {
  $_.Name -notmatch '^(qphotorec|Qt\d|Qt6|Qt5|platforms|imageformats|styles|iconengines|translations)'
} | ForEach-Object { Copy-Item $_.FullName (Join-Path $out 'photorec') -Recurse -Force }

Write-Host "── The Sleuth Kit $tskVersion ──"
$tzip = Join-Path $tmp 'tsk.zip'
Invoke-WebRequest -Uri "https://github.com/sleuthkit/sleuthkit/releases/download/sleuthkit-$tskVersion/sleuthkit-$tskVersion-win32.zip" -OutFile $tzip -UseBasicParsing
Expand-Archive $tzip (Join-Path $tmp 'tsk') -Force
$bin = Get-ChildItem (Join-Path $tmp 'tsk') -Recurse -Filter 'tsk_recover.exe' | Select-Object -First 1
if (-not $bin) { throw 'tsk_recover.exe not found in the download' }
# Only tsk_recover and the DLLs it loads; the other TSK tools aren't used.
Copy-Item $bin.FullName (Join-Path $out 'tsk') -Force
Get-ChildItem $bin.DirectoryName -Filter '*.dll' | ForEach-Object { Copy-Item $_.FullName (Join-Path $out 'tsk') -Force }
$lic = Get-ChildItem (Join-Path $tmp 'tsk') -Recurse -Directory -Filter 'licenses' | Select-Object -First 1
if ($lic) { Copy-Item $lic.FullName (Join-Path $out 'tsk/licenses') -Recurse -Force }

Write-Host '── Android platform tools (adb) ──'
$pzip = Join-Path $tmp 'platform-tools.zip'
Invoke-WebRequest -Uri 'https://dl.google.com/android/repository/platform-tools-latest-windows.zip' -OutFile $pzip -UseBasicParsing
Expand-Archive $pzip (Join-Path $tmp 'pt') -Force
# adb and the two USB DLLs it loads; the other platform tools aren't used.
foreach ($f in 'adb.exe', 'AdbWinApi.dll', 'AdbWinUsbApi.dll', 'NOTICE.txt') {
  $src = Join-Path $tmp "pt/platform-tools/$f"
  if (Test-Path $src) { Copy-Item $src (Join-Path $out 'adb') -Force }
  elseif ($f -ne 'NOTICE.txt') { throw "$f not found in the platform tools download" }
}

Write-Host '── Smoke test ──'
$v = & (Join-Path $out 'photorec/photorec_win.exe') /version 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $v -notmatch 'PhotoRec') { throw "PhotoRec did not start: $v" }
$v.Split("`n") | Select-Object -First 3 | ForEach-Object { Write-Host $_ }
$t = & (Join-Path $out 'tsk/tsk_recover.exe') -V 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $t -notmatch 'Sleuth') { throw "tsk_recover did not start: $t" }
Write-Host $t
$a = & (Join-Path $out 'adb/adb.exe') version 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $a -notmatch 'Android Debug Bridge') { throw "adb did not start: $a" }
Write-Host ($a.Split("`n") | Select-Object -First 1)
Get-ChildItem -Recurse $out | Measure-Object -Property Length -Sum | ForEach-Object { "Engines: {0:N1} MB" -f ($_.Sum / 1MB) }
