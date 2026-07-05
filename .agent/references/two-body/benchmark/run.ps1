[CmdletBinding()]
param(
    [ValidateRange(1, [int]::MaxValue)]
    [int] $Iterations = 1000000,

    [ValidateRange(1, 100)]
    [int] $Samples = 5,

    [ValidateRange(1, 1000)]
    [int] $PollMilliseconds = 2,

    [string[]] $Implementations = @('orskit', 'orekit', 'lox', 'nyx'),

    [string] $NyxExecutable,

    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$normalizedImplementations = [Collections.Generic.List[string]]::new()
foreach ($item in $Implementations) {
    $parts = $item.Replace(',', ' ').Split(
        [char[]]@(' '),
        [StringSplitOptions]::RemoveEmptyEntries
    )
    foreach ($part in $parts) {
        $normalizedImplementations.Add($part.Trim().ToLowerInvariant())
    }
}
$Implementations = $normalizedImplementations.ToArray()
$unknownImplementations = @($Implementations | Where-Object { $_ -notin @('orskit', 'orekit', 'lox', 'nyx') })
if ($unknownImplementations.Count -gt 0) {
    throw "Unknown implementation(s): $($unknownImplementations -join ', ')"
}
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$isWindowsPlatform = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
$executableSuffix = if ($isWindowsPlatform) { '.exe' } else { '' }

function Invoke-Checked {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $ArgumentList,
        [Parameter(Mandatory)] [string] $WorkingDirectory
    )

    Push-Location $WorkingDirectory
    try {
        & $FilePath @ArgumentList
        if ($LASTEXITCODE -ne 0) {
            throw "$FilePath exited with code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

if (-not $SkipBuild) {
    if ($Implementations -contains 'orskit') {
        Invoke-Checked cargo @('build', '--release', '-p', 'orskit-dynamics', '--example', 'two_body_benchmark') $repository
    }
    if ($Implementations -contains 'lox') {
        Invoke-Checked cargo @(
            'build',
            '--release',
            '--manifest-path',
            (Join-Path $PSScriptRoot 'lox\Cargo.toml')
        ) $repository
    }
    if ($Implementations -contains 'orekit') {
        $gradle = if ($isWindowsPlatform) { 'gradle.bat' } else { 'gradle' }
        Invoke-Checked $gradle @('installDist', '--quiet') (Join-Path $repository '.agent\references\two-body\orekit')
    }
    if (($Implementations -contains 'nyx') -and -not $NyxExecutable) {
        Invoke-Checked cargo @(
            'build',
            '--release',
            '--manifest-path',
            (Join-Path $PSScriptRoot 'nyx\Cargo.toml')
        ) $repository
    }
}

function Measure-Process {
    param(
        [Parameter(Mandatory)] [string] $Implementation,
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $ArgumentList,
        [Parameter(Mandatory)] [int] $Sample
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $ArgumentList) {
        $startInfo.ArgumentList.Add($argument)
    }

    $process = [System.Diagnostics.Process]::Start($startInfo)
    $peakWorkingSet = 0L
    while (-not $process.HasExited) {
        try {
            $process.Refresh()
            $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.WorkingSet64)
        }
        catch [System.InvalidOperationException] {
            break
        }
        Start-Sleep -Milliseconds $PollMilliseconds
    }
    $process.WaitForExit()
    $stdout = $process.StandardOutput.ReadToEnd().Trim()
    $stderr = $process.StandardError.ReadToEnd().Trim()
    if ($process.ExitCode -ne 0) {
        throw "$Implementation failed with code $($process.ExitCode): $stderr"
    }

    $fields = @{}
    foreach ($token in $stdout -split '\s+') {
        $pair = $token -split '=', 2
        if ($pair.Count -eq 2) {
            $fields[$pair[0]] = $pair[1]
        }
    }
    if (
        -not $fields.ContainsKey('implementation') -or
        -not $fields.ContainsKey('iterations') -or
        -not $fields.ContainsKey('elapsed_ns') -or
        -not $fields.ContainsKey('checksum')
    ) {
        throw "Unexpected $Implementation output: $stdout"
    }
    if ($fields['implementation'] -ne $Implementation) {
        throw "Expected implementation=$Implementation, received: $stdout"
    }
    if ([int]$fields['iterations'] -ne $Iterations) {
        throw "Expected iterations=$Iterations, received: $stdout"
    }

    $elapsedNs = [double]::Parse($fields['elapsed_ns'], [Globalization.CultureInfo]::InvariantCulture)
    [pscustomobject]@{
        implementation = $Implementation
        sample = $Sample
        iterations = $Iterations
        elapsed_ns = [long]$elapsedNs
        queries_per_second = ($Iterations * 1.0e9 / $elapsedNs).ToString(
            'F3',
            [Globalization.CultureInfo]::InvariantCulture
        )
        peak_working_set_bytes = $peakWorkingSet
        checksum = $fields['checksum']
    }
}

$commands = @{}
if ($Implementations -contains 'orskit') {
    $commands['orskit'] = @{
        FilePath = Join-Path $repository "target\release\examples\two_body_benchmark$executableSuffix"
        Arguments = @([string]$Iterations)
    }
}
if ($Implementations -contains 'lox') {
    $commands['lox'] = @{
        FilePath = Join-Path $PSScriptRoot "lox\target\release\lox-two-body-benchmark$executableSuffix"
        Arguments = @([string]$Iterations)
    }
}
if ($Implementations -contains 'orekit') {
    $installRoot = Join-Path $repository '.agent\references\two-body\orekit\build\install\orskit-orekit-two-body-reference'
    $commands['orekit'] = @{
        FilePath = 'java'
        Arguments = @(
            '-cp',
            (Join-Path $installRoot 'lib\*'),
            'org.orskit.reference.TwoBodyBenchmark',
            [string]$Iterations
        )
    }
}
if ($Implementations -contains 'nyx') {
    $commands['nyx'] = @{
        FilePath = if ($NyxExecutable) {
            (Resolve-Path $NyxExecutable).Path
        }
        else {
            Join-Path $PSScriptRoot "nyx\target\release\nyx-two-body-benchmark$executableSuffix"
        }
        Arguments = @([string]$Iterations)
    }
}

$results = foreach ($sample in 1..$Samples) {
    foreach ($implementation in $Implementations) {
        $command = $commands[$implementation]
        Measure-Process $implementation $command.FilePath $command.Arguments $sample
    }
}

$results | ConvertTo-Csv -NoTypeInformation
