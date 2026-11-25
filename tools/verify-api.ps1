#Requires -Version 7.5
<#
.SYNOPSIS
    Checks the Kopia web server API described in docs/API.md against a live server.

.DESCRIPTION
    Read-only. Sends only these requests, all GET:
        /    /api/v1/repo/status    /api/v1/sources    /api/v1/snapshots    /api/v1/objects/{oid}
    Any other path is refused before it reaches the network.

    The password is asked interactively and never written to disk or printed.
    The TLS certificate must match the SHA-256 fingerprint you pass (self-signed certificates
    are fine; certificate validation is never simply turned off).

    Raw JSON responses are saved to local/api-samples/ (git-ignored). They contain real file
    names from your backups: do not commit or share them without anonymizing.

.EXAMPLE
    pwsh -File tools/verify-api.ps1 -Server https://kopia.example:51515 -User ui-user `
         -Fingerprint 0a1b2c...  -Source /data/docs
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Server,
    [Parameter(Mandatory)] [string] $User,
    # SHA-256 of the server certificate. Accepts Kopia format (hex) or OpenSSL format (AA:BB:...).
    [Parameter(Mandatory)] [string] $Fingerprint,
    # Optional: substring of the source path to use for the directory/download test.
    [string] $Source = '',
    [string] $OutDir = (Join-Path $PSScriptRoot '..' 'local' 'api-samples')
)

$ErrorActionPreference = 'Stop'
$TestedVersions = @('0.23.1')

# --- Whitelist -------------------------------------------------------------------------------
$AllowedPaths = @(
    '^/$'
    '^/api/v1/repo/status$'
    '^/api/v1/sources$'
    '^/api/v1/snapshots$'
    '^/api/v1/objects/(I+|Z)?[g-z]?[0-9a-f]{2,64}$'
)

# --- HTTP client with certificate pinning ----------------------------------------------------
if (-not ('RewindVerify.PinnedClient' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;

namespace RewindVerify {
    public static class PinnedClient {
        public static HttpClient Create(string fingerprint, CookieContainer jar) {
            string want = fingerprint.Replace(":", "").Replace(" ", "").Trim().ToLowerInvariant();
            var handler = new HttpClientHandler {
                CookieContainer = jar,
                UseCookies = true,
                AllowAutoRedirect = false,
                ServerCertificateCustomValidationCallback = (msg, cert, chain, errors) => {
                    if (cert == null) return false;
                    string got = Convert.ToHexString(SHA256.HashData(cert.RawData)).ToLowerInvariant();
                    return got == want;
                }
            };
            return new HttpClient(handler) { Timeout = TimeSpan.FromMinutes(10) };
        }
    }
}
'@
}

$base = [Uri]$Server.TrimEnd('/')
if ($base.Scheme -ne 'https') { throw 'Server must be an https:// URL.' }
if (($Fingerprint -replace '[:\s]', '') -notmatch '^[0-9a-fA-F]{64}$') {
    throw 'Fingerprint must be 64 hex characters (SHA-256), with or without colons.'
}

$jar = [System.Net.CookieContainer]::new()
$client = [RewindVerify.PinnedClient]::Create($Fingerprint, $jar)
$script:CsrfToken = $null
$script:Problems = [System.Collections.Generic.List[string]]::new()

function Write-Check([string] $Label, [bool] $Ok, [string] $Detail = '') {
    $mark = if ($Ok) { '[ OK ]' } else { '[FAIL]' }
    $color = if ($Ok) { 'Green' } else { 'Red' }
    Write-Host ("{0} {1}" -f $mark, $Label) -ForegroundColor $color -NoNewline
    if ($Detail) { Write-Host "  $Detail" } else { Write-Host '' }
    if (-not $Ok) { $script:Problems.Add($Label) }
}

function Invoke-KopiaGet {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [hashtable] $Query = @{},
        [switch] $NoCsrf,
        [string] $Range
    )
    if (-not ($AllowedPaths | Where-Object { $Path -match $_ })) {
        throw "Refusing request to non-whitelisted path: $Path"
    }
    $qs = ($Query.GetEnumerator() | Sort-Object Name | ForEach-Object {
            [Uri]::EscapeDataString($_.Name) + '=' + [Uri]::EscapeDataString([string]$_.Value)
        }) -join '&'
    $uri = [Uri]::new($base, $Path + ($(if ($qs) { "?$qs" } else { '' })))

    $req = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::Get, $uri)
    if ($script:CsrfToken -and -not $NoCsrf) { $req.Headers.Add('X-Kopia-Csrf-Token', $script:CsrfToken) }
    if ($Range) { $req.Headers.Range = [System.Net.Http.Headers.RangeHeaderValue]::Parse($Range) }

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $resp = $client.SendAsync($req).GetAwaiter().GetResult()
    $bytes = $resp.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
    $sw.Stop()
    [pscustomobject]@{
        Status = [int]$resp.StatusCode
        Bytes  = $bytes
        Text   = [Text.Encoding]::UTF8.GetString($bytes)
        Ms     = $sw.ElapsedMilliseconds
    }
}

