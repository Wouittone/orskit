#Requires -Version 7.2
[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$PushOnly,
    [switch]$AllowWorkflowFiles,
    [string]$Base,
    [string]$Title,
    [string]$BodyFile,
    [switch]$Draft,
    [string]$PrivateKeyPath = (Join-Path $HOME '.config\orskit-copilot\orskit-copilot.pem')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = 'Wouittone/orskit'
$appId = 5179598
$appSlug = 'orskit-copilot'
$root = Split-Path -Parent $PSScriptRoot

function Invoke-Git {
    param([string[]]$Arguments)
    $output = & git -C $root @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Git command failed: git $($Arguments -join ' ')"
    }
    return $output
}

function ConvertTo-Base64Url {
    param([byte[]]$Bytes)
    return [Convert]::ToBase64String($Bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_')
}

function Invoke-GitHub {
    param([string]$Method, [string]$Path, [string]$Token, [object]$Body)
    $parameters = @{
        Method = $Method
        Uri = "https://api.github.com$Path"
        Headers = @{
            Authorization = "Bearer $Token"
            Accept = 'application/vnd.github+json'
            'X-GitHub-Api-Version' = '2022-11-28'
        }
    }
    if ($null -ne $Body) {
        $parameters.ContentType = 'application/json'
        $parameters.Body = ConvertTo-Json -InputObject $Body -Depth 10 -Compress
    }
    try {
        return Invoke-RestMethod @parameters
    }
    catch {
        # Do not include request headers or token-bearing exception details.
        $response = $_.Exception.PSObject.Properties['Response']
        $status = if ($null -ne $response -and $null -ne $response.Value) {
            [int]$response.Value.StatusCode
        } else { 'transport failure' }
        throw "GitHub $Method $Path failed ($status). Check App installation, permissions, and connectivity."
    }
}

if ($Check -and ($PushOnly -or $Draft -or $Title -or $BodyFile -or $Base)) {
    throw '-Check is read-only and cannot be combined with publishing options.'
}
if (-not $Check -and [string]::IsNullOrWhiteSpace($Base)) {
    throw 'Specify the intended PR target explicitly with -Base.'
}
if ($PushOnly -and ($Title -or $BodyFile -or $Draft)) {
    throw '-PushOnly cannot be combined with PR creation options.'
}
$remotes = @(Invoke-Git -Arguments @('remote', 'get-url', '--push', '--all', 'origin'))
if ($remotes.Count -ne 1 -or $remotes[0] -cne 'https://github.com/Wouittone/orskit.git') {
    throw 'This publisher only supports origin https://github.com/Wouittone/orskit.git.'
}
$branch = Invoke-Git -Arguments @('branch', '--show-current')
if (-not $Check) {
    if ([string]::IsNullOrWhiteSpace($branch) -or $branch -in @('main', $Base)) {
        throw 'Refusing to publish detached HEAD, main, or the target branch.'
    }
    if (Invoke-Git -Arguments @('status', '--porcelain')) {
        throw 'Commit or otherwise resolve working-tree changes before publishing. This script never commits or stashes.'
    }
    Invoke-Git -Arguments @('check-ref-format', '--branch', $Base) | Out-Null
}
$keyPath = (Resolve-Path -LiteralPath $PrivateKeyPath).Path
$rootPrefix = $root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
if ($keyPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The private key must be stored outside the repository.'
}
$rsa = [Security.Cryptography.RSA]::Create()
try {
    $rsa.ImportFromPem([IO.File]::ReadAllText($keyPath))
    $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $header = ConvertTo-Base64Url ([Text.Encoding]::UTF8.GetBytes('{"alg":"RS256","typ":"JWT"}'))
    $claims = @{ iat = $now - 60; exp = $now + 540; iss = $appId } | ConvertTo-Json -Compress
    $payload = ConvertTo-Base64Url ([Text.Encoding]::UTF8.GetBytes($claims))
    $unsigned = "$header.$payload"
    $signature = $rsa.SignData(
        [Text.Encoding]::UTF8.GetBytes($unsigned),
        [Security.Cryptography.HashAlgorithmName]::SHA256,
        [Security.Cryptography.RSASignaturePadding]::Pkcs1
    )
    $jwt = "$unsigned.$(ConvertTo-Base64Url $signature)"
}
finally {
    $rsa.Dispose()
}

$app = Invoke-GitHub GET '/app' $jwt
if ($app.slug -cne $appSlug -or $app.id -ne $appId) {
    throw 'The private key did not authenticate the expected orskit-copilot App.'
}
$installation = Invoke-GitHub GET "/repos/$repository/installation" $jwt
$permissions = @{ contents = 'write'; pull_requests = 'write' }
if ($AllowWorkflowFiles) {
    $permissions.workflows = 'write'
}
$grant = Invoke-GitHub POST "/app/installations/$($installation.id)/access_tokens" $jwt @{
    repositories = @('orskit')
    permissions = $permissions
}
$token = $grant.token
$jwt = $null
try {
    $repo = Invoke-GitHub GET "/repos/$repository" $token
    $workflowPermission = $grant.permissions.PSObject.Properties['workflows']
    if ($repo.full_name -cne $repository -or
        $grant.permissions.contents -ne 'write' -or
        $grant.permissions.pull_requests -ne 'write' -or
        ($AllowWorkflowFiles -and
            ($null -eq $workflowPermission -or $workflowPermission.Value -ne 'write'))) {
        throw 'Installation token lacks access to the expected repository or required publishing permissions.'
    }
    if ($Check) {
        Write-Output "Authenticated as $appSlug[bot] for $repository. App/review login and Git configuration are unchanged."
        return
    }

    $encodedBase = [Uri]::EscapeDataString($Base)
    Invoke-GitHub GET "/repos/$repository/git/ref/heads/$encodedBase" $token | Out-Null
    $encodedHead = [Uri]::EscapeDataString("Wouittone:$branch")
    # Invoke-RestMethod emits an empty JSON array as one element; unroll it.
    $existing = @(Invoke-GitHub GET "/repos/$repository/pulls?state=open&head=$encodedHead" $token | ForEach-Object { $_ })
    if ($existing.Count -gt 1) {
        throw 'Multiple open PRs use this head branch; resolve the ambiguity before publishing.'
    }
    if ($existing.Count -eq 1) {
        if ($existing[0].user.login -cne "$appSlug[bot]") {
            throw 'The existing PR is not bot-created. Updating it would not make it independently approvable by its author.'
        }
        if ($existing[0].base.ref -cne $Base) {
            throw 'The existing PR targets a different base. This script does not retarget PRs.'
        }
        if ($Title -or $BodyFile -or $Draft) {
            throw 'The PR already exists. Omit creation options; its title, body, and draft state will be preserved.'
        }
    } elseif (-not $PushOnly) {
        if ([string]::IsNullOrWhiteSpace($Title) -or [string]::IsNullOrWhiteSpace($BodyFile)) {
            throw 'A new PR requires -Title and -BodyFile. Fill in .github\pull_request_template.md, including exact validation results.'
        }
        $body = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $BodyFile).Path)
        foreach ($heading in @('## Outcome', '## Evidence', '## Checklist')) {
            if ($body -notmatch "(?m)^$([regex]::Escape($heading))\r?$") {
                throw "PR body must retain the repository template heading: $heading"
            }
        }
    }

    # Override inherited HTTP auth for this child Git process only.
    $names = @(
        'GIT_CONFIG_COUNT', 'GIT_TERMINAL_PROMPT', 'GIT_ASKPASS', 'GCM_INTERACTIVE',
        'GIT_TRACE', 'GIT_TRACE_CURL', 'GIT_CURL_VERBOSE', 'GIT_TRACE_REDACT'
    )
    for ($i = 0; $i -lt 5; $i++) {
        $names += "GIT_CONFIG_KEY_$i", "GIT_CONFIG_VALUE_$i"
    }
    $saved = @{}
    foreach ($name in $names) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    }
    try {
        $env:GIT_CONFIG_COUNT = '5'
        $env:GIT_CONFIG_KEY_0 = 'credential.helper'
        $env:GIT_CONFIG_VALUE_0 = ''
        $env:GIT_CONFIG_KEY_1 = 'http.extraHeader'
        $env:GIT_CONFIG_VALUE_1 = ''
        $env:GIT_CONFIG_KEY_2 = 'http.https://github.com/Wouittone/orskit.git.extraHeader'
        $env:GIT_CONFIG_VALUE_2 = ''
        $env:GIT_CONFIG_KEY_3 = 'http.https://github.com/Wouittone/orskit.git.extraHeader'
        $env:GIT_CONFIG_VALUE_3 = 'Authorization: Basic ' +
            [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("x-access-token:$token"))
        $env:GIT_CONFIG_KEY_4 = 'http.followRedirects'
        $env:GIT_CONFIG_VALUE_4 = 'false'
        $env:GIT_TERMINAL_PROMPT = '0'
        $env:GIT_ASKPASS = ''
        $env:GCM_INTERACTIVE = 'Never'
        # Git enables curl tracing when GIT_CURL_VERBOSE is merely present.
        foreach ($traceName in 'GIT_TRACE', 'GIT_TRACE_CURL', 'GIT_CURL_VERBOSE') {
            Remove-Item -LiteralPath "Env:$traceName" -ErrorAction SilentlyContinue
        }
        $env:GIT_TRACE_REDACT = '1'
        Invoke-Git -Arguments @('push', 'origin', "HEAD:refs/heads/$branch") | Out-Host
    }
    finally {
        foreach ($name in $names) {
            if ($null -eq $saved[$name]) {
                if (Test-Path -LiteralPath "Env:$name") {
                    Remove-Item -LiteralPath "Env:$name"
                }
            } else {
                [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
            }
        }
    }
    if ($existing.Count -eq 1) {
        Write-Output $existing[0].html_url
    } elseif ($PushOnly) {
        Write-Output "Published $branch as $appSlug[bot]; no PR created."
    } else {
        $pr = Invoke-GitHub POST "/repos/$repository/pulls" $token @{
            head = $branch; base = $Base; title = $Title; body = $body; draft = [bool]$Draft
        }
        if ($pr.user.login -cne "$appSlug[bot]") {
            throw 'GitHub returned an unexpected PR creator.'
        }
        Write-Output $pr.html_url
    }
}
finally {
    # Explicitly expire the short-lived credential even after a failed push.
    Invoke-GitHub DELETE '/installation/token' $token | Out-Null
    $token = $null
    $grant = $null
}
