[CmdletBinding()]
param(
    [ValidateRange(1, [int]::MaxValue)]
    [int] $Iterations = 10000,

    [ValidateRange(1, 100)]
    [int] $Samples = 3
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$orekit = Join-Path $PSScriptRoot '..\orekit'
$orskitBinary = Join-Path $repository 'target\release\examples\cartesian_position_od_benchmark.exe'

Push-Location $repository
try {
    cargo build --release -j 1 -p orbit-determination --example cartesian_position_od_benchmark
}
finally {
    Pop-Location
}

Push-Location $orekit
try {
    gradle classes --quiet
}
finally {
    Pop-Location
}

for ($sample = 1; $sample -le $Samples; $sample++) {
    & $orskitBinary $Iterations
    if ($LASTEXITCODE -ne 0) { throw "orskit benchmark failed" }
    Push-Location $orekit
    try {
        gradle run --quiet --args=$Iterations
        if ($LASTEXITCODE -ne 0) { throw "Orekit benchmark failed" }
    }
    finally {
        Pop-Location
    }
}
