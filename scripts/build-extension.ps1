$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$extensionRoot = Join-Path $projectRoot 'extension'
$distRoot = Join-Path $projectRoot 'dist'
$output = Join-Path $distRoot 'ily_gfs_minecraftmotd.c7s.zip'

New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
if (Test-Path -LiteralPath $output) {
    Remove-Item -LiteralPath $output -Force
}

Compress-Archive -Path (Join-Path $extensionRoot '*') -DestinationPath $output -CompressionLevel Optimal
$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $output
Write-Output "Created $output"
Write-Output "SHA256 $($hash.Hash.ToLowerInvariant())"

