# Spike S10: an OS-level layer (L6) on Windows. Windows Defender Firewall blocks every
# executable of the eepview-owned WebView2 Fixed Version runtime from any address but loopback.
# Needs an elevated shell. The fw_harness.py script calls one action at a time.
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Install', 'AuditOn', 'Add', 'Remove', 'Procs', 'DnsClear', 'DnsDump', 'Audit', 'CaptureStart', 'CaptureStop')]
    [string]$Action,
    [string]$Path = '',
    [string]$Out = '',
    [string]$Pattern = '',
    [string]$Since = ''
)

$ErrorActionPreference = 'Stop'
$Group = 'eepview-L6'
# Every address except 127.0.0.0/8 and ::1.
$NotLoopback = @(
    '0.0.0.0-126.255.255.255',
    '128.0.0.0-255.255.255.255',
    '::',
    '::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff'
)

function Assert-MicrosoftSigned([string]$File) {
    $sig = Get-AuthenticodeSignature -FilePath $File
    $subject = $sig.SignerCertificate.Subject
    if ($sig.Status -ne 'Valid' -or $subject -notmatch 'O=Microsoft Corporation') {
        throw "bad signature on ${File}: $($sig.Status) $subject"
    }
    Write-Output "signature ok: $(Split-Path $File -Leaf) - $($sig.Status) - $subject"
}

# Path = the downloaded CAB, Out = the install root (admin-only, under Program Files).
function Install-Runtime {
    Assert-MicrosoftSigned $Path
    New-Item -ItemType Directory -Force -Path $Out | Out-Null
    & expand.exe $Path "-F:*" $Out | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "expand.exe failed: $LASTEXITCODE" }
    $exe = Get-ChildItem -Path $Out -Recurse -Filter 'msedgewebview2.exe' | Select-Object -First 1
    Assert-MicrosoftSigned $exe.FullName
    $dir = $exe.DirectoryName
    # The renderer runs in an AppContainer; it needs read and execute on the runtime folder.
    & icacls.exe $dir /grant '*S-1-15-2-2:(OI)(CI)(RX)' /T /Q | Out-Null
    & icacls.exe $dir /grant '*S-1-15-2-1:(OI)(CI)(RX)' /T /Q | Out-Null
    $files = Get-ChildItem -Path $dir -Recurse -File
    $mb = [math]::Round(($files | Measure-Object -Property Length -Sum).Sum / 1MB)
    Write-Output "runtime: $dir"
    Write-Output "version: $($exe.VersionInfo.ProductVersion), files: $($files.Count), size on disk: $mb MiB"
    Write-Output "executables: $((Get-ChildItem -Path $dir -Recurse -Filter '*.exe').Name -join ', ')"
    if ($env:GITHUB_ENV) { "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=$dir" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV }
}

# Path = the runtime folder. One outbound and one inbound block rule per executable.
function Add-Rules {
    Set-NetFirewallProfile -All -Enabled True -DefaultOutboundAction Allow
    foreach ($exe in Get-ChildItem -Path $Path -Recurse -Filter '*.exe') {
        foreach ($direction in 'Outbound', 'Inbound') {
            New-NetFirewallRule -DisplayName "$Group $direction $($exe.Name)" -Group $Group `
                -Direction $direction -Action Block -Program $exe.FullName -Protocol Any `
                -RemoteAddress $NotLoopback -Profile Any | Out-Null
        }
    }
    Get-NetFirewallRule -Group $Group | Get-NetFirewallApplicationFilter |
        Select-Object -ExpandProperty Program | Sort-Object -Unique |
        ForEach-Object { Write-Output "blocked: $_" }
}

# WFP events 5156 (allowed) and 5157 (blocked), on for every phase so the control phase is real.
function Set-Audit([string]$State) {
    & auditpol.exe /set /subcategory:"Filtering Platform Connection" "/success:$State" "/failure:$State" | Out-Null
}

function Get-EngineProcesses {
    Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | ForEach-Object {
        $type = if ($_.CommandLine -match '--type=(\S+)') { $Matches[1] } else { 'browser' }
        $sub = if ($_.CommandLine -match '--utility-sub-type=(\S+)') { $Matches[1] } else { '' }
        [pscustomobject]@{ pid = $_.ProcessId; path = $_.ExecutablePath; type = $type; sub = $sub }
    } | ConvertTo-Json -Compress
}

# Events 5156 (allowed) and 5157 (blocked) to non-loopback addresses since $Since.
function Get-AuditSummary {
    $start = [datetime]::Parse($Since)
    $events = Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 5156, 5157; StartTime = $start } `
        -ErrorAction SilentlyContinue
    $rows = foreach ($e in $events) {
        $x = [xml]$e.ToXml()
        $d = @{}
        foreach ($n in $x.Event.EventData.Data) { $d[$n.Name] = $n.'#text' }
        if ($d.Direction -ne '%%14593' -or $d.DestAddress -match '^(127\.|::1$)') { continue }
        $verdict = if ($e.Id -eq 5157) { 'blocked' } else { 'allowed' }
        "$verdict $(Split-Path $d.Application -Leaf) -> $($d.DestAddress):$($d.DestPort)/$($d.Protocol)"
    }
    $rows | Group-Object | Sort-Object Count -Descending |
        ForEach-Object { [pscustomobject]@{ n = $_.Count; flow = $_.Name } } | ConvertTo-Json -Compress
}

switch ($Action) {
    'Install' { Install-Runtime }
    'Add' { Add-Rules }
    'AuditOn' { Set-Audit 'enable' }
    'Remove' {
        Remove-NetFirewallRule -Group $Group -ErrorAction SilentlyContinue
        Set-Audit 'disable'
    }
    'Procs' { Get-EngineProcesses }
    'DnsClear' { Clear-DnsClientCache }
    'DnsDump' {
        Get-DnsClientCache | Where-Object { $_.Entry -like "*$Pattern*" } |
            ForEach-Object { "$($_.Entry) status=$($_.Status) data=$($_.Data)" } | ConvertTo-Json -Compress
    }
    'Audit' { Get-AuditSummary }
    'CaptureStart' {
        & pktmon.exe filter remove | Out-Null
        & pktmon.exe filter add S10DNS -p 53 | Out-Null
        & pktmon.exe start --capture --pkt-size 0 --file-name $Out | Out-Null
    }
    'CaptureStop' {
        & pktmon.exe stop | Out-Null
        & pktmon.exe etl2pcap $Path --out $Out | Out-Null
    }
}
