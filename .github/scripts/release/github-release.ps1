#Requires -Version 7
<#
.SYNOPSIS
  リリースタグの GitHub Release を冪等に作成・公開する。公開済みなら飛ばし、下書きなら公開し、無ければ公開の状態で作る。
.DESCRIPTION
  認証は環境変数 GH_TOKEN（gh がそのまま読む）。対象のリポジトリは gh の既定（checkout の remote か GH_REPO）。
  状態は GitHub 側だけにあり、再実行では gh release view で無し / 下書き / 公開済みを見て続ける。
  削除・上書き（--clobber）・添付をしない。タグは作らない（--verify-tag）。
  -DryRun では判定だけを行い、作成・公開をせずに行うはずの操作を表示する。
.EXAMPLE
  pwsh -NoProfile -File .github/scripts/release/github-release.ps1 -Tag v0.0.1 -NotesFile release-body.md -DryRun
#>
param(
    [Parameter(Mandatory)][string]$Tag,
    [string]$Title = "shiori4 $Tag",
    # Release の本文（release-notes.ps1 -BodyFile の出力）。呼び出し元のカレントディレクトリからの相対パス可。
    [Parameter(Mandatory)][string]$NotesFile,
    [switch]$DryRun
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$started = Get-Date
$url = ''

# $GITHUB_OUTPUT / $GITHUB_STEP_SUMMARY が未設定なら標準出力へ（手元での実行）。
# 値を返す関数の中からも呼ぶので、Write-Output でなく Write-Host（戻り値に混ざらない）。
function Write-To([string]$EnvName, [string]$Text) {
    $path = [Environment]::GetEnvironmentVariable($EnvName)
    if ($path) { Add-Content -LiteralPath $path -Value $Text -Encoding utf8 }
    else { Write-Host $Text }
}

# status（と failed のときの reason）と、分かっていれば url を書いて終える。exit 0 = published / skipped、1 = failed。
function Complete([string]$Status, [string]$Reason, [string]$Message) {
    Write-To GITHUB_OUTPUT "status=$Status"
    if ($Reason) { Write-To GITHUB_OUTPUT "reason=$Reason" }
    if ($url) { Write-To GITHUB_OUTPUT "url=$url" }
    if ($env:GITHUB_OUTPUT) { Write-Host "status=$Status$(if ($Reason) { " reason=$Reason" })$(if ($url) { " url=$url" })" }
    $secs = [int]((Get-Date) - $started).TotalSeconds
    Write-To GITHUB_STEP_SUMMARY "- GitHub Release ``$Tag``: **$Status**$(if ($Reason) { "（reason=$Reason）" }) — $Message$(if ($url) { "（$url）" })（所要 $secs 秒）"
    exit $(if ($Status -eq 'failed') { 1 } else { 0 })
}

# gh を呼び、終了コード・標準出力・全出力（標準エラーを含む）を返す。
function Invoke-Gh([string[]]$GhArgs) {
    $out = [System.Collections.Generic.List[string]]::new()
    $all = [System.Collections.Generic.List[string]]::new()
    & gh @GhArgs 2>&1 | ForEach-Object {
        $line = "$_"
        if ($_ -isnot [System.Management.Automation.ErrorRecord]) { $out.Add($line) }
        $all.Add($line)
    }
    [pscustomobject]@{ Code = $LASTEXITCODE; Out = $out -join "`n"; Text = $all -join "`n" }
}

# タグの Release（下書きも tag 名で見つかる）。無ければ $null。
# 「無い」以外の失敗は状態を判定できないので transient で失敗する。
function Get-Release {
    $r = Invoke-Gh @('release', 'view', $Tag, '--json', 'isDraft,url')
    if ($r.Code -eq 0) { return ($r.Out | ConvertFrom-Json) }
    if ($r.Text -match 'release not found') { return $null }
    Complete failed transient "``gh release view $Tag`` が失敗した（終了コード $($r.Code): $($r.Text)）。状態を判定できない。再実行で続く"
}

try {
    if ($Tag -cnotmatch '^v[0-9]+\.[0-9]+\.[0-9]+\z') { Complete failed publish "タグ ``$Tag`` が vX.Y.Z の形でない" }
    if (-not (Test-Path -LiteralPath $NotesFile -PathType Leaf)) { Complete failed publish "本文のファイル ``$NotesFile`` が無い" }

    # (1) Release の状態を見る
    $rel = Get-Release
    if ($rel) { $url = $rel.url }

    # 公開済み → 飛ばす。作り直さない・題名と本文を上書きしない（4.6）
    if ($rel -and -not $rel.isDraft) { Complete skipped '' '公開済みのため飛ばした' }

    # (2) 行う操作を組み立てる
    if ($rel) {
        # 下書き → 公開する。公開前なので題名と本文を入れ直してよい（4.7）
        $op = @('release', 'edit', $Tag, '--draft=false', '--title', $Title, '--notes-file', $NotesFile)
    }
    else {
        # 無い → 公開の状態で作る。--verify-tag で既存のタグだけを使い、タグを作らない（4.8）
        $op = @('release', 'create', $Tag, '--verify-tag', '--title', $Title, '--notes-file', $NotesFile)
    }

    if ($DryRun) {
        $state = if ($rel) { '下書き' } else { '無い' }
        Write-Output "dry-run: ``$Tag`` の Release は$state。行うはずの操作:"
        Write-Output "  gh $($op -join ' ')"
        if ($url) { Write-To GITHUB_OUTPUT "url=$url" }
        exit 0
    }

    # (3) 実行する
    $r = Invoke-Gh $op
    if ($r.Code -ne 0) { Complete failed publish "``gh $($op[0..1] -join ' ') $Tag`` が失敗した（終了コード $($r.Code): $($r.Text)）。ログを読む" }

    # (4) 状態を取り直す（下書きの間の URL は untagged-… なので取り直す）
    $rel = Get-Release
    if (-not $rel -or $rel.isDraft) { Complete failed transient '操作は成功したが、Release が公開済みとして見つからない。再実行で状態を確かめて続く' }
    $url = $rel.url
    Complete published '' $(if ($op[1] -eq 'edit') { '下書きを公開した' } else { '今回作成した' })
}
catch {
    Complete failed publish "予期しないエラー: $($_.Exception.Message)"
}
