#Requires -Version 7
<#
.SYNOPSIS
  定型コマンド B: リリース CI の結果の読み取り（release スキルの段 7）。読むだけで、何も変えない。
.DESCRIPTION
  実行・job・公開の step の結論と、公開の step がログに書いた status= の行を出す。実行が completed になってから使う。
  出力の形は procedure.md の「定型コマンド B の出力」。リポジトリのルートを作業ディレクトリにして実行する。
  Publish shiori4* の「期限まで」は、3 クレートで共有する Auth crates.io の完了から数えたトークンの残り秒（30 分 = 1800 秒）。
.PARAMETER Run
  リリース CI の実行の ID。
.EXAMPLE
  & "$PWD/.claude/skills/release/scripts/ci-result.ps1" -Run 1234567890
#>
param([Parameter(Mandatory)][string]$Run)
$r = (gh run view $RUN --json status,conclusion,attempt,url,headSha,jobs) -join "`n" | ConvertFrom-Json
"run | $RUN | $($r.status) | $(if ($r.conclusion) { $r.conclusion } else { '-' }) | attempt $($r.attempt) | $($r.url)"
$r.jobs | ForEach-Object { "job | $($_.name) | $($_.conclusion)" }
$r.jobs | Where-Object { $_.name -in 'publish-crates', 'github-release' } | ForEach-Object {
    $job = $_.name
    $auth = $_.steps | Where-Object name -eq 'Auth crates.io' | Select-Object -First 1
    $_.steps | Where-Object { $_.name -match '^(Auth crates\.io|Publish |Create GitHub Release)' } | ForEach-Object {
        $sec = if ($_.startedAt -and $_.completedAt) { [int](([datetime]$_.completedAt) - ([datetime]$_.startedAt)).TotalSeconds } else { 0 }
        $left = if ($_.name -notlike 'Publish shiori4*') { '' }
        elseif ($auth.completedAt -and $_.completedAt) { " | 期限まで $(1800 - [int](([datetime]$_.completedAt) - ([datetime]$auth.completedAt)).TotalSeconds) 秒" }
        else { ' | 期限まで - 秒' }  # auth か step が完了していないと数えられない
        "step | $job | $($_.name) | $($_.conclusion) | $sec 秒$left"
    }
}
$pattern = '^(publish-crates|github-release)\t(Publish [^\t]*|Create GitHub Release)\t\S+ (status=(published|skipped|failed)\b.*)$'
1..$r.attempt | ForEach-Object {
    $a = $_
    gh run view $RUN --attempt $a --log 2>$null | Select-String -Pattern $pattern | ForEach-Object {
        $g = $_.Matches[0].Groups
        "line | attempt $a | $($g[1].Value) | $($g[2].Value) | $($g[3].Value.Trim())"
    }
}
