param([Parameter(Mandatory=$true)][string]$FilePath)
$ErrorActionPreference = 'Stop'
& artifact-signing-cli -e $env:AZURE_SIGNING_ENDPOINT -a $env:AZURE_SIGNING_ACCOUNT -c $env:AZURE_SIGNING_PROFILE -d 'Table Por' $FilePath
if ($LASTEXITCODE -ne 0) { throw 'Azure Artifact Signing failed' }
