param(
    [switch]$Check
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$metadata = cargo metadata --format-version 1 --no-deps --locked --manifest-path "$repositoryRoot/Cargo.toml" |
    ConvertFrom-Json

$layers = @(
    [pscustomobject]@{ Name = 'Public facade'; Packages = @('orskit') }
    [pscustomobject]@{ Name = 'Workflows and I/O'; Packages = @('ccsds', 'orbit-determination', 'measurements') }
    [pscustomobject]@{ Name = 'Dynamics'; Packages = @('dynamics', 'dynamics-core', 'dynamics-numerical', 'dynamics-two-bodies') }
    [pscustomobject]@{ Name = 'Physical model'; Packages = @('core', 'orbits', 'attitude', 'gravity', 'frames', 'bodies') }
    [pscustomobject]@{ Name = 'Foundations'; Packages = @('utils', 'units') }
)

function ConvertTo-NodeId([string]$Name) {
    return ($Name -replace '[^A-Za-z0-9]', '_').ToUpperInvariant()
}

$workspaceNames = [System.Collections.Generic.HashSet[string]]::new(
    [string[]]$metadata.packages.name
)
$declaredNames = [System.Collections.Generic.HashSet[string]]::new(
    [string[]]($layers.Packages | ForEach-Object { $_ })
)
if (-not $workspaceNames.SetEquals($declaredNames)) {
    $missing = [string[]]($workspaceNames | Where-Object { -not $declaredNames.Contains($_) })
    $stale = [string[]]($declaredNames | Where-Object { -not $workspaceNames.Contains($_) })
    throw "Update the layer map. Unassigned packages: [$($missing -join ', ')]; stale packages: [$($stale -join ', ')]."
}

$diagram = [System.Collections.Generic.List[string]]::new()
$diagram.Add('flowchart TB')
foreach ($layer in $layers) {
    $diagram.Add("    subgraph LAYER_$(ConvertTo-NodeId $layer.Name)[`"$($layer.Name)`"]")
    foreach ($packageName in $layer.Packages) {
        $label = if ($packageName -eq 'core') { 'core (lib: orskit_core)' } else { $packageName }
        $diagram.Add("        $(ConvertTo-NodeId $packageName)[`"$label`"]")
    }
    $diagram.Add('    end')
}

$edges = foreach ($package in $metadata.packages) {
    foreach ($dependency in $package.dependencies) {
        if ($null -eq $dependency.kind -and $workspaceNames.Contains([string]$dependency.name)) {
            [pscustomobject]@{
                Source = [string]$package.name
                Target = [string]$dependency.name
                Optional = [bool]$dependency.optional
            }
        }
    }
}
foreach ($edge in $edges | Sort-Object Source, Target, Optional -Unique) {
    $arrow = if ($edge.Optional) { '-.->|optional|' } else { '-->' }
    $diagram.Add("    $(ConvertTo-NodeId $edge.Source) $arrow $(ConvertTo-NodeId $edge.Target)")
}

$generatedDiagram = $diagram -join "`n"
$expected = @"
# Current Rust crate architecture

This diagram describes the checked-in Cargo workspace, not the complete target
architecture. It is generated from normal path dependencies returned by
``cargo metadata --format-version 1 --no-deps --locked``; development-only
dependencies are omitted and optional dependencies use dashed arrows.

``````mermaid
$generatedDiagram
``````

Dependencies point from a consumer to the crate it uses. The layer groupings
are maintained in ``scripts/check_crate_diagram.ps1``; the script fails when a
workspace package has not been assigned to a layer. Run ``just diagram`` after
a manifest change and ``just diagram-check`` in review. Both commands use the
locked dependency graph.

The graph is descriptive. The normative dependency direction and the meaning
of each domain boundary remain in [the target architecture](../.agent/ARCHITECTURE.md).
In particular, the feature-gated ``dynamics`` and ``orskit`` facades point inward
to implementations because they curate exports; this does not permit a
physical-model crate to depend upward on either facade.
"@
$expected = $expected.Replace("`r`n", "`n").TrimEnd() + "`n"
$destination = Join-Path $repositoryRoot 'docs/architecture.md'

if ($Check) {
    $actual = if (Test-Path -LiteralPath $destination) {
        [IO.File]::ReadAllText($destination).Replace("`r`n", "`n")
    } else {
        ''
    }
    if ($actual -cne $expected) {
        Write-Error 'docs/architecture.md is stale; run `just diagram`.'
    }
    Write-Output 'docs/architecture.md matches Cargo metadata.'
    exit 0
}

[IO.File]::WriteAllText($destination, $expected, [Text.UTF8Encoding]::new($false))
Write-Output 'Updated docs/architecture.md.'
