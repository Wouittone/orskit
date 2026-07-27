[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Invoke-Cargo {
    param(
        [Parameter(Mandatory)]
        [string] $Label,
        [Parameter(Mandatory)]
        [string[]] $Arguments
    )

    Write-Host "Checking $Label"
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo failed while checking $Label"
    }
}

function Assert-FacadeExcludesPackages {
    param(
        [Parameter(Mandatory)]
        [string] $Features,
        [Parameter(Mandatory)]
        [string[]] $Packages
    )

    $tree = & cargo tree -p orskit --no-default-features --features $Features --locked `
        --edges normal --prefix none --format "{p}"
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo tree failed for facade features: $Features"
    }
    foreach ($package in $Packages) {
        if ($tree -match "^$([regex]::Escape($package)) v") {
            throw "Facade features '$Features' unexpectedly enable package '$package'"
        }
    }
}

$previousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = "$previousRustFlags -D warnings".Trim()

try {
    $facadeRows = @(
        @{ Label = "minimal facade"; Features = $null },
        @{ Label = "Earth orientation"; Features = "earth-orientation" },
        @{ Label = "Cartesian states"; Features = "cartesian" },
        @{ Label = "CCSDS ingestion"; Features = "ccsds" },
        @{ Label = "TLE ingestion"; Features = "tle" },
        @{ Label = "SGP4 propagation"; Features = "sgp4" },
        @{ Label = "physical ephemeris"; Features = "ephemeris" },
        @{ Label = "two-body propagation"; Features = "two-bodies" },
        @{ Label = "numerical propagation"; Features = "numerical" },
        @{ Label = "measurement contracts"; Features = "measurements" },
        @{ Label = "range measurement"; Features = "measurement-range" },
        @{ Label = "geometric range estimation"; Features = "measurement-geometric-range" },
        @{ Label = "orbit determination"; Features = "orbit-determination" },
        @{ Label = "serialization boundary only"; Features = "serialization" },
        @{ Label = "serialization plus Cartesian states"; Features = "serialization,cartesian" },
        @{ Label = "serialization plus two-body propagation"; Features = "serialization,two-bodies" },
        @{ Label = "JSON serialization boundary"; Features = "serialization-json" }
    )

    foreach ($row in $facadeRows) {
        $arguments = @("check", "-p", "orskit", "--no-default-features", "--locked")
        if ($null -ne $row.Features) {
            $arguments += @("--features", $row.Features)
        }
        Invoke-Cargo -Label $row.Label -Arguments $arguments
    }

    Assert-FacadeExcludesPackages -Features "serialization" -Packages @(
        "orbits", "dynamics-two-bodies"
    )
    Assert-FacadeExcludesPackages -Features "serialization-json" -Packages @(
        "orbits", "dynamics-two-bodies"
    )

    Invoke-Cargo -Label "maximal facade" -Arguments @(
        "check", "-p", "orskit", "--all-features", "--locked"
    )

    foreach ($features in @($null, "orbits", "orbits,json", "two-bodies,json")) {
        $arguments = @("test", "-p", "orskit-export", "--no-default-features", "--locked")
        $label = "export boundary only"
        if ($null -ne $features) {
            $arguments += @("--features", $features)
            $label = "export features: $features"
        }
        Invoke-Cargo -Label $label -Arguments $arguments
    }
}
finally {
    if ($null -eq $previousRustFlags) {
        Remove-Item Env:RUSTFLAGS
    }
    else {
        $env:RUSTFLAGS = $previousRustFlags
    }
}
