# リリースの手順（段 1〜9）

`SKILL.md` の「段」が指す、実行するコマンド・合否の基準・判定表である。コマンドは、ここに書いてあるものを、波かっこ（`{V}` など）だけ置き換えて実行する。

かっこの中の番号（1.4 など）と、各段の表の `Requirements` は、もとになった要件（`.kiro/specs/completed/release-workflow/requirements.md`）の番号である。実行には使わない。

## 実行変数

段の間で受け渡す値。コマンドの中の `{V}` などの波かっこは、この値に置き換える。文脈を失ったら、右の方法で取り直す。どの値もファイルに書かない。

| 変数 | 意味 | 取り直し方 |
|------|------|------------|
| `VREQ` | 開発者が指定した版（無いこともある。先頭の `v` は外す） | 開発者の指示 |
| `V_MAIN` | `origin/main` の `Cargo.toml` の版 | 定型コマンド A の `main` の行 |
| `V` | 今回リリースする版（`v` を付けない） | 段 3 の手順 5。再開のときは `V_MAIN` |
| `BRANCH` | 作業ブランチ | `git rev-parse --abbrev-ref HEAD` |
| `PR` | 版の更新の PR の番号 | `gh pr list --state all --head {BRANCH} --json number,title,state` |
| `TARGET` | タグを付けるコミットの SHA | 段 6 の手順 1 |
| `RUN` | リリース CI の実行の ID | 定型コマンド A の `run` の行 |
| `ATTEMPT` | 待つ対象の試行の番号（新しい実行は 1。再実行のたびに 1 増える） | 定型コマンド A の `run` の行の `attempt` |
| `N_AUTO` | この作業の中で行った自動の再実行の回数（初期値 0） | 取り直せない。再実行（`/release`）で 0 に戻る |
| `F7_DONE` | 判定表 F7 の切り分けの再実行を行ったか（初期値 いいえ） | 同上 |

## 段 1: 開始の確認

| Field | Detail |
|-------|--------|
| Intent | リリースと関係のない変更を巻き込まない状態から始める |
| Requirements | 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 12.5 |

手順 1 と 2 は何も変えない。どちらかで不合格なら、何も変えずに止まる。

1. **ブランチ**（7.1）

   ```bash
   git rev-parse --abbrev-ref HEAD
   ```

   出力が `main` か `HEAD` なら何も変えずに止まり、ハーネスのワークツリーでの再実行を求める。それ以外を `BRANCH` とする。

2. **必要な操作**（7.2・7.3）。4 つとも終了コード 0 で、2 つ目の出力が `ADMIN true`・`MAINTAIN true`・`WRITE true` のどれかであること。

   ```bash
   gh auth status
   gh repo view --json viewerPermission,squashMergeAllowed -q '.viewerPermission + " " + (.squashMergeAllowed|tostring)'
   gh run list --workflow release.yml --limit 1 --json databaseId
   git ls-remote --tags origin 'refs/tags/v*'
   ```

   3 つ目は、`release.yml` がまだ main に無い（初めてのリリースの前）とき、終了コードが 0 でなく出力に `workflow release.yml not found` が出る。このときはリリース CI の閲覧ができないのではないので、合格とする。それ以外の失敗は不合格である。

   不合格なら、できない操作（上から順に: GitHub の認証、PR の統合の権限と squash の許可、リリース CI の閲覧、リモートのタグの読み取り）を示して止まる。

