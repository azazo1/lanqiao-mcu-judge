$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

# 输出当前构建应该展示的版本号, 与 scripts/build-version.sh 行为一致.

$PackageName = "stcjudge"
$TagPrefix = "v"

$root = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $root
try {

function Get-GitOutput {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$GitArgs
    )

    $output = & git @GitArgs 2>$null
    if ($LASTEXITCODE -ne 0) {
        return $null
    }

    $text = (($output | Out-String) -replace "`r", "").Trim()
    if ([string]::IsNullOrWhiteSpace($text)) {
        return $null
    }

    return $text
}

function Select-VersionTag {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Tags
    )

    $lines = $Tags -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
    if ($TagPrefix) {
        $prefixed = $lines | Where-Object { $_.StartsWith($TagPrefix) } | Select-Object -First 1
        if ($prefixed) {
            return $prefixed
        }
    }

    return $lines | Select-Object -First 1
}

# 用该生态的结构化 metadata 读取稳定包版本, 不要正则扫清单文件.
function Read-PackageVersion {
    $metadata = cargo metadata --locked --no-deps --format-version 1 | Out-String | ConvertFrom-Json
    $packageVersion = $metadata.packages |
        Where-Object { $_.name -eq $PackageName } |
        Select-Object -First 1 -ExpandProperty version
    if (-not $packageVersion) {
        throw "failed to resolve $PackageName version from cargo metadata"
    }
    return $packageVersion
}

function Get-LatestDescribedTag {
    if ($TagPrefix) {
        return Get-GitOutput -GitArgs @("describe", "--tags", "--abbrev=0", "--match", "$TagPrefix*", "HEAD")
    }
    return Get-GitOutput -GitArgs @("describe", "--tags", "--abbrev=0", "HEAD")
}

function Strip-TagPrefix {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Display
    )

    if ($TagPrefix -and $Display.StartsWith($TagPrefix)) {
        return $Display.Substring($TagPrefix.Length)
    }
    return $Display
}

$fallbackTag = "$TagPrefix$(Read-PackageVersion)"
$exactTag = $null
$tags = Get-GitOutput -GitArgs @("tag", "--points-at", "HEAD")
if ($tags) {
    $exactTag = Select-VersionTag -Tags $tags
}

if ($exactTag) {
    $tag = $exactTag
} else {
    $described = Get-LatestDescribedTag
    if ($described) {
        $tag = $described
    } else {
        $tag = $fallbackTag
    }
}

$commit = Get-GitOutput -GitArgs @("rev-parse", "--short=7", "HEAD")
$dirty = $false
if ($commit) {
    & git diff-index --quiet HEAD -- | Out-Null
    if ($LASTEXITCODE -eq 1) {
        $dirty = $true
    }
}

if (-not $commit) {
    $display = $tag
} elseif ($dirty) {
    $display = "$tag^$commit"
} elseif ($exactTag) {
    $display = $tag
} else {
    $display = "$tag-$commit"
}

Write-Output (Strip-TagPrefix -Display $display)
} finally {
    Pop-Location
}
