<#
.SYNOPSIS
    Mote speed benchmark (docs/benchmark.md, part 3). PowerShell edition of scripts/bench.sh.

.DESCRIPTION
    Fetches the pinned public repositories listed in bench/corpora.toml (shallow, one commit each),
    scans every corpus cold (empty cache directory) and warm (cache primed) -Runs times, and prints
    a Markdown table: files, lines, findings, min/median wall time, lines per second.

    Wall time is measured around the whole mote process (start-up and writing the JSON results
    included). "Cold" means an empty analysis cache, not a cold OS file cache: an untimed run warms
    the file cache (and primes the warm cache) before the timed runs.

    Exit status: 0 done, 1 a median regressed beyond the threshold, 2 usage or setup error.
    Works with Windows PowerShell 5.1 and PowerShell 7.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\bench.ps1 -Only symmetric-ds,vscode -Reuse ..\mote-corpora

.EXAMPLE
    scripts\bench.ps1 -Checkout -Thresholds bench\thresholds.toml -Machine i7-8700k-windows
#>
[CmdletBinding()]
param(
    # mote binary (default: target\release\mote.exe)
    [string]$Bin = "",
    # Corpus list (default: bench\corpora.toml)
    [string]$Corpora = "",
    # Corpus names to run (default: all)
    [string[]]$Only = @(),
    # Timed runs per mode
    [int]$Runs = 5,
    # Worker threads passed to mote (0 = all cores)
    [int]$Threads = 0,
    # Where corpora are fetched (default: $env:MOTE_BENCH_DIR or bench\.corpora)
    [string]$WorkDir = "",
    # Use <Reuse>\<name> when it already holds the pinned commit (never modified; also measured as
    # a working tree when HEAD is the pinned commit and clean)
    [string]$Reuse = "",
    # Check out fetched corpora too and measure working-tree scans
    [switch]$Checkout,
    # Fail instead of fetching a missing corpus (offline runs)
    [switch]$NoFetch,
    # Compare medians with the baselines in this file; exit 1 on a regression
    [string]$Thresholds = "",
    # Baseline table in the thresholds file (default: $env:MOTE_BENCH_MACHINE)
    [string]$Machine = "",
    # Also write the Markdown report to this file
    [string]$Output = ""
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$IsWin = [System.IO.Path]::DirectorySeparatorChar -eq '\'

function Write-Log([string]$Message) { [Console]::Error.WriteLine("bench: $Message") }
function Stop-Bench([string]$Message) {
    [Console]::Error.WriteLine("bench: error: $Message")
    exit 2
}

# Quote one argument for ProcessStartInfo.Arguments (MSVC command-line rules).
function ConvertTo-Arg([string]$Arg) {
    if ($Arg.Length -gt 0 -and $Arg -notmatch '[\s"]') { return $Arg }
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    $backslashes = 0
    foreach ($ch in $Arg.ToCharArray()) {
        if ($ch -eq '\') { $backslashes++; continue }
        if ($ch -eq '"') { [void]$sb.Append([char]92, 2 * $backslashes + 1) }
        elseif ($backslashes -gt 0) { [void]$sb.Append([char]92, $backslashes) }
        $backslashes = 0
        [void]$sb.Append($ch)
    }
    [void]$sb.Append([char]92, 2 * $backslashes)
    [void]$sb.Append('"')
    return $sb.ToString()
}

# Run a program; returns @{ Code; Out; Err }. Output is captured unless -Inherit.
function Invoke-Native([string]$File, [string[]]$Arguments, [switch]$Inherit) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $File
    $psi.Arguments = (($Arguments | ForEach-Object { ConvertTo-Arg $_ }) -join ' ')
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = -not $Inherit
    $psi.RedirectStandardError = -not $Inherit
    $p = [System.Diagnostics.Process]::Start($psi)
    $out = ''; $err = ''
    if (-not $Inherit) {
        $outTask = $p.StandardOutput.ReadToEndAsync()
        $errTask = $p.StandardError.ReadToEndAsync()
        $p.WaitForExit()
        $out = $outTask.Result; $err = $errTask.Result
    } else {
        $p.WaitForExit()
    }
    return @{ Code = $p.ExitCode; Out = $out; Err = $err }
}

function Invoke-Git([string[]]$Arguments) { return Invoke-Native 'git' $Arguments }

# --- options -----------------------------------------------------------------------------------
if (-not $Corpora) { $Corpora = Join-Path $Root 'bench\corpora.toml' }
if (-not $WorkDir) { $WorkDir = if ($env:MOTE_BENCH_DIR) { $env:MOTE_BENCH_DIR } else { Join-Path $Root 'bench\.corpora' } }
if (-not $Machine -and $env:MOTE_BENCH_MACHINE) { $Machine = $env:MOTE_BENCH_MACHINE }
if ($Runs -lt 1) { Stop-Bench '-Runs must be a positive integer' }
if ($Threads -lt 0) { Stop-Bench '-Threads must be a positive integer' }
if (-not (Test-Path -LiteralPath $Corpora -PathType Leaf)) { Stop-Bench "no corpus list at $Corpora" }
if ($Thresholds) {
    if (-not (Test-Path -LiteralPath $Thresholds -PathType Leaf)) { Stop-Bench "no thresholds file at $Thresholds" }
    if (-not $Machine) { Stop-Bench '-Thresholds needs -Machine NAME (or MOTE_BENCH_MACHINE)' }
}
if (-not $Bin) {
    $Bin = Join-Path $Root 'target\release\mote.exe'
    if (-not (Test-Path -LiteralPath $Bin)) { $Bin = Join-Path $Root 'target/release/mote' }
}
if (-not (Test-Path -LiteralPath $Bin -PathType Leaf)) {
    Stop-Bench "mote binary not found at $Bin; build it with: cargo build --release -p mote-cli"
}
$Bin = (Resolve-Path -LiteralPath $Bin).Path
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Stop-Bench 'git is required' }
# Accept both -Only a,b and -Only "a,b".
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

$Scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("mote-bench-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $Scratch | Out-Null
$ResultJson = Join-Path $Scratch 'result.json'

# --- bench/corpora.toml ------------------------------------------------------------------------
function Read-Corpora([string]$Path) {
    $list = New-Object System.Collections.ArrayList
    $cur = $null
    foreach ($line in [System.IO.File]::ReadAllLines($Path)) {
        if ($line -match '^\s*(#|$)') { continue }
        if ($line -match '^\s*\[\[corpus\]\]') {
            if ($cur) { [void]$list.Add($cur) }
            $cur = @{ name = ''; url = ''; rev = ''; language = '' }
            continue
        }
        if ($cur -and $line -match '^\s*([A-Za-z_]+)\s*=\s*"([^"]*)"') { $cur[$Matches[1]] = $Matches[2] }
    }
    if ($cur) { [void]$list.Add($cur) }
    return $list
}

# --- corpora -----------------------------------------------------------------------------------
function Test-Commit([string]$Dir, [string]$Rev) {
    if (-not (Test-Path -LiteralPath (Join-Path $Dir '.git'))) { return $false }
    return (Invoke-Git @('-C', $Dir, 'cat-file', '-e', "$Rev^{commit}")).Code -eq 0
}

function Test-CleanCheckout([string]$Dir, [string]$Rev) {
    $head = Invoke-Git @('-C', $Dir, 'rev-parse', '-q', '--verify', 'HEAD')
    if ($head.Code -ne 0 -or $head.Out.Trim() -ne $Rev) { return $false }
    $status = Invoke-Git @('-C', $Dir, 'status', '--porcelain', '--untracked-files=no')
    return ($status.Code -eq 0 -and -not $status.Out.Trim())
}

function Get-CorpusDir($Corpus) {
    $name = $Corpus.name; $rev = $Corpus.rev
    if ($Reuse) {
        $candidate = Join-Path $Reuse $name
        if (Test-Commit $candidate $rev) { return $candidate }
    }
    $dir = Join-Path $WorkDir $name
    if (-not (Test-Commit $dir $rev)) {
        if ($NoFetch) { Stop-Bench "corpus $name is not available at $rev in $dir (and -NoFetch was given)" }
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        if (-not (Test-Path -LiteralPath (Join-Path $dir '.git'))) {
            if ((Invoke-Git @('-C', $dir, 'init', '-q')).Code -ne 0) { Stop-Bench "git init failed in $dir" }
        }
        if ((Invoke-Git @('-C', $dir, 'remote', 'get-url', 'origin')).Code -eq 0) {
            [void](Invoke-Git @('-C', $dir, 'remote', 'set-url', 'origin', $Corpus.url))
        } else {
            [void](Invoke-Git @('-C', $dir, 'remote', 'add', 'origin', $Corpus.url))
        }
        Write-Log "fetching $name at $($rev.Substring(0, 12)) (depth 1) from $($Corpus.url)"
        $r = Invoke-Git @('-C', $dir, 'fetch', '-q', '--depth', '1', '--no-tags', 'origin', $rev)
        if ($r.Code -ne 0) { Stop-Bench "could not fetch $rev from $($Corpus.url): $($r.Err)" }
    }
    if ($Checkout -and -not (Test-CleanCheckout $dir $rev)) {
        Write-Log "checking out $name at $($rev.Substring(0, 12))"
        $r = Invoke-Git @('-C', $dir, '-c', 'advice.detachedHead=false', 'checkout', '-q', '--force', '--detach', $rev)
        if ($r.Code -ne 0) { Stop-Bench "checkout of $rev failed in ${dir}: $($r.Err)" }
    }
    return $dir
}

# --- measuring ---------------------------------------------------------------------------------
function Measure-Scan([string[]]$Arguments) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $r = Invoke-Native $Bin $Arguments -Inherit
    $sw.Stop()
    if ($r.Code -ne 0) { Stop-Bench "mote exited with $($r.Code): $($Arguments -join ' ')" }
    return $sw.Elapsed.TotalSeconds
}

