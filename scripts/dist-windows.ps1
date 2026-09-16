# 生成 windows 发布产物: CLI 与 GUI 的 zip.
#
# 输入环境变量同 scripts/dist-linux.sh.

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

$versionRaw = $env:PROJECT_BUILD_VERSION
if (-not $versionRaw) {
    throw '未设置 PROJECT_BUILD_VERSION, 请通过 just dist 调用'
}
$version = $versionRaw.TrimStart('v')
$target = $env:PROJECT_DIST_TARGET
$outDir = if ($env:PROJECT_DIST_OUT) { $env:PROJECT_DIST_OUT } else { 'dist' }
$fakeSuffix = if ($env:PROJECT_DIST_FAKE) { '-fake' } else { '' }

# 把一个文件列表打成 $outDir 下的 zip, 每个包使用独立的临时目录.
function New-Package {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string[]]$Files
    )

    $packageDir = Join-Path ([System.IO.Path]::GetTempPath()) ("stcjudge-dist-" + [guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
    try {
        foreach ($file in $Files) {
            if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
                throw "缺少打包文件: $file"
            }
            Copy-Item -LiteralPath $file -Destination $packageDir
        }
        Compress-Archive -Path (Join-Path $packageDir '*') -DestinationPath (Join-Path $outDir $Name) -Force
    } finally {
        Remove-Item -LiteralPath $packageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$root = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $root
try {
    $platformArch = switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'x86_64' }
        'ARM64' { 'aarch64' }
        default { throw "不支持的构建架构: $($env:PROCESSOR_ARCHITECTURE)" }
    }

    $releaseDir = 'target/release'
    $buildArgs = @('build', '--release', '--workspace', '--bins')
    if ($target) {
        $releaseDir = "target/$target/release"
        $buildArgs += @('--target', $target)
    }

    Write-Output "构建 $versionRaw ($platformArch), 输出到 $releaseDir"
    $env:PROJECT_BUILD_VERSION = $versionRaw
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo build 失败'
    }

    $bins = @('stcjudge.exe', 'stcjudge-gui.exe', 'analyze-objects.exe')
    foreach ($bin in $bins) {
        $path = Join-Path $releaseDir $bin
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "缺少可执行文件: $path"
        }
        $header = [System.IO.File]::ReadAllBytes($path)
        if ($header.Length -lt 2 -or $header[0] -ne 0x4d -or $header[1] -ne 0x5a) {
            throw "PE 文件头无效: $path"
        }
    }

    $cliVersion = & (Join-Path $releaseDir 'stcjudge.exe') '--version'
    if ($LASTEXITCODE -ne 0) {
        throw 'stcjudge --version 执行失败'
    }
    if ($cliVersion -notmatch [regex]::Escape($versionRaw)) {
        throw "stcjudge --version 输出与构建版本不一致: $cliVersion"
    }

    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $cliName = "stcjudge-$version-windows-$platformArch$fakeSuffix.zip"
    $guiName = "stcjudge-gui-$version-windows-$platformArch$fakeSuffix.zip"

    New-Package -Name $cliName -Files @(
        (Join-Path $releaseDir 'stcjudge.exe'),
        (Join-Path $releaseDir 'analyze-objects.exe'),
        'LICENSE',
        'README.md'
    )
    New-Package -Name $guiName -Files @(
        (Join-Path $releaseDir 'stcjudge-gui.exe'),
        'LICENSE'
    )

    Write-Output "已生成 $outDir/$cliName"
    Write-Output "已生成 $outDir/$guiName"
} finally {
    Pop-Location
}
