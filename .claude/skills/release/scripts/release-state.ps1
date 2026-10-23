#Requires -Version 7
<#
.SYNOPSIS
  定型コマンド A: リリースの状態の照会（release スキルの段 2・3・7・8）。読むだけで、何も変えない。
.DESCRIPTION
  main の版・リモートのタグ・公開先（crates.io の 3 クレート・GitHub Release）・リリース CI の実行・
  版の更新の PR を調べ、1 行に 1 つの事実を「 | 」区切りで出す。出力の形は procedure.md の「定型コマンド A の出力」。
  リポジトリのルートを作業ディレクトリにして実行する。
.PARAMETER Version
  調べる版（X.Y.Z）。省略すると origin/main の版を調べる。
.EXAMPLE
  & "$PWD/.claude/skills/release/scripts/release-state.ps1" -Version 0.0.2
#>
param([string]$Version = '')
$V = $Version -replace '^v', ''
git fetch origin main --quiet
$fetch = if ($LASTEXITCODE -eq 0) { '成功' } else { '不明' }
"fetch | origin/main | $fetch"
$found = 0; $unknown = 0; $all = [System.Collections.Generic.List[version]]::new()
if ($fetch -ne '成功') { $unknown++ }
function Add-Ver([string]$s) { if ($s -cmatch '^[0-9]+\.[0-9]+\.[0-9]+\z') { $all.Add([version]$s) } }
function Max-Ver([string[]]$list) {
    $v = @($list | Where-Object { $_ -cmatch '^[0-9]+\.[0-9]+\.[0-9]+\z' } | ForEach-Object { [version]$_ } | Sort-Object)
    if ($v.Count) { "$($v[-1])" } else { '無し' }
}
# main の版（verify-tag.ps1 と同じ読み方）。[workspace.dependencies] の shiori4_abi・shiori4 の version も同じ版でなければ不明
$cargo = (git show origin/main:Cargo.toml) -join "`n"
function Get-Section([string]$name) { if ($cargo -match ('(?ms)^\[' + [regex]::Escape($name) + '\][ \t]*\r?$(.*?)(?=^\[|\z)')) { $Matches[1] } else { '' } }
$vMain = if ((Get-Section 'workspace.package') -match '(?m)^[ \t]*version[ \t]*=[ \t]*"([^"]*)"') { $Matches[1] } else { '不明' }
$deps = Get-Section 'workspace.dependencies'
foreach ($c in 'shiori4_abi', 'shiori4') {
    $dv = if ($deps -match "(?m)^[ \t]*$c[ \t]*=[ \t]*\{[^}\r\n]*\bversion[ \t]*=[ \t]*`"([^`"]*)`"") { $Matches[1] } else { $null }
    if ($dv -cne $vMain) { $vMain = '不明' }
}
"main | Cargo.toml | $vMain"
Add-Ver $vMain
if ($vMain -eq '不明') { $unknown++ }
if (-not $V) { $V = $vMain }
"照会する版 | $V"
# Git のタグ（リモート）
$ls = @(git ls-remote --tags origin 'refs/tags/v*')
$sha = ''
if ($LASTEXITCODE -ne 0) { $unknown++; 'tag | 不明' }
else {
    $names = @($ls | ForEach-Object { ($_ -split "`t")[1] -replace '^refs/tags/v', '' -replace '\^\{\}\z', '' } | Select-Object -Unique)
    $names | ForEach-Object { Add-Ver $_ }
    $mine = @($ls | Where-Object { ($_ -split "`t")[1] -in "refs/tags/v$V", "refs/tags/v$V^{}" })
    if ($mine.Count) { $sha = ($mine[-1] -split "`t")[0] }
    "tag | 最大 $(Max-Ver $names) | v$V $(if ($sha) { "あり $sha" } else { '無し' })"
}
# crates.io（User-Agent 必須。200 = あり、404 = 無し、それ以外 = 不明）
$ua = 'shiori4-release-ci (https://github.com/ekicyou/shiori4)'
function Get-Http([string]$Url) {
    try { $r = Invoke-WebRequest -Uri $Url -UserAgent $ua -SkipHttpErrorCheck -TimeoutSec 30 -MaximumRetryCount 0; @{ Code = [int]$r.StatusCode; Body = [string]$r.Content } }
    catch { @{ Code = 0; Body = '' } }
}
foreach ($c in 'shiori4_abi', 'shiori4', 'shiori4_host') {
    $top = Get-Http "https://crates.io/api/v1/crates/$c"
    # 一度も公開していないクレートは 404。最大は「無し」で、不明には数えない
    $max = switch ($top.Code) { 200 { [string]($top.Body | ConvertFrom-Json).crate.max_version } 404 { '無し' } default { '不明' } }
    Add-Ver $max
    $one = Get-Http "https://crates.io/api/v1/crates/$c/$V"
    $state = switch ($one.Code) { 200 { $found++; 'あり' } 404 { '無し' } default { "不明（HTTP $($one.Code)）" } }
    if ($max -eq '不明' -or $state -like '不明*') { $unknown++ }
    "crates.io | $c | 最大 $max | $V $state"
}
# GitHub Releases（添付は持たない。公開済み = 在って下書きでない）
$list = gh release list --limit 100 --json tagName 2>$null
if ($LASTEXITCODE -ne 0) { $unknown++; 'GitHub Release | 不明' }
else {
    $tags = @(($list -join "`n" | ConvertFrom-Json) | ForEach-Object { $_.tagName -replace '^v', '' })
    $tags | ForEach-Object { Add-Ver $_ }
    $out = gh release view "v$V" --json isDraft,url 2>&1
    $state = if ($LASTEXITCODE -eq 0) {
        $j = ($out -join "`n") | ConvertFrom-Json
        if ($j.isDraft) { '下書き' } else { $found++; "あり（公開） $($j.url)" }
    }
    elseif (($out -join ' ') -match 'release not found') { '無し' }
    else { $unknown++; '不明' }
    "GitHub Release | 最大 $(Max-Ver $tags) | v$V $state"
}
# リリース CI の実行（タグ名とタグのコミットが一致するもののうち、最も新しいもの）
$runs = gh run list --workflow release.yml --limit 30 --json databaseId,headBranch,headSha,status,conclusion,attempt,url,createdAt 2>&1
# release.yml がまだ main に無い（初版の前）なら実行も無い
if ($LASTEXITCODE -ne 0 -and ($runs -join ' ') -match 'workflow release\.yml not found') { 'run | 無し' }
elseif ($LASTEXITCODE -ne 0) { $unknown++; 'run | 不明' }
else {
    $run = @(($runs -join "`n" | ConvertFrom-Json) | Where-Object { $_.headBranch -eq "v$V" -and $sha -and $_.headSha -eq $sha } | Sort-Object createdAt -Descending)
    if ($run.Count) { $r = $run[0]; "run | $($r.databaseId) | $($r.status) | $(if ($r.conclusion) { $r.conclusion } else { '-' }) | attempt $($r.attempt) | $($r.url)" }
    else { 'run | 無し' }
}
# 版の更新の PR（統合されていないもの）
$prs = gh pr list --state open --base main --limit 100 --json number,title,headRefName,url 2>$null
if ($LASTEXITCODE -ne 0) { $unknown++; 'PR | 不明' }
else {
    $pr = @(($prs -join "`n" | ConvertFrom-Json) | Where-Object { $_.title -ceq "chore(release): v$V" })
    if ($pr.Count) { $pr | ForEach-Object { "PR | 未統合 | #$($_.number) | $($_.headRefName) | $($_.url)" } } else { 'PR | 無し' }
}
$maxAll = if ($all.Count) { ($all | Sort-Object)[-1] } else { $null }
"集計 | $V の公開済み $found/4 | 不明 $unknown"
if ($maxAll -and $unknown -eq 0) { "提案 | $($maxAll.Major).$($maxAll.Minor).$($maxAll.Build + 1)（すべての出どころの最大 $maxAll の PATCH + 1）" } else { '提案 | 出せない（不明がある）' }
