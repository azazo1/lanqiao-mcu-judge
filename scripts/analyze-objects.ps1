param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Sample,

    [Parameter(Mandatory = $false, Position = 1)]
    [string]$Pattern = ""
)

$sampleDir = Join-Path 'sample' $Sample

if (-not (Test-Path -LiteralPath $sampleDir -PathType Container)) {
    [Console]::Error.WriteLine("sample not found: $sampleDir")
    exit 1
}

$runner = Join-Path 'scripts' 'analyze-objects.sh'
if (-not (Get-Command 'bash' -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('bash not found in PATH')
    exit 1
}

if ($Pattern -eq '') {
    & bash $runner $Sample
} else {
    & bash $runner $Sample $Pattern
}

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