# files, lines, ncloc, findings (measures.total) and files_analyzed, cache_hits (stats) from the
# pretty-printed results JSON. Read line by line: the file can be tens of MB.
function Read-Numbers([string]$Path) {
    $v = @{ files = 0; lines = 0; ncloc = 0; findings = 0; files_analyzed = 0; cache_hits = 0 }
    $section = ''
    foreach ($line in [System.IO.File]::ReadLines($Path)) {
        if ($line -match '^  "measures": \{') { $section = 'measures'; continue }
        if ($section -eq 'measures' -and $line -match '^    "total": \{') { $section = 'total'; continue }
        if ($section -eq 'total' -and $line -match '^    \}') { $section = ''; continue }
        if ($section -eq 'total' -and $line -match '^      "(files|lines|ncloc|findings)": (\d+)') { $v[$Matches[1]] = [long]$Matches[2]; continue }
        if ($line -match '^  "stats": \{') { $section = 'stats'; continue }
        if ($section -eq 'stats' -and $line -match '^  \}') { $section = ''; continue }
        if ($section -eq 'stats' -and $line -match '^    "(files_analyzed|cache_hits)": (\d+)') { $v[$Matches[1]] = [long]$Matches[2] }
    }
    return $v
}

function Get-MinMedian([double[]]$Values) {
    $s = @($Values | Sort-Object)
    $n = $s.Count
    $median = if ($n % 2) { $s[($n - 1) / 2] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 }
    return @($s[0], $median)
}

$Inv = [System.Globalization.CultureInfo]::InvariantCulture
function Format-Int([long]$N) { return $N.ToString('N0', $Inv) }
function Format-Secs([double]$S) { if ($S -lt 10) { return $S.ToString('0.00', $Inv) + ' s' } return $S.ToString('0.0', $Inv) + ' s' }
function Format-Rate([long]$Lines, [double]$S) {
    if ($S -le 0) { return '-' }
    $r = $Lines / $S
    if ($r -ge 1e6) { return ($r / 1e6).ToString('0.0', $Inv) + 'M' }
    return ($r / 1e3).ToString('0', $Inv) + 'k'
}