3. **作業ツリー**（7.4）。`git status --porcelain` の出力が空で、`git merge-tree` の出力と `git rev-parse` の出力が同じであること。

   ```bash
   git fetch origin main
   git status --porcelain
   git merge-tree --write-tree origin/main HEAD
   git rev-parse 'origin/main^{tree}'
   ```

   `git merge-tree` は、作業ブランチを main に取り込んだ結果のツリーを、何も変えずに計算する。main のツリーと同じなら、作業ブランチは main に無い内容を持たない（squash で統合済みのコミットは、内容が同じなので通る）。不合格なら、下の「版の更新のコミットだけが残っている場合」にあたるかを確かめる。あたらなければ、未コミットの変更と main に無い内容をリリースに含めず、次の出力を示して止まる。

   ```bash
   git status --short
   git log --oneline --no-merges origin/main..HEAD
   git diff --stat origin/main...HEAD
   ```

   **版の更新のコミットだけが残っている場合**（12.5。段 4 の後、PR を作る前に会話が終わった場合）。`git status --porcelain` の出力が空で、次の 2 つの出力がどちらも基準どおりなら、止まらずに続ける。

   ```bash
   git log --no-merges --format=%s origin/main..HEAD
   git diff --name-only origin/main...HEAD
   ```

   - 1 つ目が 1 行だけで、`chore(release): vX.Y.Z`（`X.Y.Z` は数字 3 つ）の形である。
   - 2 つ目が、段 4 の 2 ファイル（`Cargo.toml` と、見どころのファイル `doc/release-notes/v{V}.md`）だけである。

   続け方: 件名の `X.Y.Z` を `V` として開発者に示す。`VREQ` があって `V` と違うときは、止まって開発者に確かめる。手順 4（main の取り込み）を行い、段 2 を飛ばして、次の 3 つをやり直す。

   - 段 3 の手順 4（重複の検査。定型コマンド A を `-Version {V}` で実行する）
   - 版の更新の確認: 次の 1 つ目の出力が `3`、2 つ目の出力が `Cargo.toml` の 1 行だけで、追加と削除がどちらも `3` であること

     ```bash
     grep -cF 'version = "{V}"' Cargo.toml
     git diff --numstat origin/main...HEAD -- Cargo.toml
     ```

   - 定型コマンド D（`-Version {V} -Head origin/main`）。4 行とも `合格`

   どれも合格なら、段 5 から続ける。不合格なら止まる。

4. **main の取り込み**（7.5・7.6）

   ```bash
   git merge-base --is-ancestor origin/main HEAD
   ```

   終了コード 0 なら何もしない。1 なら、版の更新より前に、履歴を書き換えずに取り込む。

   ```bash
   git merge --no-edit origin/main
   ```

   取り込みが失敗したら、衝突したファイルを `git diff --name-only --diff-filter=U` で控えてから `git merge --abort` を実行し、そのファイルを示して止まる。`git rebase`・`git reset` を使わない。

## 段 2: 状態の判定

| Field | Detail |
|-------|--------|
| Intent | 実際の状態から、どの段から始めるかを決める |
| Requirements | 12.1, 12.2, 12.3, 12.4, 12.6, 12.7, 14.2 |

定型コマンド A を、版を指定せずに（`origin/main` の版を調べる）実行し、出力を次の表に上から当てる。最初に当てはまった行に従う。公開済みは `集計` の行の `n/4`（4 = crates.io の 3 クレート + GitHub Release）である。

| # | `tag` | `集計` の公開済み | `run` | ほかの条件 | 判定 | 次の手 |
|---|-------|-------------------|-------|------------|------|--------|
| S0 | — | — | — | `不明` が 1 以上、または `main` の行が `不明`（`Cargo.toml` の 3 か所の版が食い違う） | 判定できない | 止まる。`不明` の行を示す（12.6） |
| S1 | あり | 4/4 | — | `VREQ` が `V_MAIN` と同じ | 指定された版はリリース済み | エラーを示し、別の版の入力を求める（12.2）。入力されたら段 3 の手順 3 へ |
| S2【初版のみ】 | 無し | crates.io の 3 行が `あり`、`GitHub Release` が `無し` | — | `V_MAIN` が `0.0.1` | 初版の状態（14.2） | 節「初版の経路【初版のみ】」へ |
| S3 | あり | 4/4 | — | 上のどれでもない | リリース済み（12.2） | 段 3 から始める |
| S4 | あり | 3/4 以下 | `queued`・`in_progress`・`waiting`・`requested`・`pending` | — | リリース CI が実行中（12.3） | `V` = `V_MAIN` を示し、段 7 の手順 2 から |
| S5 | あり | 3/4 以下 | `completed` | — | リリース CI が未完了で終わっている（12.3） | `V` = `V_MAIN` を示し、段 7 の手順 3 から |
| S6 | あり | 3/4 以下 | 無し | — | タグはあるが、リリース CI の実行が無い | 止まる。起動していないことを示す（12.3） |
| S7 | 無し | 0/4 | — | `VREQ` が無いか `V_MAIN` と同じ | 版の更新は main にあり、タグが無い（12.4） | `V` = `V_MAIN` を示し、段 6 から |
| S8 | 無し | 0/4 | — | `VREQ` が `V_MAIN` と違う | main に未リリースの版があるのに、別の版が指定された | 止まる。`V_MAIN` を先に出すか、飛ばすかを開発者に確かめる（12.4・12.7） |
| S9 | 無し | 上以外 | — | — | タグが無いのに公開済みのものがある | 止まる。公開済みの行を示し、開発者に確かめる（12.6） |

