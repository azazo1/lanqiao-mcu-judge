[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
judge-sample sample script="":
    $sampleDir = Join-Path 'samples' {{quote(sample)}}
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

    cargo build --release -p stcjudge --bin stcjudge
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
    cargo build --release -p stcjudge --bin stcjudge
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $bin = 'target/release/stcjudge.exe'
    if (Test-Path -LiteralPath 'samples' -PathType Container) {
        $judges = @(Get-ChildItem -LiteralPath 'samples' -File -Recurse -Filter '*.rhai' |
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
    $sampleDir = Join-Path 'samples' {{quote(sample)}}
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

    cargo build --release -p stcjudge --bin stcjudge
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
    $sampleDir = Join-Path 'samples' {{quote(sample)}}
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

    & 'scripts/build-uvproj-windows.ps1' -Uvproj $uvproj.FullName
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
build-uvproj uvproj:
    & 'scripts/build-uvproj-windows.ps1' -Uvproj {{quote(uvproj)}}
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
analyze-objects sample pattern="":
    cargo build --quiet --release -p stcjudge --bin analyze-objects
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $bin = 'target/release/analyze-objects.exe'
    $patternValue = {{quote(pattern)}}
    if ($patternValue -eq '') {
        & $bin {{quote(sample)}}
    } else {
        & $bin {{quote(sample)}} $patternValue
    }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