function Save-Sample([string] $Name, [string] $Text) {
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    Set-Content -LiteralPath (Join-Path $OutDir $Name) -Value $Text -Encoding utf8NoBOM
}

function Format-Local([string] $Iso) {
    if (-not $Iso) { return '-' }
    $d = [DateTimeOffset]::Parse($Iso, [Globalization.CultureInfo]::InvariantCulture)
    $age = [DateTimeOffset]::Now - $d
    '{0:yyyy-MM-dd HH:mm} (local, {1:N1} h ago)' -f $d.LocalDateTime, $age.TotalHours
}

# --- Password ----------------------------------------------------------------------------------
$secure = Read-Host -AsSecureString "Password for '$User'"
$plain = [System.Net.NetworkCredential]::new('', $secure).Password
$client.DefaultRequestHeaders.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new(
    'Basic', [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("${User}:$plain")))
$plain = $null

try {
    Write-Host "`n== 1. Bootstrap: GET /" -ForegroundColor Cyan
    $r = Invoke-KopiaGet -Path '/'
    if ($r.Status -ne 200) {
        Write-Check 'GET / returned 200' $false "HTTP $($r.Status): $($r.Text.Trim())"
        throw 'Cannot continue without a session.'
    }
    Write-Check 'GET / returned 200' $true
    $script:CsrfToken = [regex]::Match($r.Text, 'name="kopia-csrf-token" content="([0-9a-f]+)"').Groups[1].Value
    Write-Check 'CSRF token found in page' ([bool]$script:CsrfToken)
    $title = [regex]::Match($r.Text, '<title>([^<]*)</title>').Groups[1].Value
    $version = [regex]::Match($title, 'v(\d+\.\d+\.\d+)').Groups[1].Value
    Write-Check "Server version in <title> ('$title')" ($version -in $TestedVersions) `
        $(if ($version -in $TestedVersions) { 'tested version' } else { "not in tested list: $($TestedVersions -join ', ')" })
    $cookieNames = ($jar.GetCookies($base) | ForEach-Object Name) -join ', '
    Write-Check 'Session cookie set' ($cookieNames -match 'Kopia-Session-Cookie') "cookies: $cookieNames"

    Write-Host "`n== 2. CSRF is enforced" -ForegroundColor Cyan
    $r = Invoke-KopiaGet -Path '/api/v1/sources' -NoCsrf
    Write-Check 'GET /api/v1/sources without CSRF header -> 401' `
        ($r.Status -eq 401 -and $r.Text -match 'CSRF') "HTTP $($r.Status): $($r.Text.Trim())"

    Write-Host "`n== 3. GET /api/v1/repo/status" -ForegroundColor Cyan
    $r = Invoke-KopiaGet -Path '/api/v1/repo/status'
    Write-Check 'status returned 200' ($r.Status -eq 200) "HTTP $($r.Status)"
    $status = $r.Text | ConvertFrom-Json -DateKind String
    Save-Sample 'repo-status.json' $r.Text
    Write-Check 'connected = true' ($status.connected -eq $true)
    Write-Check 'readonly = true' ($status.readonly -eq $true) `
        $(if ($status.readonly -ne $true) { 'WARNING: this server can modify the repository' } else { '' })

    Write-Host "`n== 4. GET /api/v1/sources" -ForegroundColor Cyan
    $r = Invoke-KopiaGet -Path '/api/v1/sources'
    Write-Check 'sources returned 200' ($r.Status -eq 200) "HTTP $($r.Status)"
    $sources = ($r.Text | ConvertFrom-Json -DateKind String).sources
    Save-Sample 'sources.json' $r.Text
    Write-Host "   $($sources.Count) source(s)"

    Write-Host "`n== 5. GET /api/v1/snapshots (per source)" -ForegroundColor Cyan
    $i = 0
    $chosen = $null
    foreach ($s in $sources | Select-Object -First 10) {
        $i++
        $src = $s.source
        $q = @{ host = $src.host; userName = $src.userName; path = $src.path }
        $g = Invoke-KopiaGet -Path '/api/v1/snapshots' -Query $q
        $qa = $q.Clone(); $qa.all = '1'
        $a = Invoke-KopiaGet -Path '/api/v1/snapshots' -Query $qa
        Save-Sample "snapshots-$i.json" $g.Text
        Save-Sample "snapshots-all-$i.json" $a.Text
        $gj = $g.Text | ConvertFrom-Json -DateKind String
        $aj = $a.Text | ConvertFrom-Json -DateKind String
        $list = @($aj.snapshots)
        Write-Host ("   [{0}] {1}@{2}:{3}" -f $i, $src.userName, $src.host, $src.path)
        Write-Host ("       grouped: {0}   all: {1}   (server says unique {2}, unfiltered {3})" -f `
                @($gj.snapshots).Count, $list.Count, $aj.uniqueCount, $aj.unfilteredCount)
        if ($list.Count -ge 2) {
            $asc = [DateTimeOffset]::Parse($list[0].startTime) -le [DateTimeOffset]::Parse($list[-1].startTime)
            Write-Check '       order is oldest -> newest' $asc
        }
        if ($list.Count -ge 1) {
            Write-Host ("       newest snapshot (list):        {0}" -f (Format-Local $list[-1].startTime))
        }
        Write-Host ("       lastSnapshot (sources cache):  {0}" -f (Format-Local $s.lastSnapshot.startTime))
        if (-not $chosen -and $list.Count -ge 1 -and ($Source -eq '' -or $src.path -like "*$Source*")) {
            $chosen = [pscustomobject]@{ Source = $src; Newest = $list[-1] }
        }
    }

    if (-not $chosen) { throw 'No snapshot found for the directory test.' }

    Write-Host "`n== 6. GET /api/v1/objects/{rootID} (directory, no CSRF header)" -ForegroundColor Cyan
    $rootId = $chosen.Newest.rootID
    Write-Host "   source: $($chosen.Source.path)   snapshot: $(Format-Local $chosen.Newest.startTime)"
    $r = Invoke-KopiaGet -Path "/api/v1/objects/$rootId" -NoCsrf
    Write-Check 'directory listing returned 200 without CSRF' ($r.Status -eq 200) "HTTP $($r.Status), $($r.Bytes.Length) bytes, $($r.Ms) ms"
    $dir = $r.Text | ConvertFrom-Json -DateKind String
    Save-Sample 'dir-root.json' $r.Text
    $entries = @($dir.entries)
    $types = ($entries | Group-Object type | ForEach-Object { "$($_.Name)=$($_.Count)" }) -join ' '
    Write-Host "   $($entries.Count) entries ($types)"

    Write-Host "`n== 7. Partial download of one file (Range: bytes=0-99)" -ForegroundColor Cyan
    $file = $entries | Where-Object { $_.type -eq 'f' -and $_.size -ge 100 } | Select-Object -First 1
    if (-not $file) {
        foreach ($d in $entries | Where-Object type -EQ 'd' | Select-Object -First 5) {
            $sub = (Invoke-KopiaGet -Path "/api/v1/objects/$($d.obj)" -NoCsrf).Text | ConvertFrom-Json -DateKind String
            $file = @($sub.entries) | Where-Object { $_.type -eq 'f' -and $_.size -ge 100 } | Select-Object -First 1
            if ($file) { break }
        }
    }
    if ($file) {
        $r = Invoke-KopiaGet -Path "/api/v1/objects/$($file.obj)" -NoCsrf -Range 'bytes=0-99'
        Write-Check 'Range request returned 206 with 100 bytes' ($r.Status -eq 206 -and $r.Bytes.Length -eq 100) `
            "HTTP $($r.Status), $($r.Bytes.Length) bytes (file size $($file.size), OID prefix '$($file.obj.Substring(0,1))')"
    } else {
        Write-Host '   No file >= 100 bytes found near the root; skipped.'
    }
}
finally {
    $client.Dispose()
}

Write-Host ''
if ($script:Problems.Count -eq 0) {
    Write-Host 'All checks passed.' -ForegroundColor Green
} else {
    Write-Host "$($script:Problems.Count) check(s) failed:" -ForegroundColor Red
    $script:Problems | ForEach-Object { Write-Host "  - $_" }
}
Write-Host "Raw responses saved in: $((Resolve-Path $OutDir -ErrorAction SilentlyContinue) ?? $OutDir)"
