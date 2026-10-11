#Requires -Version 7.2
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$publisher = Join-Path $PSScriptRoot 'publish_bot.ps1'
$keyPath = Join-Path ([IO.Path]::GetTempPath()) "orskit-test-$([Guid]::NewGuid()).pem"
$bodyPath = Join-Path ([IO.Path]::GetTempPath()) "orskit-test-$([Guid]::NewGuid()).md"
$testRsa = [Security.Cryptography.RSA]::Create(2048)

function Assert {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function git {
    $global:LASTEXITCODE = 0
    $arguments = @($args | Select-Object -Skip 2)
    switch ($arguments[0]) {
        remote { return $state.Remotes }
        branch { return $state.Branch }
        status { return $state.Dirty }
        check-ref-format { return }
        push {
            $state.Pushes++
            Assert ($env:GIT_CONFIG_COUNT -eq '5') 'Missing scoped Git authentication.'
            Assert ($env:GIT_CONFIG_VALUE_0 -eq '') 'Personal credential helper was not disabled.'
            $expected = 'Authorization: Basic ' +
                [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('x-access-token:fake-installation-token'))
            Assert ($env:GIT_CONFIG_VALUE_3 -ceq $expected) 'Push used the wrong credential.'
            Assert ($env:GIT_CONFIG_VALUE_4 -eq 'false') 'HTTP redirects were not disabled.'
            Assert (-not (Test-Path Env:GIT_TRACE_CURL) -and -not (Test-Path Env:GIT_CURL_VERBOSE) -and -not (Test-Path Env:GIT_TRACE)) 'HTTP tracing was not disabled.'
            Assert ($arguments -notcontains '--force') 'Unexpected force push.'
            Assert ($arguments[-1] -ceq "HEAD:refs/heads/$($state.Branch)") 'Wrong pushed ref.'
            if ($state.FailPush) { $global:LASTEXITCODE = 1 }
            return
        }
        default { throw "Unexpected Git command: $arguments" }
    }
}

function Invoke-RestMethod {
    param($Method, $Uri, $Headers, $ContentType, $Body)
    $path = ([Uri]$Uri).PathAndQuery
    $state.Requests.Add("$Method $path")
    switch -Regex ($path) {
        '^/app$' {
            $jwt = $Headers.Authorization.Substring(7).Split('.')
            $signature = $jwt[2].Replace('-', '+').Replace('_', '/')
            $signature = $signature.PadRight($signature.Length + ((4 - $signature.Length % 4) % 4), '=')
            Assert ($testRsa.VerifyData(
                [Text.Encoding]::UTF8.GetBytes("$($jwt[0]).$($jwt[1])"),
                [Convert]::FromBase64String($signature),
                [Security.Cryptography.HashAlgorithmName]::SHA256,
                [Security.Cryptography.RSASignaturePadding]::Pkcs1
            )) 'JWT signature verification failed.'
            return @{ id = 5179598; slug = 'orskit-copilot' }
        }
        '^/repos/Wouittone/orskit/installation$' { return @{ id = 123 } }
        '^/app/installations/123/access_tokens$' {
            $request = $Body | ConvertFrom-Json
            Assert ($request.repositories.Count -eq 1 -and $request.repositories[0] -eq 'orskit') 'Token was not repository-scoped.'
            Assert (@($request.permissions.PSObject.Properties).Count -eq 2) 'Unexpected token permissions.'
            return @{ token = 'fake-installation-token'; permissions = @{ contents = 'write'; pull_requests = 'write' } }
        }
        '^/repos/Wouittone/orskit$' { return @{ full_name = 'Wouittone/orskit' } }
        '^/repos/Wouittone/orskit/git/ref/heads/' { return @{ ref = 'refs/heads/main' } }
        # Mirror Invoke-RestMethod: a JSON array is emitted as a single object.
        '^/repos/Wouittone/orskit/pulls\?' { return , [object[]]$state.Existing }
        '^/repos/Wouittone/orskit/pulls$' {
            Assert ($Headers.Authorization -ceq 'Bearer fake-installation-token') 'PR used personal credentials.'
            $request = $Body | ConvertFrom-Json
            Assert ($request.base -ceq 'main' -and $request.head -ceq 'bot/test') 'Wrong PR branches.'
            $state.Creates++
            return @{ user = @{ login = 'orskit-copilot[bot]' }; html_url = 'https://github.com/Wouittone/orskit/pull/999' }
        }
        '^/installation/token$' { $state.Revokes++; return }
        default { throw "Unexpected API request: $Method $path" }
    }
}

function Run-Case {
    param([string]$Name, [hashtable]$Options, [hashtable]$Overrides, [string]$ExpectedError, [int]$Pushes, [int]$Creates)
    $state = @{
        Branch = 'bot/test'; Dirty = ''; Existing = @(); FailPush = $false
        Remotes = @('https://github.com/Wouittone/orskit.git')
        Pushes = 0; Creates = 0; Revokes = 0
        Requests = [Collections.Generic.List[string]]::new()
    }
    foreach ($key in $Overrides.Keys) { $state[$key] = $Overrides[$key] }
    $before = @{}
    $names = @('GH_TOKEN', 'GIT_CONFIG_COUNT', 'GIT_TRACE_CURL', 'GIT_CURL_VERBOSE', 'GIT_CONFIG_VALUE_3')
    # Inherited tracing must be suppressed during push and restored afterwards.
    $env:GIT_CURL_VERBOSE = '0'
    foreach ($variable in $names) { $before[$variable] = [Environment]::GetEnvironmentVariable($variable, 'Process') }
    $caught = $null
    try { & $publisher -PrivateKeyPath $keyPath @Options | Out-Null }
    catch { $caught = $_.Exception.Message }
    if ($ExpectedError) {
        Assert ($null -ne $caught -and $caught.Contains($ExpectedError)) "$Name failed with unexpected error: $caught"
    } else {
        Assert ($null -eq $caught) "$Name failed: $caught"
    }
    Assert ($state.Pushes -eq $Pushes -and $state.Creates -eq $Creates) "$Name had unexpected publishing side effects."
    if ($state.Requests -contains 'POST /app/installations/123/access_tokens') {
        Assert ($state.Revokes -eq 1) "$Name did not revoke its token."
    }
    foreach ($variable in $names) {
        Assert ([Environment]::GetEnvironmentVariable($variable, 'Process') -ceq $before[$variable]) "$Name changed $variable."
    }
    Write-Output "Passed: $Name"
}

try {
    [IO.File]::WriteAllText($keyPath, $testRsa.ExportRSAPrivateKeyPem())
    [IO.File]::WriteAllText($bodyPath, "## Outcome`nTest`n## Evidence`nOffline mocked checks passed.`n## Checklist`n- [x] Test")
    Run-Case 'Read-only authentication on main' @{ Check = $true } @{ Branch = 'main' } '' 0 0
    Run-Case 'New bot PR' @{ Base = 'main'; Title = 'Test'; BodyFile = $bodyPath } @{} '' 1 1
    $botPr = @{ user = @{ login = 'orskit-copilot[bot]' }; base = @{ ref = 'main' }; html_url = 'https://github.com/Wouittone/orskit/pull/999' }
    Run-Case 'Reuse existing bot PR' @{ Base = 'main' } @{ Existing = @($botPr) } '' 1 0
    Run-Case 'Branch-only push' @{ Base = 'main'; PushOnly = $true } @{} '' 1 0
    Run-Case 'Refuse main' @{ Base = 'other' } @{ Branch = 'main' } 'Refusing to publish' 0 0
    Run-Case 'Refuse detached HEAD' @{ Base = 'main' } @{ Branch = '' } 'Refusing to publish' 0 0
    Run-Case 'Refuse multiple push destinations' @{ Check = $true } @{ Remotes = @('https://github.com/Wouittone/orskit.git', 'https://example.com/other.git') } 'only supports origin' 0 0
    Run-Case 'Refuse publishing options during check' @{ Check = $true; Base = 'main' } @{} 'read-only' 0 0
    Run-Case 'Refuse dirty checkout' @{ Base = 'main' } @{ Dirty = ' M README.md' } 'working-tree changes' 0 0
    Run-Case 'Require explicit base' @{} @{} 'target explicitly' 0 0
    Run-Case 'Require new PR body' @{ Base = 'main' } @{} 'new PR requires' 0 0
    $humanPr = @{ user = @{ login = 'Wouittone' }; base = @{ ref = 'main' } }
    Run-Case 'Refuse human-created PR' @{ Base = 'main' } @{ Existing = @($humanPr) } 'not bot-created' 0 0
    $otherBasePr = @{ user = @{ login = 'orskit-copilot[bot]' }; base = @{ ref = 'other' } }
    Run-Case 'Refuse retargeting' @{ Base = 'main' } @{ Existing = @($otherBasePr) } 'different base' 0 0
    Run-Case 'Do not overwrite existing PR' @{ Base = 'main'; Title = 'Overwrite' } @{ Existing = @($botPr) } 'Omit creation options' 0 0
    Run-Case 'Revoke after failed push' @{ Base = 'main'; PushOnly = $true } @{ FailPush = $true } 'Git command failed' 1 0
}
finally {
    $testRsa.Dispose()
    Remove-Item -LiteralPath $keyPath, $bodyPath
}
