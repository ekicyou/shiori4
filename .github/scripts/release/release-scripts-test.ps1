#Requires -Version 7
<#
.SYNOPSIS
  リリースの補助スクリプト（release-notes.ps1・verify-tag.ps1・定型コマンド D の highlights-check.ps1）を、一時の git リポジトリで実際に動かして確かめる。
.DESCRIPTION
  一時のフォルダに git リポジトリを作り、決まった件名のコミットとタグ（v0.0.1・v0.0.2）を積んで、スクリプトを子の pwsh で呼ぶ。
  ネットワーク・gh・crates.io を使わない。一時のフォルダは終わりに消し、リポジトリの作業ツリーを変えない。
  項目ごとに ok か NG を 1 行で出す。1 つでも NG なら NG の項目を並べて終了コード 1、すべて ok なら 0。
  項目を足すときは、Test-Item の並びに 1 つ足す。
.EXAMPLE
  pwsh -NoProfile -File .github/scripts/release/release-scripts-test.ps1
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = 'test/repo'
$Pwsh = [Environment]::ProcessPath
# 詳しいリリースノートの見出し（この順で出る）
$Headings = @(
    '## ⚠️ 動きが変わる変更'
    '## ✨ 新機能'
    '## 🐛 不具合の修正'
    '## ♻️ リファクタリング'
    '## 📝 ドキュメント'
    '## 🧪 テスト'
    '## 🔧 そのほか'
)
# v0.0.1..v0.0.2 で載る件数（別名・戻り先・失敗表記・公開の準備・落ちる不具合・手引き・接頭辞の無い件名）
$ListedCount = 7
$Highlights = "## 今回の見どころ`n`n### 別名が書けます (#1)`n`nこれは見本です。`n`n### ご注意`n`n- 戻り先の文脈が変わります。 (#2)`n- 失敗表記が出ます。 (#5)`n`n`n"

# 子のスクリプトが GitHub Actions の出力先へ書かないようにする（この処理の中だけで効く）
Remove-Item Env:GITHUB_OUTPUT, Env:GITHUB_STEP_SUMMARY -ErrorAction SilentlyContinue

$tmp = Join-Path ([IO.Path]::GetTempPath()) "release-scripts-test-$([guid]::NewGuid().ToString('N'))"
$repoDir = Join-Path $tmp 'repo'
$ng = [System.Collections.Generic.List[string]]::new()
$savedEncoding = [Console]::OutputEncoding
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

function Write-Utf8([string]$Path, [string]$Text) {
    New-Item -ItemType Directory -Force -Path (Split-Path $Path) | Out-Null
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}

# 一時のリポジトリで git を動かす。失敗したら止める（検査の前提が崩れている）。
function Invoke-Git {
    $out = & git -C $repoDir @args 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git $args が失敗した: $($out | Out-String)" }
    $out
}

function Add-Commit([string]$Subject) { Invoke-Git commit -q --allow-empty --no-verify -m $Subject | Out-Null }

# 補助スクリプトを子の pwsh で呼ぶ（スクリプトの exit で検査が終わらないように）。
function Invoke-Script([string]$Name, [string[]]$ArgList) {
    $out = & $Pwsh -NoProfile -File (Join-Path $PSScriptRoot $Name) @ArgList 2>&1 | Out-String
    [pscustomobject]@{ Exit = $LASTEXITCODE; Output = $out.Trim() }
}

