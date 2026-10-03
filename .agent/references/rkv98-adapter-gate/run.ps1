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

function Convert-RecordToFields([string]$RecordLine) {
    $fields = @{}
    foreach ($match in [regex]::Matches($RecordLine, '(?:^|\s)([A-Za-z][A-Za-z0-9_]*)=([^\s]+)')) {
        $fields[$match.Groups[1].Value] = $match.Groups[2].Value
    }
    return $fields
}

function Assert-PairedError([string]$Label, [string]$NativeLine, [string]$Vern9Line, [string]$PositionKey, [string]$VelocityKey) {
    $native = Convert-RecordToFields $NativeLine
    $vern9 = Convert-RecordToFields $Vern9Line
    foreach ($metric in @(
        @{ Name = "position"; Key = $PositionKey },
        @{ Name = "velocity"; Key = $VelocityKey }
    )) {
        if (-not $native.ContainsKey($metric.Key) -or -not $vern9.ContainsKey($metric.Key)) {
            throw "$Label is missing paired $($metric.Name) error field '$($metric.Key)'."
        }
        $nativeValue = [double]::Parse($native[$metric.Key], [Globalization.CultureInfo]::InvariantCulture)
        $vern9Value = [double]::Parse($vern9[$metric.Key], [Globalization.CultureInfo]::InvariantCulture)
        $scale = [math]::Max([math]::Abs($nativeValue), [math]::Abs($vern9Value))
        if ($scale -ne 0 -and [math]::Abs($nativeValue - $vern9Value) -gt 0.25 * $scale) {
            throw "$Label paired $($metric.Name) errors differ by more than 25%: native=$nativeValue Vern9=$vern9Value."
        }
    }
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
$sourceFile = Join-Path $PSScriptRoot "src\main.rs"
$runnerFile = Join-Path $PSScriptRoot "run.ps1"
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
    harness_input_sha256 = [ordered]@{
        cargo_toml = (Get-FileHash $manifest -Algorithm SHA256).Hash.ToLowerInvariant()
        cargo_lock = (Get-FileHash $lockFile -Algorithm SHA256).Hash.ToLowerInvariant()
        main_rs = (Get-FileHash $sourceFile -Algorithm SHA256).Hash.ToLowerInvariant()
        run_ps1 = (Get-FileHash $runnerFile -Algorithm SHA256).Hash.ToLowerInvariant()
    }
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
    peak_capture_protocol = "After all benchmark regions finish, the harness flushes a completion marker and waits. The runner reads Process.PeakWorkingSet64 while the process is held, then releases it."
    memory_metric = "Windows Process.PeakWorkingSet64 OS-maintained process high-water working set captured after benchmark completion and before process exit; whole process footprint, including runtime and code, not retained heap memory."
}
$metadata | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $metadataPath
Set-Content -Encoding utf8 $rawPath "# Each record comes from an isolated lane/mode process; peak_process_working_set_bytes is the OS-maintained Process.PeakWorkingSet64 high-water working set captured at the post-measurement completion handshake, before process exit; not retained heap memory."

$modes = @(
    "native-two-body",
    "vern9-two-body",
    "native-drag",
    "vern9-drag",
    "native-dense",
    "vern9-dense"
)
for ($round = 1; $round -le $Rounds; $round++) {
    for ($sample = 1; $sample -le $SamplesPerRound; $sample++) {
        $sampleRecords = @{}
        foreach ($mode in $modes) {
            $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
            $startInfo.FileName = $executable
            $startInfo.Arguments = "$Warmup $Iterations $QueryRepetitions $mode hold-for-peak"
            $startInfo.WorkingDirectory = $PSScriptRoot
            $startInfo.UseShellExecute = $false
            $startInfo.CreateNoWindow = $true
            $startInfo.RedirectStandardOutput = $true
            $startInfo.RedirectStandardError = $true
            $startInfo.RedirectStandardInput = $true
            $process = [System.Diagnostics.Process]::new()
            $process.StartInfo = $startInfo
            if (-not $process.Start()) {
                throw "Could not start benchmark process for mode $mode."
            }
            $stderrTask = $process.StandardError.ReadToEndAsync()
            $stdoutLines = [System.Collections.Generic.List[string]]::new()
            $completionMarkerSeen = $false
            $peakWorkingSet = 0L
            $lineTask = $process.StandardOutput.ReadLineAsync()
            while ($null -ne ($line = $lineTask.GetAwaiter().GetResult())) {
                if ($line -eq "BENCHMARK_COMPLETE_WAITING_FOR_PEAK") {
                    if ($completionMarkerSeen) {
                        throw "Benchmark emitted duplicate peak-capture completion markers in round $round sample $sample mode $mode."
                    }
                    $completionMarkerSeen = $true
                    $process.Refresh()
                    $peakWorkingSet = $process.PeakWorkingSet64
                    $process.StandardInput.WriteLine("release_peak_capture")
                    $process.StandardInput.Flush()
                    $process.StandardInput.Close()
                } else {
                    $stdoutLines.Add($line)
                }
                $lineTask = $process.StandardOutput.ReadLineAsync()
            }
            $process.WaitForExit()
            $stderr = $stderrTask.GetAwaiter().GetResult()
            if ($process.ExitCode -ne 0) {
                throw "Benchmark failed in round $round sample $sample mode $mode (exit $($process.ExitCode)): $stderr"
            }
            if (-not $completionMarkerSeen) {
                throw "Benchmark exited without the required post-measurement peak-capture marker in round $round sample $sample mode $mode."
            }
            if ($peakWorkingSet -eq 0) {
                throw "Could not capture the OS-maintained peak working set for round $round sample $sample mode $mode."
            }
            $resultLines = @($stdoutLines | Where-Object { $_ -match '^record=(endpoint|dense-query)\s' })
            if ($resultLines.Count -ne 1) {
                throw "Expected exactly one measured record in round $round sample $sample mode $mode, got $($resultLines.Count)."
            }
            $sampleRecords[$mode] = $resultLines[0]
            foreach ($line in $stdoutLines) {
                if ($line.Trim()) {
                    $record = "round=$round sample=$sample mode=$mode peak_process_working_set_bytes=$peakWorkingSet $line"
                    Add-Content -Encoding utf8 $rawPath $record
                    Write-Output $record
                }
            }
            if ($stderr.Trim()) {
                Add-Content -Encoding utf8 $rawPath "# stderr round=$round sample=$sample mode=$mode $($stderr.Trim())"
            }
            $process.Dispose()
        }
        Assert-PairedError "Two-body LEO endpoint" $sampleRecords["native-two-body"] $sampleRecords["vern9-two-body"] "position_error_m" "velocity_error_m_s"
        Assert-PairedError "Velocity-dependent LEO endpoint" $sampleRecords["native-drag"] $sampleRecords["vern9-drag"] "position_error_m" "velocity_error_m_s"
        Assert-PairedError "Dense-query trajectory" $sampleRecords["native-dense"] $sampleRecords["vern9-dense"] "max_position_error_m" "max_velocity_error_m_s"
    }
}

Write-Output "raw_evidence=$rawPath"
Write-Output "metadata=$metadataPath"
