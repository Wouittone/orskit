[CmdletBinding()]
param(
    [string[]] $Phase = @('accuracy', 'timing'),

    [string[]] $Workload = @('oem', 'two-body', 'od'),

    [ValidateRange(1, 100)]
    [int] $Samples = 5,

    [ValidateRange(1, [int]::MaxValue)]
    [int] $TwoBodyIterations = 1000000,

    [ValidateRange(1, [int]::MaxValue)]
    [int] $OdIterations = 10000,

    [string] $OutputDirectory,

    [switch] $Quick,

    [switch] $IncludeReferences
)

$ErrorActionPreference = 'Stop'

function Resolve-Selection {
    param(
        [Parameter(Mandatory)] [string[]] $Values,
        [Parameter(Mandatory)] [string[]] $Allowed,
        [Parameter(Mandatory)] [string] $Name
    )

    $selected = [Collections.Generic.List[string]]::new()
    foreach ($value in $Values) {
        foreach ($part in $value.Replace(',', ' ').Split(
            [char[]]@(' '),
            [StringSplitOptions]::RemoveEmptyEntries
        )) {
            $normalized = $part.Trim().ToLowerInvariant()
            if ($normalized -notin $Allowed) {
                throw "Unknown $Name '$normalized'; expected one of: $($Allowed -join ', ')"
            }
            if ($normalized -notin $selected) {
                $selected.Add($normalized)
            }
        }
    }
    if ($selected.Count -eq 0) { throw "At least one $Name is required" }
    $selected.ToArray()
}

$Phase = Resolve-Selection $Phase @('accuracy', 'timing') 'phase'
$Workload = Resolve-Selection $Workload @('oem', 'two-body', 'od') 'workload'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$timestamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repository "target\benchmark-runs\$timestamp"
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

function Invoke-Captured {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $ArgumentList,
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [Parameter(Mandatory)] [string] $OutputPath
    )

    Push-Location $WorkingDirectory
    try {
        $lines = & $FilePath @ArgumentList 2>&1
        $exitCode = $LASTEXITCODE
        $lines | Tee-Object -FilePath $OutputPath | Out-Host
        if ($exitCode -ne 0) {
            throw "$FilePath exited with code $exitCode"
        }
    }
    finally {
        Pop-Location
    }
}

Push-Location $repository
try {
    $commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'git rev-parse failed' }
    $dirty = [bool](& git status --porcelain)
    if ($LASTEXITCODE -ne 0) { throw 'git status failed' }
    $rust = (& rustc -vV) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'rustc -vV failed' }
    $cargo = (& cargo -V) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'cargo -V failed' }
}
finally {
    Pop-Location
}

$metadata = [ordered]@{
    schema_version = 1
    recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    git_commit = $commit
    git_dirty = $dirty
    cargo_lock_sha256 = (Get-FileHash (Join-Path $repository 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
    phases = @($Phase)
    workloads = @($Workload)
    samples = $Samples
    two_body_iterations = $TwoBodyIterations
    od_iterations = $OdIterations
    quick = [bool]$Quick
    include_references = [bool]$IncludeReferences
    os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
    process_architecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
    logical_processors = [Environment]::ProcessorCount
    cpu_identifier = $env:PROCESSOR_IDENTIFIER
    powershell = $PSVersionTable.PSVersion.ToString()
    cargo = $cargo
    rustc_verbose = $rust
}
$metadata | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'metadata.json')

if ($Phase -contains 'accuracy') {
    $accuracyCommands = [ordered]@{
        oem = @('test', '-p', 'ccsds', '--lib', '--all-features', '--locked')
        'two-body' = @('test', '-p', 'dynamics-two-bodies', '--lib', '--all-features', '--locked')
        od = @('test', '-p', 'orbit-determination', '--lib', '--locked')
    }
    foreach ($name in $Workload) {
        Invoke-Captured cargo $accuracyCommands[$name] $repository (Join-Path $OutputDirectory "$name-accuracy.txt")
    }
}

if ($Phase -contains 'timing') {
    if ($Workload -contains 'oem') {
        $arguments = @('bench', '-p', 'ccsds', '--bench', 'oem', '--locked', '--')
        if ($Quick) { $arguments += '--quick' }
        Invoke-Captured cargo $arguments $repository (Join-Path $OutputDirectory 'oem-timing.txt')
    }

    if ($Workload -contains 'two-body') {
        $implementations = if ($IncludeReferences) { 'orskit,orekit,lox,nyx' } else { 'orskit' }
        $arguments = @(
            (Join-Path $repository '.agent\references\two-body\benchmark\run.ps1'),
            '-Iterations', [string]$TwoBodyIterations,
            '-Samples', [string]$Samples,
            '-Implementations', $implementations
        )
        Invoke-Captured pwsh $arguments $repository (Join-Path $OutputDirectory 'two-body-timing.csv')
    }

    if ($Workload -contains 'od') {
        if ($IncludeReferences) {
            $arguments = @(
                (Join-Path $repository '.agent\references\orbit-determination\benchmark\run.ps1'),
                '-Iterations', [string]$OdIterations,
                '-Samples', [string]$Samples
            )
            Invoke-Captured pwsh $arguments $repository (Join-Path $OutputDirectory 'od-reference-timing.txt')
        }
        else {
            Invoke-Captured cargo @(
                'build', '--release', '-p', 'orbit-determination',
                '--example', 'cartesian_position_od_benchmark', '--locked'
            ) $repository (Join-Path $OutputDirectory 'od-build.txt')
            $suffix = if ([Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
                [Runtime.InteropServices.OSPlatform]::Windows
            )) { '.exe' } else { '' }
            $binary = Join-Path $repository "target\release\examples\cartesian_position_od_benchmark$suffix"
            $lines = for ($sample = 1; $sample -le $Samples; $sample++) {
                $line = & $binary $OdIterations
                if ($LASTEXITCODE -ne 0) { throw "OD benchmark sample $sample failed" }
                "sample=$sample $line"
            }
            $lines | Tee-Object -FilePath (Join-Path $OutputDirectory 'od-timing.txt') | Out-Host
        }
    }
}

Write-Output "benchmark_record=$OutputDirectory"
