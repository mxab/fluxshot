# Build the FluxShot setup.exe with Inno Setup. Called by GoReleaser after the
# Windows build (.goreleaser/windows.yaml):
#   pwsh -File packaging/windows/build-installer.ps1 <fluxshot.exe> <version> <out-dir>
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = 'Stop'

$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) {
    throw 'Inno Setup 6 (ISCC.exe) not found. Install it, e.g. `choco install innosetup`.'
}

$exePath = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$outPath = (Resolve-Path $OutDir).Path
$script = Join-Path $PSScriptRoot 'fluxshot.iss'

& $iscc /Q "/DAppVersion=$Version" "/DSourceExe=$exePath" "/DOutputDir=$outPath" $script
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }
Get-ChildItem $outPath
