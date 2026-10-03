param(
    [int]$Rounds = 3,
    [int]$SamplesPerRound = 5,
    [int]$Warmup = 5,
    [int]$Iterations = 1000,
    [int]$QueryRepetitions = 20,
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
Set-Location $PSScriptRoot

if ($Rounds -lt 1 -or $SamplesPerRound -lt 2 -or $Warmup -lt 0 -or
    $Iterations -lt 1 -or $QueryRepetitions -lt 1) {
    throw "Rounds, samples, iterations, and query repetitions must be positive; use at least two samples per round."
}

if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $PSScriptRoot ("results\run-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$OutputDirectory = (Resolve-Path $OutputDirectory).Path
$rawPath = Join-Path $OutputDirectory "raw.txt"
$metadataPath = Join-Path $OutputDirectory "metadata.json"
$manifest = Join-Path $PSScriptRoot "Cargo.toml"
$lockFile = Join-Path $PSScriptRoot "Cargo.lock"
$executable = Join-Path $PSScriptRoot "target\release\rkv98-adapter-gate.exe"

cargo build --release --manifest-path $manifest --locked --features adapter
if ($LASTEXITCODE -ne 0) {
    throw "The isolated adapter-gate benchmark did not build (exit $LASTEXITCODE)."
}

$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$computer = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
$powerScheme = (powercfg /getactivescheme | Out-String).Trim()
$metadata = [ordered]@{
    recorded_at_local = (Get-Date).ToString("o")
    repository = "Wouittone/orskit"
    source_commit = (& git -C $repoRoot rev-parse HEAD).Trim()
    working_tree_dirty = [bool](@(& git -C $repoRoot status --porcelain).Count)
    cargo_lock_sha256 = (Get-FileHash $lockFile -Algorithm SHA256).Hash.ToLowerInvariant()
    command = "pwsh .agent/references/rkv98-adapter-gate/run.ps1 -Rounds $Rounds -SamplesPerRound $SamplesPerRound -Warmup $Warmup -Iterations $Iterations -QueryRepetitions $QueryRepetitions"
    rounds = $Rounds
    samples_per_round = $SamplesPerRound
    warmup_propagations = $Warmup
    measured_propagations_per_sample = $Iterations
    dense_query_repetitions_per_sample = $QueryRepetitions
    solver_versions = @{
        differential_equations_rs = "1.4.1"
        numeris = "0.6.0"
        stats_alloc = "0.1.10"
    }
    host = @{
        os = "$($os.Caption) $($os.Version) build $($os.BuildNumber)"
        architecture = $env:PROCESSOR_ARCHITECTURE
        cpu = $cpu.Name
        cores = $cpu.NumberOfCores
        logical_processors = $cpu.NumberOfLogicalProcessors
        reported_current_clock_mhz = $cpu.CurrentClockSpeed
        manufacturer = $computer.Manufacturer
        model = $computer.Model
        hypervisor_present = $computer.HypervisorPresent
        active_power_scheme = $powerScheme
        thermal_conditions = "Not instrumented; assess host cooling and throttling before treating timings as publishable."
        host_idle_state = "Not independently verified; run on an otherwise idle host for promotion evidence."
    }
    powershell = $PSVersionTable.PSVersion.ToString()
    cargo = (& cargo --version).Trim()
    rustc = (& rustc --version --verbose | Out-String).Trim()
    working_set_sampling_interval_ms = 10
    memory_note = "Peak is whole-process working set sampled every 10 ms, including runtime and code; it is not a per-lane allocation count."
}
$metadata | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $metadataPath
Set-Content -Encoding utf8 $rawPath "# Raw stdout from each independent benchmark process; peak_process_working_set_bytes is sampled by this runner."

for ($round = 1; $round -le $Rounds; $round++) {
    for ($sample = 1; $sample -le $SamplesPerRound; $sample++) {
        $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $executable
        $startInfo.Arguments = "$Warmup $Iterations $QueryRepetitions"
        $startInfo.WorkingDirectory = $PSScriptRoot
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $process = [System.Diagnostics.Process]::new()
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "Could not start benchmark process."
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $peakWorkingSet = 0L
        while (-not $process.HasExited) {
            $process.Refresh()
            if ($process.WorkingSet64 -gt $peakWorkingSet) {
                $peakWorkingSet = $process.WorkingSet64
            }
            Start-Sleep -Milliseconds 10
        }
        $process.WaitForExit()
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) {
            throw "Benchmark failed in round $round sample $sample (exit $($process.ExitCode)): $stderr"
        }
        foreach ($line in ($stdout -split "\r?\n")) {
            if ($line.Trim()) {
                $record = "round=$round sample=$sample peak_process_working_set_bytes=$peakWorkingSet $line"
                Add-Content -Encoding utf8 $rawPath $record
                Write-Output $record
            }
        }
        if ($stderr.Trim()) {
            Add-Content -Encoding utf8 $rawPath "# stderr round=$round sample=$sample $($stderr.Trim())"
        }
        $process.Dispose()
    }
}

Write-Output "raw_evidence=$rawPath"
Write-Output "metadata=$metadataPath"