- S4・S5 で `VREQ` が `V_MAIN` と違うときは、`V_MAIN` のリリースが未完了なので先に続きを行うこと、`VREQ` は始めていないことを開発者に示す（12.7）。
- 判定は毎回やり直す。前回の実行の会話の記録を使わず、定型コマンド A の出力だけで決める（12.1）。
- どの行にも当たらない状態は無い（S9 が残りを受ける）。推測で進めない（12.6）。

## 段 3: 版の決定

| Field | Detail |
|-------|--------|
| Intent | すでに出ている版と重ならない版を確定する |
| Requirements | 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7 |

1. `VREQ` があれば、先頭の `v` を外してそれを候補にし、手順 3 へ進む（8.1）。
2. `VREQ` が無ければ、段 2 で得た定型コマンド A の出力（出どころごとの `最大` と、`提案` の行）を開発者に示し、`提案` の行の版を候補にする（8.2）。承認を求めない。開発者の答えを待たずに、手順 3 へ進む。候補にするのは `提案` の行の版（PATCH + 1）だけで、MINOR・MAJOR を自動では上げない（8.3）。`提案` の行が `出せない` のときは、`不明` の行を示して止まる（8.6）。
3. 候補が次の形かを確かめる（8.4）。`False` ならエラーを示し、入力し直しを求める。先頭の `v` は外してから確かめる。

   ```powershell
   '{候補}' -cmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z'
   ```

4. 定型コマンド A を `-Version {候補}` で実行し、次のすべてを満たすことを確かめる。
   - `集計` の `不明` が 0（満たさなければ、「無い」と見なさず、`不明` の行を示して止まる。8.6）
   - `tag` が `無し`、`集計` の公開済みが 0/4、`GitHub Release` が `無し`（満たさなければ、すでにある出どころを示し、別の版の入力を求めて手順 3 へ戻る。8.5）
   - `PR` が `無し`（`未統合` の行があれば、新しい PR を作らず、その PR を示して開発者に扱いを確かめる。8.7）
5. 候補を `V` に確定し、開発者に示す。

## 定型コマンド A の出力（状態の照会）

1 行に 1 つの事実を、` | ` で区切って出す。値の語は固定である。

| 行 | 形 | 値 |
|----|----|----|
| `fetch` | `fetch \| origin/main \| <結果>` | `成功`・`不明` |
| `main` | `main \| Cargo.toml \| <版>` | `[workspace.package]` の `version` の値。`[workspace.dependencies]` の `shiori4_abi`・`shiori4` の `version` と合わせた 3 か所が同じでなければ `不明` |
| `照会する版` | `照会する版 \| <V>` | — |
| `tag` | `tag \| 最大 <版> \| v{V} <状態>` | `あり <コミットの SHA>`・`無し`。照会できなければ `tag \| 不明` |
| `crates.io` | `crates.io \| <クレート> \| 最大 <版> \| {V} <状態>`（`shiori4_abi`・`shiori4`・`shiori4_host` の 3 行） | `あり`・`無し`・`不明（HTTP <コード>）`。一度も公開していないクレートは `最大 無し` |
| `GitHub Release` | `GitHub Release \| 最大 <版> \| v{V} <状態>` | `あり（公開） <URL>`・`下書き`・`無し`・`不明` |
| `run` | `run \| <ID> \| <status> \| <conclusion> \| attempt <n> \| <URL>` | 無ければ `run \| 無し`。照会できなければ `run \| 不明` |
| `PR` | `PR \| 未統合 \| #<番号> \| <ブランチ> \| <URL>` | 件名が `chore(release): v{V}` の open な PR。無ければ `PR \| 無し` |
| `集計` | `集計 \| {V} の公開済み <n>/4 \| 不明 <m>` | 4 = 3 クレート + GitHub Release |
| `提案` | `提案 \| <版>（すべての出どころの最大 <版> の PATCH + 1）` | `不明` があれば `提案 \| 出せない（不明がある）` |