$Medians = New-Object System.Collections.ArrayList

function Invoke-Bench($Corpus, [string]$Dir, [string]$Source) {
    $name = $Corpus.name; $rev = $Corpus.rev
    $scanArgs = @('scan', $Dir, '--no-gate', '-q', '--format', 'json', '-o', $ResultJson)
    if ($Source -eq 'rev') { $scanArgs += @('--rev', $rev) }
    if ($Threads -gt 0) { $scanArgs += @('--threads', "$Threads") }
    $warmCache = Join-Path $Scratch 'cache-warm'
    $coldCache = Join-Path $Scratch 'cache-cold'

    if (Test-Path $warmCache) { Remove-Item -Recurse -Force $warmCache }
    Write-Log "$name ($Source): priming run"
    [void](Measure-Scan ($scanArgs + @('--cache-dir', $warmCache)))
    $n = Read-Numbers $ResultJson

    $cold = @(); $warm = @()
    for ($i = 0; $i -lt $Runs; $i++) {
        if (Test-Path $coldCache) { Remove-Item -Recurse -Force $coldCache }
        $cold += Measure-Scan ($scanArgs + @('--cache-dir', $coldCache))
    }
    for ($i = 0; $i -lt $Runs; $i++) {
        $warm += Measure-Scan ($scanArgs + @('--cache-dir', $warmCache))
    }
    $last = Read-Numbers $ResultJson
    if ($last.files_analyzed -ne 0) { Write-Log "warning: a warm scan of $name ($Source) still analyzed $($last.files_analyzed) files" }
    foreach ($d in @($coldCache, $warmCache)) { if (Test-Path $d) { Remove-Item -Recurse -Force $d } }

    $c = Get-MinMedian $cold
    $w = Get-MinMedian $warm
    $fmt = { param($xs) ($xs | ForEach-Object { $_.ToString('0.000', $Inv) }) -join ' ' }
    Write-Log "$name ($Source): cold $(& $fmt $cold) | warm $(& $fmt $warm)"
    [void]$Medians.Add(@("$name.$Source.cold", $c[1]))
    [void]$Medians.Add(@("$name.$Source.warm", $w[1]))
    return ('| {0} | {1} | `{2}` {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} | {11}/s |' -f `
            $name, $Corpus.language, $rev.Substring(0, 10), $Source, (Format-Int $n.files), (Format-Int $n.lines),
        (Format-Int $n.findings), (Format-Secs $c[0]), (Format-Secs $c[1]), (Format-Secs $w[0]), (Format-Secs $w[1]),
        (Format-Rate $n.lines $c[1]))
}

# --- thresholds --------------------------------------------------------------------------------
function Get-ThresholdValue([string]$Key, [string]$Section) {
    $cur = ''
    foreach ($raw in [System.IO.File]::ReadAllLines($Thresholds)) {
        $line = ($raw -replace '#.*$', '').Trim()
        if (-not $line) { continue }
        if ($line -match '^\[(.*)\]$') { $cur = ($Matches[1] -replace '\s', ''); continue }
        if ($cur -ne $Section) { continue }
        if ($line -match '^"?([^"=]+?)"?\s*=\s*"?([^"]*)"?$' -and $Matches[1].Trim() -eq $Key) { return $Matches[2].Trim() }
    }
    return $null
}

function Test-Thresholds {
    $lines = New-Object System.Collections.ArrayList
    $pct = Get-ThresholdValue 'max_regression_pct' "machine.$Machine"
    if (-not $pct) { $pct = Get-ThresholdValue 'max_regression_pct' '' }
    if (-not $pct) { $pct = '25' }
    $pctValue = [double]::Parse($pct, $Inv)
    $failed = $false; $checked = 0
    foreach ($m in $Medians) {
        $base = Get-ThresholdValue $m[0] "machine.$Machine"
        if (-not $base) { continue }
        $checked++
        $baseValue = [double]::Parse($base, $Inv)
        $limit = $baseValue * (1 + $pctValue / 100)
        $median = $m[1].ToString('0.000', $Inv)
        $limitText = $limit.ToString('0.000', $Inv)
        if ($m[1] -gt $limit) {
            [void]$lines.Add("- **Regression** ``$($m[0])``: median $median s > $limitText s (baseline $base s + $pct%)")
            $failed = $true
        } else {
            [void]$lines.Add("- ok ``$($m[0])``: median $median s <= $limitText s (baseline $base s + $pct%)")
        }
    }
    if ($checked -eq 0) { [void]$lines.Add("- No baselines for machine ``$Machine`` in $Thresholds; nothing checked.") }
    return @{ Failed = $failed; Lines = $lines }
}

# --- main --------------------------------------------------------------------------------------
try {
    $rows = New-Object System.Collections.ArrayList
    $selected = 0
    foreach ($corpus in (Read-Corpora $Corpora)) {
        if ($Only.Count -gt 0 -and $Only -notcontains $corpus.name) { continue }
        $selected++
        if (-not $corpus.url -or -not $corpus.rev) { Stop-Bench "corpus $($corpus.name) in $Corpora needs url and rev" }
        $dir = Get-CorpusDir $corpus
        [void]$rows.Add((Invoke-Bench $corpus $dir 'rev'))
        if (Test-CleanCheckout $dir $corpus.rev) { [void]$rows.Add((Invoke-Bench $corpus $dir 'worktree')) }
    }
    if ($selected -eq 0) { Stop-Bench "no corpus selected (-Only $($Only -join ','))" }

    $cpu = ''
    try { $cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim() } catch { $cpu = 'unknown CPU' }
    $version = ((Invoke-Native $Bin @('version')).Out -split "`n")[0].Trim()
    $os = [System.Environment]::OSVersion.VersionString
    $threadsNote = if ($Threads -gt 0) { ", $Threads threads" } else { '' }
    $report = New-Object System.Collections.ArrayList
    [void]$report.Add('### Mote speed benchmark')
    [void]$report.Add('')
    [void]$report.Add("- $version, $([DateTime]::UtcNow.ToString('yyyy-MM-dd HH:mm', $Inv)) UTC")
    [void]$report.Add("- $cpu; $([Environment]::ProcessorCount) logical CPUs; $os; PowerShell $($PSVersionTable.PSVersion)")
    [void]$report.Add("- $Runs timed runs per mode$threadsNote; wall time of the whole process, JSON results written to a file")
    [void]$report.Add('')
    [void]$report.Add('| Corpus | Lang | Source | Files | Lines | Findings | Cold min | Cold median | Warm min | Warm median | Lines/s (cold) |')
    [void]$report.Add('|---|---|---|--:|--:|--:|--:|--:|--:|--:|--:|')
    foreach ($r in $rows) { [void]$report.Add($r) }
    [void]$report.Add('')
    [void]$report.Add('Source: `rev` scans the commit from the object database (`--rev`); `worktree` scans the checkout.')
    [void]$report.Add('Cold: empty cache directory. Warm: cache primed by an earlier run of the same scan.')

    $status = 0
    if ($Thresholds) {
        $t = Test-Thresholds
        [void]$report.Add('')
        [void]$report.Add("#### Regression check (machine ``$Machine``)")
        [void]$report.Add('')
        foreach ($l in $t.Lines) { [void]$report.Add($l) }
        if ($t.Failed) { $status = 1 }
    }
    $text = ($report -join "`n") + "`n"
    [Console]::Out.Write($text)
    if ($Output) { [System.IO.File]::WriteAllText($Output, $text, (New-Object System.Text.UTF8Encoding($false))) }
    if ($status -ne 0) { Write-Log 'median wall time regressed beyond the threshold' }
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Scratch
}
exit $status
