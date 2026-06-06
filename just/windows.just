[script("powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
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
            ForEach-Object { [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName) })
    }

    if ($scriptName -ne '') {
        if (-not $scriptName.EndsWith('.rhai')) {
            $scriptName = "$scriptName.rhai"
        }
        $judges = @((Join-Path $judgeDir $scriptName))
    } elseif (Test-Path -LiteralPath $judgeDir -PathType Container) {
        $judges = @(Get-ChildItem -LiteralPath $judgeDir -File -Filter '*.rhai' |
            Sort-Object FullName |
            ForEach-Object { [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName) })
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

[script("powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
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
            ForEach-Object { [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName) })
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
                ForEach-Object { [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName) })
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

[script("powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
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
            ForEach-Object { [System.IO.Path]::GetRelativePath((Get-Location).Path, $_.FullName) })
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
