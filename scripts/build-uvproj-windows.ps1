param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Uvproj
)

$ErrorActionPreference = 'Stop'
[System.Text.Encoding]::RegisterProvider([System.Text.CodePagesEncodingProvider]::Instance)
$Utf8OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $Utf8OutputEncoding
$OutputEncoding = $Utf8OutputEncoding

function Resolve-UvprojPath {
    param(
        [string]$InputPath
    )

    $resolved = Resolve-Path -LiteralPath $InputPath -ErrorAction SilentlyContinue
    if ($null -eq $resolved) {
        [Console]::Error.WriteLine("uvproj not found: $InputPath")
        exit 1
    }

    $fullPath = $resolved.Path
    if (-not $fullPath.EndsWith('.uvproj', [System.StringComparison]::OrdinalIgnoreCase)) {
        [Console]::Error.WriteLine("not a uvproj file: $fullPath")
        exit 1
    }

    return $fullPath
}

function Trim-WrappingQuotes {
    param(
        [AllowNull()]
        [string]$Value
    )

    if ($null -eq $Value) {
        return ''
    }

    $trimmed = $Value.Trim()
    if ($trimmed.Length -ge 2 -and $trimmed.StartsWith('"') -and $trimmed.EndsWith('"')) {
        return $trimmed.Substring(1, $trimmed.Length - 2)
    }

    return $trimmed
}