「公開済み」に数えるのは、crates.io の `あり` と、GitHub Release の `あり（公開）` だけである。下書きは数えない。

## 定型コマンド B の出力（リリース CI の結果）

| 行 | 形 |
|----|----|
| `run` | `run \| <ID> \| <status> \| <conclusion> \| attempt <n> \| <URL>` |
| `job` | `job \| <job 名> \| <結論>`（`gate` の中の job は `gate / <名前>`） |
| `step` | `step \| <job 名> \| <step 名> \| <結論> \| <所要> 秒`（`Publish shiori4*` には ` \| 期限まで <秒> 秒` が付く。1800 から、その step の完了と `Auth crates.io` の完了の差を引いた秒数） |
| `line` | `line \| attempt <n> \| <job 名> \| <step 名> \| status=<値> [reason=<値>] [url=<値>]` |

対象の job は `publish-crates`・`github-release`、対象の step は `Auth crates.io`・`Publish <クレート>`・`Create GitHub Release` である。

## 定型コマンドの呼び出し方

定型コマンドは、`.claude/skills/release/scripts/` のスクリプトである。4 本とも読むだけで、何も変えない。リポジトリのルートを作業ディレクトリにして、PowerShell ツールで次の 1 行をそのまま実行する（波かっこだけを置き換える）。スクリプトの中身を写して実行しない。

| 定型コマンド | 呼び出し | 出力の形 |
|--------------|----------|----------|
| A: 状態の照会 | `& "$PWD/.claude/skills/release/scripts/release-state.ps1" -Version {V}` | 上の「定型コマンド A の出力」 |
| A（`origin/main` の版を調べる） | `& "$PWD/.claude/skills/release/scripts/release-state.ps1"` | 同上 |
| B: リリース CI の結果の読み取り | `& "$PWD/.claude/skills/release/scripts/ci-result.ps1" -Run {RUN}` | 上の「定型コマンド B の出力」 |
| C: タグを付ける前の検査 | `& "$PWD/.claude/skills/release/scripts/pretag-check.ps1" -Version {V} -Target {TARGET}` | 4 行（`タグの形`・`Cargo.toml の版`・`main からの到達`・`見どころ`）。それぞれ `合格` か `不合格` |
| D: 見どころの確認 | `& "$PWD/.claude/skills/release/scripts/highlights-check.ps1" -Version {V} -Head origin/main` | 4 行（`見どころのファイル`・`PR の番号`・`動きが変わる点`・`spec 名`）。それぞれ `合格` か `不合格`。ファイルが無いか空なら 4 行とも `不合格` |

- A は、crates.io の照会に User-Agent を付ける。200・404 以外の応答と、コマンドの失敗を `不明` にする。一度も公開していないクレート（クレートの照会が 404）と、`release.yml` がまだ main に無いこと（`workflow release.yml not found`）は `不明` にせず `無し` とする。
- B は、実行が `completed` になってから使う。
- C は、`.github/scripts/release/verify-tag.ps1` と同じ 4 つの条件を、タグを付けるコミットの中身（`git show`）で確かめる。先に `git fetch origin main` を済ませる。
- D は、作業ディレクトリの `doc/release-notes/v{V}.md` を、`.github/scripts/release/release-notes.ps1` の出力（材料）と、`.kiro/specs/`・`.kiro/specs/completed/` のフォルダの名前に照らして確かめる。タグの在る版を確かめるときは、`-Head origin/main` を省く。
