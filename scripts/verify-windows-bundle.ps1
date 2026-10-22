param([Parameter(Mandatory = $true)][string]$BundleDirectory)
$ErrorActionPreference = 'Stop'
$installers = @(Get-ChildItem (Join-Path $BundleDirectory 'nsis/*.exe'))
if ($installers.Count -ne 1) { throw 'Expected exactly one Windows installer' }
$destination = Join-Path $env:RUNNER_TEMP ('buggy-vpn-install-' + [guid]::NewGuid().ToString('N'))
$process = Start-Process -FilePath $installers[0].FullName -ArgumentList @('/S', "/D=$destination") -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
$sidecar = Join-Path $destination 'buggy-vpn.exe'
if (!(Test-Path $sidecar)) { throw 'Installed app is missing its VPN engine' }
$savedPath = $env:PATH
try {
    $env:PATH = "$env:SystemRoot\System32"
    & $sidecar --self-test
    if ($LASTEXITCODE -ne 0) { throw 'Packaged VPN engine could not run independently' }
} finally {
    $env:PATH = $savedPath
}
Write-Output 'Windows installer contains a runnable standalone VPN engine.'