function Normalize-DirectoryPath {
    param(
        [string]$PathValue
    )

    $normalized = $PathValue
    while (
        $normalized.Length -gt 3 -and
        ($normalized.EndsWith('\') -or $normalized.EndsWith('/'))
    ) {
        $normalized = $normalized.Substring(0, $normalized.Length - 1)
    }

    return $normalized
}

function Resolve-ProjectPath {
    param(
        [string]$ProjectDir,
        [string]$RawPath,
        [string]$DefaultChild
    )

    $pathValue = Trim-WrappingQuotes $RawPath
    if ([string]::IsNullOrWhiteSpace($pathValue)) {
        $pathValue = $DefaultChild
    }

    $pathValue = $pathValue.Replace('/', '\')
    if ([System.IO.Path]::IsPathRooted($pathValue)) {
        return Normalize-DirectoryPath ([System.IO.Path]::GetFullPath($pathValue))
    }

    return Normalize-DirectoryPath ([System.IO.Path]::GetFullPath((Join-Path $ProjectDir $pathValue)))
}

function Get-FirstExistingFile {
    param(
        [string[]]$Candidates
    )

    foreach ($candidate in $Candidates) {
        if (
            -not [string]::IsNullOrWhiteSpace($candidate) -and
            (Test-Path -LiteralPath $candidate -PathType Leaf)
        ) {
            return $candidate
        }
    }

    return ''
}

function Get-FirstMatchingFile {
    param(
        [string]$DirPath,
        [string]$Pattern
    )

    if (-not (Test-Path -LiteralPath $DirPath -PathType Container)) {
        return ''
    }

    $match = Get-ChildItem -LiteralPath $DirPath -File -Filter $Pattern |
        Sort-Object FullName |
        Select-Object -First 1
    if ($null -eq $match) {
        return ''
    }

    return $match.FullName
}

function Decode-BytesToString {
    param(
        [byte[]]$Bytes
    )

    $utf8Encoding = [System.Text.UTF8Encoding]::new($false, $true)
    try {
        return $utf8Encoding.GetString($Bytes).TrimStart([char]0xFEFF)
    } catch {
    }

    $normalizedBytes = [byte[]]::new($Bytes.Length)
    [System.Array]::Copy($Bytes, $normalizedBytes, $Bytes.Length)
    for ($i = 0; $i -lt ($normalizedBytes.Length - 1); $i++) {
        if ($normalizedBytes[$i] -eq 0xA6 -and $normalizedBytes[$i + 1] -eq 0xCC) {
            $normalizedBytes[$i] = 0xB5
            $normalizedBytes[$i + 1] = 0x20
        }
    }

    $encodings = @(
        [System.Text.Encoding]::GetEncoding(
            'GB18030',
            [System.Text.EncoderExceptionFallback]::new(),
            [System.Text.DecoderExceptionFallback]::new()
        ),
        [System.Text.Encoding]::GetEncoding(
            1252,
            [System.Text.EncoderExceptionFallback]::new(),
            [System.Text.DecoderExceptionFallback]::new()
        ),
        [System.Text.Encoding]::GetEncoding(
            28591,
            [System.Text.EncoderExceptionFallback]::new(),
            [System.Text.DecoderExceptionFallback]::new()
        )
    )

    $candidateBytesList = @($normalizedBytes, $Bytes)
    foreach ($encoding in $encodings) {
        foreach ($candidateBytes in $candidateBytesList) {
            try {
                return $encoding.GetString($candidateBytes).TrimStart([char]0xFEFF)
            } catch {
            }
        }
    }

    return [System.Text.Encoding]::UTF8.GetString($Bytes).TrimStart([char]0xFEFF)
}

function Get-FileTextUtf8 {
    param(
        [string]$Path
    )

    return Decode-BytesToString ([System.IO.File]::ReadAllBytes($Path))
}

function Get-BuildLogReport {
    param(
        [string]$BuildLogPath,
        [string]$FallbackLogPath
    )

    if (
        -not [string]::IsNullOrWhiteSpace($BuildLogPath) -and
        (Test-Path -LiteralPath $BuildLogPath -PathType Leaf)
    ) {
        $rawText = Get-FileTextUtf8 $BuildLogPath
        $plainText = [System.Net.WebUtility]::HtmlDecode(
            [regex]::Replace($rawText, '<[^>]+>', '')
        )
        $lines = $plainText -split "`r?`n" |
            ForEach-Object { $_.TrimEnd() } |
            Where-Object { $_ -ne '' }
        if ($lines.Count -gt 0) {
            return ($lines -join [Environment]::NewLine)
        }
    }

    if (Test-Path -LiteralPath $FallbackLogPath -PathType Leaf) {
        return (Get-FileTextUtf8 $FallbackLogPath).Trim()
    }

    return ''
}

$uvprojPath = Resolve-UvprojPath $Uvproj
$projectDir = Split-Path -Parent $uvprojPath
$targetName = ''
$outputName = ''
$outputDirectory = ''

try {
    [xml]$project = Get-Content -LiteralPath $uvprojPath -Raw
    $targetName = [string]$project.Project.Targets.Target.TargetName
    $outputName = [string]$project.Project.Targets.Target.TargetOption.TargetCommonOption.OutputName
    $outputDirectory = [string]$project.Project.Targets.Target.TargetOption.TargetCommonOption.OutputDirectory
} catch {
}

if ([string]::IsNullOrWhiteSpace($targetName)) {
    $targetName = [System.IO.Path]::GetFileNameWithoutExtension($uvprojPath)
}

if ([string]::IsNullOrWhiteSpace($outputName)) {
    $outputName = $targetName
}

$outputNameBase = [System.IO.Path]::GetFileName(($outputName -replace '/', '\'))
$objectsDir = Resolve-ProjectPath -ProjectDir $projectDir -RawPath $outputDirectory -DefaultChild 'Objects'
New-Item -ItemType Directory -Path $objectsDir -Force | Out-Null

$logPath = Join-Path $objectsDir 'uv4.log'
$hexCandidates = @(
    (Join-Path $objectsDir ($outputNameBase + '.hex')),
    (Join-Path $objectsDir ($targetName + '.hex'))
)
$buildLogCandidates = @(
    (Join-Path $objectsDir ($outputNameBase + '.build_log.htm')),
    (Join-Path $objectsDir ($targetName + '.build_log.htm'))
)

$uv4 = $null
$uv4Command = Get-Command 'UV4.exe' -ErrorAction SilentlyContinue
if ($null -ne $uv4Command) {
    $uv4 = $uv4Command.Source
} elseif (Test-Path 'C:/Keil_v5/UV4/UV4.exe') {
    $uv4 = 'C:/Keil_v5/UV4/UV4.exe'
} else {
    [Console]::Error.WriteLine('UV4.exe not found in PATH or at C:/Keil_v5/UV4/UV4.exe')
    exit 1
}

Write-Output '==> 构建 uvproj'
Write-Output "uvproj: $uvprojPath"
Write-Output "target: $targetName"
Write-Output "output dir: $objectsDir"
Write-Output "log: $logPath"

$arguments = @(
    '-b', $uvprojPath,
    '-j0',
    '-t', $targetName,
    '-o', $logPath
)

$process = Start-Process -FilePath $uv4 -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
$buildLogPath = Get-FirstExistingFile $buildLogCandidates
if ([string]::IsNullOrWhiteSpace($buildLogPath)) {
    $buildLogPath = Get-FirstMatchingFile -DirPath $objectsDir -Pattern '*.build_log.htm'
}

$hexPath = Get-FirstExistingFile $hexCandidates
if ([string]::IsNullOrWhiteSpace($hexPath)) {
    $hexPath = Get-FirstMatchingFile -DirPath $objectsDir -Pattern '*.hex'
}

$buildHasZeroErrors = $false
$errorProbePath = $buildLogPath
if ([string]::IsNullOrWhiteSpace($errorProbePath)) {
    $errorProbePath = $logPath
}
if (Test-Path -LiteralPath $errorProbePath -PathType Leaf) {
    $buildLogText = Get-FileTextUtf8 $errorProbePath
    if ($buildLogText -match '0 Error\(s\)') {
        $buildHasZeroErrors = $true
    }
}

$buildReport = Get-BuildLogReport -BuildLogPath $buildLogPath -FallbackLogPath $logPath

if (
    $process.ExitCode -ne 0 -and
    -not (
        -not [string]::IsNullOrWhiteSpace($hexPath) -and
        (Test-Path -LiteralPath $hexPath -PathType Leaf) -and
        $buildHasZeroErrors
    )
) {
    if (-not [string]::IsNullOrWhiteSpace($buildLogPath)) {
        [Console]::Error.WriteLine("UV4 exited with code $($process.ExitCode). check build log: $buildLogPath")
    } else {
        [Console]::Error.WriteLine("UV4 exited with code $($process.ExitCode). check log: $logPath")
    }
    if (-not [string]::IsNullOrWhiteSpace($buildReport)) {
        Write-Output 'build log:'
        Write-Output $buildReport
    }
    exit $process.ExitCode
}

if (-not [string]::IsNullOrWhiteSpace($buildReport)) {
    Write-Output 'build log:'
    Write-Output $buildReport
}

if (-not [string]::IsNullOrWhiteSpace($hexPath) -and (Test-Path -LiteralPath $hexPath -PathType Leaf)) {
    Write-Output "hex: $hexPath"
    if ($process.ExitCode -ne 0) {
        Write-Output "UV4 exited with code $($process.ExitCode), but build log reports 0 errors."
    }
} else {
    Write-Output "build finished, hex not found. check log: $logPath"
    exit 1
}