function Read-Lines([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    @([IO.File]::ReadAllText($Path) -split "`r?`n")
}

# 見出しの下の箇条書き（次の見出しか Full Changelog の行まで）
function Get-Section([string[]]$Lines, [string]$Heading) {
    $items = [System.Collections.Generic.List[string]]::new()
    $in = $false
    foreach ($l in $Lines) {
        if ($l -eq $Heading) { $in = $true; continue }
        if ($in -and ($l.StartsWith('#') -or $l.StartsWith('**Full Changelog**'))) { break }
        if ($in -and $l.StartsWith('- ')) { $items.Add($l) }
    }
    , $items.ToArray()
}

function Get-LastLine([string[]]$Lines) { @($Lines | Where-Object { $_.Trim() })[-1] }

# 項目を 1 つ確かめる。$Body は問題が無ければ何も返さず、あれば理由の文字列を返す。
function Test-Item([int]$Number, [string]$Desc, [scriptblock]$Body) {
    try { $reason = (& $Body) -join ' / ' }
    catch { $reason = "例外: $($_.Exception.Message)" }
    if ($reason) {
        Write-Output "NG $Number $Desc — $reason"
        $ng.Add("$Number $Desc")
    }
    else { Write-Output "ok $Number $Desc" }
}

# release-notes.ps1 の実行が失敗していれば、その理由を返す。
function Get-RunFailure($Run) {
    if ($Run.Exit -ne 0) { "release-notes.ps1 が終了コード $($Run.Exit) で終わった: $($Run.Output)" }
}

try {
    # --- 一時のリポジトリ ---
    New-Item -ItemType Directory -Force -Path $repoDir | Out-Null
    & git init -q -b main $repoDir 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'git init が失敗した' }
    Invoke-Git config user.name release-test | Out-Null
    Invoke-Git config user.email release-test@example.invalid | Out-Null
    Invoke-Git config commit.gpgsign false | Out-Null
    Invoke-Git config tag.gpgsign false | Out-Null
    Invoke-Git config core.autocrlf false | Out-Null
    $cargoPath = Join-Path $repoDir 'Cargo.toml'
    $cargoText = "[workspace]`nmembers = []`n`n[workspace.package]`nversion = `"0.0.2`"`n"
    Write-Utf8 $cargoPath $cargoText
    Invoke-Git add Cargo.toml | Out-Null
    # 初版（v0.0.1、前のタグが無い）の分
    Add-Commit 'chore: 最初のコミット'
    Add-Commit 'feat(first): 最初の機能 (#10)'
    Invoke-Git tag v0.0.1 | Out-Null

    # v0.0.1..v0.0.2（古い順）
    Add-Commit 'feat(scene-name-alias): 別名 (#1)'
    Add-Commit 'fix(host_abi)!: 戻り先の文脈を変える (#2)'
    Add-Commit 'feat!(failure-output): 失敗表記を出す (#5)'
    Add-Commit 'feat(crate-publish-ready): 公開の準備 (#3)'
    Add-Commit 'fix: 落ちる不具合を直す (#6)'
    Add-Commit 'docs(manual): 手引きを足す (#7)'
    Add-Commit 'spec(discovery): 起票する'
    Add-Commit 'docs(roadmap): 道筋を直す'
    Add-Commit 'chore(kiro-skills): スキルを更新する'
    Add-Commit 'chore(kiro): 設定を更新する'
    Add-Commit 'docs(release-workflow): 手順を書く'
    Add-Commit '接頭辞の無い件名'
    Add-Commit 'chore(release): v0.0.2'
    Invoke-Git tag v0.0.2 | Out-Null
    $sha = (Invoke-Git rev-parse 'v0.0.2^{commit}' | Select-Object -First 1).ToString().Trim()

    # タグの無い版（v0.0.3）の分
    Add-Commit 'feat(after): 後の機能 (#9)'

    $hlPath = Join-Path $repoDir 'doc/release-notes/v0.0.2.md'
    Write-Utf8 $hlPath $Highlights

    # --- 詳しいリリースノート（v0.0.2）---
    $notesPath = Join-Path $tmp 'notes.md'
    $run = Invoke-Script release-notes.ps1 @('-Tag', 'v0.0.2', '-Repo', $Repo, '-OutFile', $notesPath, '-WorkspaceRoot', $repoDir)
    $notes = Read-Lines $notesPath
    $text = $notes -join "`n"

    Test-Item 1 'feat(scene-name-alias): 別名 (#1) が「- 別名 (#1)」で新機能に載り、スコープの文字列が出力に無い' {
        Get-RunFailure $run
        if ((Get-Section $notes '## ✨ 新機能') -notcontains '- 別名 (#1)') { '「## ✨ 新機能」の下に「- 別名 (#1)」が無い' }
        if ($text.Contains('scene-name-alias')) { '出力に scene-name-alias が残っている' }
    }

    Test-Item 2 'fix(host_abi)!: … と feat!(failure-output): …（`!` がスコープの前）が「動きが変わる変更」に載り、ほかの見出しに重ならない' {
        Get-RunFailure $run
        $breaking = Get-Section $notes '## ⚠️ 動きが変わる変更'
        foreach ($item in '- 戻り先の文脈を変える (#2)', '- 失敗表記を出す (#5)') {
            if ($breaking -notcontains $item) { "「## ⚠️ 動きが変わる変更」の下に「$item」が無い" }
            foreach ($h in $Headings | Select-Object -Skip 1) {
                if ((Get-Section $notes $h) -contains $item) { "「$h」にも「$item」が載っている" }
            }
        }
        foreach ($s in 'host_abi', 'failure-output') { if ($text.Contains($s)) { "出力に $s が残っている" } }
    }

    Test-Item 3 '内部の作業（spec(discovery)・docs(roadmap)・chore(kiro-skills)・chore(kiro)・docs(release-workflow)・chore(release)）が載らない' {
        Get-RunFailure $run
        foreach ($s in '起票する', '道筋を直す', 'スキルを更新する', '設定を更新する', '手順を書く', 'discovery', 'roadmap', 'kiro', 'release-workflow', 'chore(release)', 'spec(') {
            if ($text.Contains($s)) { "出力に「$s」がある" }
        }
        if (@($notes | Where-Object { $_.StartsWith('- ') -and $_.Contains('v0.0.2') })) { '「v0.0.2」の項目が載っている' }
    }

    Test-Item 4 'spec 名のスコープ（feat(crate-publish-ready): 公開の準備 (#3)）は新機能に載り、スコープの文字列が出力に無い' {
        Get-RunFailure $run
        if ((Get-Section $notes '## ✨ 新機能') -notcontains '- 公開の準備 (#3)') { '「## ✨ 新機能」の下に「- 公開の準備 (#3)」が無い' }
        if ($text.Contains('crate-publish-ready')) { '出力に crate-publish-ready が残っている' }
    }

    Test-Item 5 '接頭辞の無い件名が「そのほか」に、そのまま載る' {
        Get-RunFailure $run
        if ((Get-Section $notes '## 🔧 そのほか') -notcontains '- 接頭辞の無い件名') { '「## 🔧 そのほか」の下に「- 接頭辞の無い件名」が無い' }
    }

    Test-Item 6 '空の見出しが出ず、見出しの順が決まりどおりで、1 行目に件数があり、末尾が compare/v0.0.1...v0.0.2 のリンクである' {
        Get-RunFailure $run
        if ($notes.Count -eq 0) { 'ノートが無い'; return }
        if ($notes[0] -ne "# shiori4 v0.0.2 の詳しいリリースノート（$ListedCount 件）") { "1 行目が違う: $($notes[0])" }
        foreach ($h in @($notes | Where-Object { $_.StartsWith('## ') })) {
            if ((Get-Section $notes $h).Count -eq 0) { "変更の無い見出し「$h」が出ている" }
        }
        $known = @($notes | Where-Object { $Headings -contains $_ } | ForEach-Object { [array]::IndexOf($Headings, $_) })
        if (($known -join ',') -ne (($known | Sort-Object) -join ',')) { '見出しの順が決まりと違う' }
        $last = Get-LastLine $notes
        if ($last -ne "**Full Changelog**: https://github.com/$Repo/compare/v0.0.1...v0.0.2") { "末尾の行が違う: $last" }
    }

    Test-Item 7 'タグの無い版（-Tag v0.0.3 -Head HEAD）で、終了コード 0 で同じ形の一覧が出る' {
        $p = Join-Path $tmp 'notes-head.md'
        $r = Invoke-Script release-notes.ps1 @('-Tag', 'v0.0.3', '-Head', 'HEAD', '-Repo', $Repo, '-OutFile', $p, '-WorkspaceRoot', $repoDir)
        if ($r.Exit -ne 0) { "終了コード $($r.Exit): $($r.Output)"; return }
        $l = Read-Lines $p
        if ($l[0] -ne '# shiori4 v0.0.3 の詳しいリリースノート（1 件）') { "1 行目が違う: $($l[0])" }
        $feat = Get-Section $l '## ✨ 新機能'
        if ($feat -notcontains '- 後の機能 (#9)') { '「## ✨ 新機能」の下に「- 後の機能 (#9)」が無い' }
        if ($feat -contains '- 別名 (#1)') { 'v0.0.2 より前の変更が載っている' }
        $last = Get-LastLine $l
        if ($last -ne "**Full Changelog**: https://github.com/$Repo/compare/v0.0.2...v0.0.3") { "末尾の行が違う: $last" }
    }

    Test-Item 8 '前のタグが無い版（v0.0.1）で全履歴が載り、**Full Changelog** の行が無い' {
        $p = Join-Path $tmp 'notes-first.md'
        $r = Invoke-Script release-notes.ps1 @('-Tag', 'v0.0.1', '-Repo', $Repo, '-OutFile', $p, '-WorkspaceRoot', $repoDir)
        if ($r.Exit -ne 0) { "終了コード $($r.Exit): $($r.Output)"; return }
        $l = Read-Lines $p
        if ($l[0] -ne '# shiori4 v0.0.1 の詳しいリリースノート（2 件）') { "1 行目が違う: $($l[0])" }
        if ((Get-Section $l '## ✨ 新機能') -notcontains '- 最初の機能 (#10)') { '「## ✨ 新機能」の下に「- 最初の機能 (#10)」が無い' }
        if ((Get-Section $l '## 🔧 そのほか') -notcontains '- 最初のコミット') { '「## 🔧 そのほか」の下に「- 最初のコミット」が無い' }
        if (($l -join "`n").Contains('別名')) { 'v0.0.1 より後の変更が載っている' }
        if (@($l | Where-Object { $_.StartsWith('**Full Changelog**') })) { '**Full Changelog** の行がある' }
    }

    Test-Item 9 '-BodyFile の本文が「見どころ（CRLF を LF に、末尾の空白を落とす）＋ 区切り ＋ 詳しいリリースノートの全文」と一致する' {
        $p = Join-Path $tmp 'notes-body.md'
        $b = Join-Path $tmp 'body.md'
        # CRLF と末尾の空白を持つ見どころ（実際のファイルが CRLF でも本文は LF になる）
        Write-Utf8 $hlPath (($Highlights -replace "`n", "`r`n") + "  `t`r`n")
        try { $r = Invoke-Script release-notes.ps1 @('-Tag', 'v0.0.2', '-Repo', $Repo, '-OutFile', $p, '-BodyFile', $b, '-WorkspaceRoot', $repoDir) }
        finally { Write-Utf8 $hlPath $Highlights }
        if ($r.Exit -ne 0) { "終了コード $($r.Exit): $($r.Output)"; return }
        if (-not (Test-Path -LiteralPath $b)) { '本文のファイルが無い'; return }
        $full = [IO.File]::ReadAllText($p)
        if ($full -cne ($notes -join "`n")) { '-BodyFile と一緒に書いたノートが、単独で書いたノートと違う' }
        $expected = $Highlights.TrimEnd() + "`n`n---`n`n" + $full
        $actual = [IO.File]::ReadAllText($b)
        if ($actual -cne $expected) { "本文が違う:`n$actual" }
    }

    $vt = @('-Tag', 'v0.0.2', '-Sha', $sha, '-MainRef', 'main', '-WorkspaceRoot', $repoDir)

    Test-Item 10 'verify-tag.ps1 が、そろっている → 0、見どころのファイルが無い → 1、空白だけ → 1' {
        $r = Invoke-Script verify-tag.ps1 $vt
        if ($r.Exit -ne 0) { "そろっている: 終了コード $($r.Exit): $($r.Output)" }
        elseif (-not $r.Output.Contains('タグの検査: 成功')) { "そろっている: 成功の行が無い: $($r.Output)" }
        try {
            Remove-Item -LiteralPath $hlPath
            $r = Invoke-Script verify-tag.ps1 $vt
            if ($r.Exit -ne 1 -or -not $r.Output.Contains('見どころのファイル')) { "無い: 終了コード $($r.Exit)（1 のはず）: $($r.Output)" }
            Write-Utf8 $hlPath " `n`t`n  `n"
            $r = Invoke-Script verify-tag.ps1 $vt
            if ($r.Exit -ne 1 -or -not $r.Output.Contains('見どころのファイル')) { "空白だけ: 終了コード $($r.Exit)（1 のはず）: $($r.Output)" }
        }
        finally { Write-Utf8 $hlPath $Highlights }
    }

    Test-Item 11 'verify-tag.ps1 が、Cargo.toml の版が違う → 1、main から到達できないコミット → 1、v0.0.2-rc.1 → 1' {
        try {
            Write-Utf8 $cargoPath ($cargoText -replace '0\.0\.2', '0.0.9')
            $r = Invoke-Script verify-tag.ps1 $vt
            if ($r.Exit -ne 1 -or -not $r.Output.Contains('ワークスペースの版の一致')) { "版が違う: 終了コード $($r.Exit)（1 のはず）: $($r.Output)" }
        }
        finally { Write-Utf8 $cargoPath $cargoText }
        # 親の無いコミット（どのブランチにも載らない）
        $orphan = (Invoke-Git commit-tree 'HEAD^{tree}' -m 'main に無いコミット' | Select-Object -First 1).ToString().Trim()
        $r = Invoke-Script verify-tag.ps1 @('-Tag', 'v0.0.2', '-Sha', $orphan, '-MainRef', 'main', '-WorkspaceRoot', $repoDir)
        if ($r.Exit -ne 1 -or -not $r.Output.Contains('main からの到達性')) { "到達できない: 終了コード $($r.Exit)（1 のはず）: $($r.Output)" }
        $r = Invoke-Script verify-tag.ps1 @('-Tag', 'v0.0.2-rc.1', '-Sha', $sha, '-MainRef', 'main', '-WorkspaceRoot', $repoDir)
        if ($r.Exit -ne 1 -or -not $r.Output.Contains('タグの形式')) { "v0.0.2-rc.1: 終了コード $($r.Exit)（1 のはず）: $($r.Output)" }
    }

    Test-Item 12 '定型コマンド D が、決まりどおりで 4 行とも合格、材料に無い番号で 2 行目、動きが変わる点の抜けで 3 行目、spec 名で 4 行目、空白だけで 4 行とも不合格' {
        $d = Join-Path $PSScriptRoot '../../../.claude/skills/release/scripts/highlights-check.ps1'
        New-Item -ItemType Directory -Force -Path (Join-Path $repoDir '.kiro/specs/demo-spec') | Out-Null
        $cases = @(
            @{ Name = '決まりどおり'; Text = $Highlights; Expect = @('合格', '合格', '合格', '合格') }
            @{ Name = '材料に無い番号'; Text = "$Highlights### 無い変更 (#99)`n"; Expect = @('合格', '不合格', '合格', '合格') }
            @{ Name = '動きが変わる点の抜け'; Text = "## 今回の見どころ`n`n### 別名が書けます (#1)`n"; Expect = @('合格', '合格', '不合格', '合格') }
            @{ Name = 'spec 名'; Text = "${Highlights}demo-spec で作りました。`n"; Expect = @('合格', '合格', '合格', '不合格') }
            @{ Name = '空白だけ'; Text = " `n`n"; Expect = @('不合格', '不合格', '不合格', '不合格') }
        )
        try {
            foreach ($c in $cases) {
                Write-Utf8 $hlPath $c.Text
                # D は作業ディレクトリ（一時のリポジトリ）から見どころと .kiro/specs/ を読む
                $out = @(& $Pwsh -NoProfile -WorkingDirectory $repoDir -File $d -Version 0.0.2 2>&1 | ForEach-Object { "$_" })
                $got = @($out | ForEach-Object { ($_ -split ' \| ')[-1] })
                if (($got -join ',') -cne ($c.Expect -join ',')) { "$($c.Name): $($out -join ' ／ ')" }
            }
        }
        finally { Write-Utf8 $hlPath $Highlights }
    }
}
finally {
    [Console]::OutputEncoding = $savedEncoding
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Recurse -Force }
}

if ($ng.Count -gt 0) {
    Write-Output ''
    Write-Output "NG の項目（$($ng.Count) 件）:"
    $ng | ForEach-Object { Write-Output "- $_" }
    exit 1
}
exit 0
