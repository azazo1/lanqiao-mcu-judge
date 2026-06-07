[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
judge-sample sample script="":
    $sampleDir = Join-Path 'sample' {{quote(sample)}}
    $hexDir = Join-Path $sampleDir 'prj/Objects'
    $judgeDir = Join-Path $sampleDir 'judge'
    $hexCandidates = @()
    $judges = @()
    $scriptName = {{quote(script)}}

    if (Test-Path -LiteralPath $hexDir -PathType Container) {
        $hexCandidates = @(Get-ChildItem -LiteralPath $hexDir -File -Filter '*.hex' |
            Sort-Object FullName |
            ForEach-Object { Resolve-Path -LiteralPath $_.FullName -Relative })
    }

    if ($scriptName -ne '') {
        if (-not $scriptName.EndsWith('.rhai')) {
            $scriptName = "$scriptName.rhai"
        }
        $judges = @((Join-Path $judgeDir $scriptName))
    } elseif (Test-Path -LiteralPath $judgeDir -PathType Container) {
        $judges = @(Get-ChildItem -LiteralPath $judgeDir -File -Filter '*.rhai' |
            Sort-Object FullName |
            ForEach-Object { Resolve-Path -LiteralPath $_.FullName -Relative })
    }

    if ($judges.Count -eq 0) {
        [Console]::Error.WriteLine("no judge scripts found in $judgeDir")
        exit 1
    }

    cargo build --release --bin stcjudge
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $bin = 'target/release/stcjudge.exe'
    foreach ($judge in $judges) {
        Write-Output "==> $judge"
        if ($hexCandidates.Count -eq 1) {
            & $bin run --hex $hexCandidates[0] --script $judge
        } elseif ($hexCandidates.Count -eq 0) {
            [Console]::Error.WriteLine("warning: no hex found in $hexDir, run script without --hex")
            & $bin run --script $judge
        } else {
            [Console]::Error.WriteLine("expected exactly one hex in $hexDir, found $($hexCandidates.Count)")
            exit 1
        }

        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
judge-samples:
    cargo build --release --bin stcjudge
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $bin = 'target/release/stcjudge.exe'
    if (Test-Path -LiteralPath 'sample' -PathType Container) {
        $judges = @(Get-ChildItem -LiteralPath 'sample' -File -Recurse -Filter '*.rhai' |
            Where-Object { $_.Directory.Name -eq 'judge' } |
            Sort-Object FullName |
            ForEach-Object { Resolve-Path -LiteralPath $_.FullName -Relative })
    } else {
        $judges = @()
    }

    foreach ($judge in $judges) {
        $sampleDir = Split-Path -Parent (Split-Path -Parent $judge)
        $hexDir = Join-Path $sampleDir 'prj/Objects'
        $hexCandidates = @()
        if (Test-Path -LiteralPath $hexDir -PathType Container) {
            $hexCandidates = @(Get-ChildItem -LiteralPath $hexDir -File -Filter '*.hex' |
                Sort-Object FullName |
                ForEach-Object { Resolve-Path -LiteralPath $_.FullName -Relative })
        }

        Write-Output "==> $judge"
        if ($hexCandidates.Count -eq 1) {
            & $bin run --hex $hexCandidates[0] --script $judge
        } elseif ($hexCandidates.Count -eq 0) {
            [Console]::Error.WriteLine("warning: no hex found in $hexDir, run script without --hex")
            & $bin run --script $judge
        } else {
            [Console]::Error.WriteLine("expected exactly one hex in $hexDir, found $($hexCandidates.Count)")
            exit 1
        }

        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
wave-sample sample script="smoke" start="0" end="" output="":
    $sampleDir = Join-Path 'sample' {{quote(sample)}}
    $scriptName = {{quote(script)}}
    $waveStart = {{quote(start)}}
    $waveEnd = {{quote(end)}}
    if (-not $scriptName.EndsWith('.rhai')) {
        $scriptName = "$scriptName.rhai"
    }

    $judge = Join-Path (Join-Path $sampleDir 'judge') $scriptName
    $hexDir = Join-Path $sampleDir 'prj/Objects'
    $hexCandidates = @()
    if (Test-Path -LiteralPath $hexDir -PathType Container) {
        $hexCandidates = @(Get-ChildItem -LiteralPath $hexDir -File -Filter '*.hex' |
            Sort-Object FullName |
            ForEach-Object { Resolve-Path -LiteralPath $_.FullName -Relative })
    }

    $outputPath = {{quote(output)}}
    if ($outputPath -eq '') {
        $outputDir = Join-Path (Join-Path 'outputs' 'waves') {{quote(sample)}}
        New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
        $outputPath = Join-Path $outputDir "$([System.IO.Path]::GetFileNameWithoutExtension($judge)).html"
    } else {
        $outputDir = Split-Path -Parent $outputPath
        if ($outputDir -ne '') {
            New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
        }
    }

    cargo build --release --bin stcjudge
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $bin = 'target/release/stcjudge.exe'
    $arguments = @('run', '--script', $judge, '--wave-start', $waveStart, '--wave-html', $outputPath)
    if ($hexCandidates.Count -eq 1) {
        $arguments = @('run', '--hex', $hexCandidates[0], '--script', $judge, '--wave-start', $waveStart, '--wave-html', $outputPath)
    } elseif ($hexCandidates.Count -eq 0) {
        [Console]::Error.WriteLine("warning: no hex found in $hexDir, run script without --hex")
    } else {
        [Console]::Error.WriteLine("expected exactly one hex in $hexDir, found $($hexCandidates.Count)")
        exit 1
    }

    if ($waveEnd -ne '') {
        $arguments += @('--wave-end', $waveEnd)
    }

    & $bin @arguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    Write-Output "wave html: $outputPath"

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
build-sample sample:
    $sampleDir = Join-Path 'sample' {{quote(sample)}}
    $prjDir = Join-Path $sampleDir 'prj'
    if (-not (Test-Path -LiteralPath $prjDir -PathType Container)) {
        [Console]::Error.WriteLine("project directory not found: $prjDir")
        exit 1
    }

    $uvproj = Get-ChildItem -LiteralPath $prjDir -File -Filter '*.uvproj' |
        Sort-Object Name |
        Select-Object -First 1
    if ($null -eq $uvproj) {
        [Console]::Error.WriteLine("no uvproj found in $prjDir")
        exit 1
    }

    $targetName = ''
    $outputName = ''
    try {
        [xml]$project = Get-Content -LiteralPath $uvproj.FullName
        $targetName = [string]$project.Project.Targets.Target.TargetName
        $outputName = [string]$project.Project.Targets.Target.TargetOption.TargetCommonOption.OutputName
    } catch {
    }

    if ([string]::IsNullOrWhiteSpace($targetName)) {
        $targetName = [System.IO.Path]::GetFileNameWithoutExtension($uvproj.Name)
    }
    if ([string]::IsNullOrWhiteSpace($outputName)) {
        $outputName = $targetName
    }

    $objectsDir = Join-Path $prjDir 'Objects'
    New-Item -ItemType Directory -Path $objectsDir -Force | Out-Null
    $logPath = Join-Path $objectsDir 'uv4.log'
    $buildLogCandidates = @(
        (Join-Path $objectsDir ($outputName + '.build_log.htm')),
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

    $arguments = @(
        '-b', $uvproj.FullName,
        '-j0',
        '-t', $targetName,
        '-o', $logPath
    )

    function Get-BuildLogReport {
        param(
            [string]$BuildLogPath,
            [string]$FallbackLogPath
        )

        if (
            -not [string]::IsNullOrWhiteSpace($BuildLogPath) -and
            (Test-Path -LiteralPath $BuildLogPath -PathType Leaf)
        ) {
            $rawText = Get-Content -LiteralPath $BuildLogPath -Raw
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
            return (Get-Content -LiteralPath $FallbackLogPath -Raw).Trim()
        }

        return ''
    }

    $process = Start-Process -FilePath $uv4 -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
    $hexPath = Join-Path $objectsDir ($outputName + '.hex')
    $buildLogPath = $buildLogCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1

    $buildHasZeroErrors = $false
    if ($null -ne $buildLogPath) {
        $buildLogText = Get-Content -LiteralPath $buildLogPath -Raw
        if ($buildLogText -match '0 Error\(s\)') {
            $buildHasZeroErrors = $true
        }
    }
    $buildReport = Get-BuildLogReport -BuildLogPath $buildLogPath -FallbackLogPath $logPath

    if (
        $process.ExitCode -ne 0 -and
        -not (
            (Test-Path -LiteralPath $hexPath -PathType Leaf) -and
            $buildHasZeroErrors
        )
    ) {
        if ($null -ne $buildLogPath) {
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

    if (Test-Path -LiteralPath $hexPath -PathType Leaf) {
        if ($process.ExitCode -ne 0) {
            Write-Output "hex: $hexPath"
            Write-Output "UV4 exited with code $($process.ExitCode), but build log reports 0 errors."
        } else {
            Write-Output "hex: $hexPath"
        }
    } else {
        Write-Output "build finished, hex not found. check log: $logPath"
        exit 1
    }

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
analyze-objects sample pattern="":
    $patternValue = {{quote(pattern)}}
    if ($patternValue -eq '') {
        & pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/analyze-objects.ps1 {{quote(sample)}}
    } else {
        & pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/analyze-objects.ps1 {{quote(sample)}} $patternValue
    }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
