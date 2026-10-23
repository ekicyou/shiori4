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

   `git merge-tree` は、作業ブランチを main に取り込んだ結果のツリーを、何も変えずに計算する。main のツリーと同じなら、作業ブランチは main に無い内容を持たない（squash で統合済みのコミットは、内容が同じなので通る）。不合格なら、下の「版の更新のコミットだけが残っている場合」にあたるかを確かめる。あたらなければ、未コミットの変更と main に無い内容をリリースに含めず、次の出力を示して止まる。止まったら、main の先頭から作った新しい作業ブランチ（別のハーネスのワークツリー）で `/release` を実行し直すよう示す（spec の作業ブランチのように、リリースと関係のないコミットを持つブランチでは始められない）。

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
| Requirements | 12.1, 12.2, 12.3, 12.4, 12.6, 12.7 |

定型コマンド A を、版を指定せずに（`origin/main` の版を調べる）実行し、出力を次の表に上から当てる。最初に当てはまった行に従う。公開済みは `集計` の行の `n/4`（4 = crates.io の 3 クレート + GitHub Release）である。

| # | `tag` | `集計` の公開済み | `run` | ほかの条件 | 判定 | 次の手 |
|---|-------|-------------------|-------|------------|------|--------|
| S0 | — | — | — | `不明` が 1 以上、または `main` の行が `不明`（`Cargo.toml` の 3 か所の版が食い違う） | 判定できない | 止まる。`不明` の行を示す（12.6） |
| S1 | あり | 4/4 | — | `VREQ` が `V_MAIN` と同じ | 指定された版はリリース済み | エラーを示し、別の版の入力を求める（12.2）。入力されたら段 3 の手順 3 へ |
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
- S2 は欠番である（初版 0.0.1 のためだけの行だった。v0.0.1 を出し終えて削った。タグが無いのに公開済みのものがある状態は、初版かどうかによらず S9 で止まる）。

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

## 段 4: 版の更新

| Field | Detail |
|-------|--------|
| Intent | 版の表記の 3 か所をそろえ、見どころを書き、2 ファイルだけの 1 つのコミットにする |
| Requirements | 9.1, 9.2, 9.3, 9.4, 9.5, 9.6 |

`V_OLD` は更新の前の版（= `V_MAIN`）である。変えてよいのは、ルートの `Cargo.toml` と見どころのファイル `doc/release-notes/v{V}.md` の 2 ファイルだけである（9.4）。`Cargo.lock` は `.gitignore` の対象で、コミットしない。手元で `cargo` のコマンドを実行しない。

1. **事前の確認**（まだ何も変えない）。出力が `3` であること。違えば、何も変えずに止まり、出力を示す。

   ```bash
   grep -cF 'version = "{V_OLD}"' Cargo.toml
   ```

   3 か所は、`[workspace.package]` の `version` と、`[workspace.dependencies]` の `shiori4_abi`・`shiori4` の `version` である。

2. **版の表記**（9.1）。`Cargo.toml` の `version = "{V_OLD}"` を `version = "{V}"` に、Edit の全置換で置き換える（3 か所）。

3. **見どころを書く**（9.2・9.3）。リリースの本文の先頭になる見どころのファイル `doc/release-notes/v{V}.md` を書く。書き方の決まりは `doc/release-notes/README.md` である。

   - **H1 材料の一覧**。出力の最後の行が `**Full Changelog**: https://github.com/ekicyou/shiori4/compare/v{V_OLD}...v{V}` であること。違えば、前のタグを読めていないので、手順 5（戻し方）を行って止まる。

     ```powershell
     & "$PWD/.github/scripts/release/release-notes.ps1" -Tag v{V} -Repo ekicyou/shiori4 -Head origin/main
     ```

   - **H2 見どころのファイルを書く**。README の「書き方」に従い、H1 の一覧から見どころを選んで、`doc/release-notes/v{V}.md` を Write で作る。「動きが変わる変更」の見出しの下の項目は、すべて動きが変わる点として載せる。版・件数・リンクを書かない。一覧の件名だけで足りないときは、`gh pr view {番号} --json title,body` を読んでよい。読むだけである。
   - **H3 確認**。定型コマンド D を `-Version {V} -Head origin/main` で実行する。4 行とも `合格` であること。合格なら、手順 4 へ進む。
   - **H4 不合格のとき**。`不合格` の行の値（材料に無い番号・載っていない番号・見つかった spec 名など）を直して、H3 をやり直す。直すのは 3 回までである。3 回直しても `合格` にならなければ、手順 5（戻し方）を行い、`不合格` の行を示して止まる（9.3）。H3 が合格するまで、手順 6（コミット）を行わない。

4. **確認**（9.4・9.5）。次の 4 つをすべて満たすこと。

   ```bash
   grep -cF 'version = "{V}"' Cargo.toml
   git diff --numstat
   git status --porcelain
   ```

   | # | 確認 | 合格の基準 |
   |---|------|------------|
   | (1) | 3 か所が同じ版 | 1 つ目の出力が `3` |
   | (2) | 版の行だけを変えた | 2 つ目の出力が `Cargo.toml` の 1 行だけで、追加と削除がどちらも `3` |
   | (3) | ほかのファイルを変えていない | 3 つ目の出力が ` M Cargo.toml` と `?? doc/release-notes/v{V}.md` の 2 行だけ |
   | (4) | 見どころが約束に合う | 定型コマンド D（`-Version {V} -Head origin/main`）の 4 行とも `合格` |

   どれかが不合格なら、手順 5 を行い、不合格の確認（番号と出力）を示して止まる（9.5）。

5. **戻し方**（不合格のとき。9.3・9.5）。変えたものだけを、変えたときの逆の操作で戻す。
   - `Cargo.toml` を変えた後なら、`version = "{V}"` を `version = "{V_OLD}"` に、Edit の全置換で戻す（3 か所）。
   - `doc/release-notes/v{V}.md` を作った後なら、パスを指定して消す。

     ```bash
     rm -- doc/release-notes/v{V}.md
     git status --porcelain
     ```

   最後の出力が空であることを確かめてから止まる。空でなければ、その出力を示して止まる。`git restore`・`git checkout --`・`git clean` は使わない（危険な Git 操作の禁止に合わせる）。

6. **コミット**（9.6）

   ```bash
   git add Cargo.toml doc/release-notes/v{V}.md
   git commit -m "chore(release): v{V}"
   git show --stat --format=%s HEAD
   ```

   最後の出力が、件名 `chore(release): v{V}` と 2 ファイル（`Cargo.toml`・`doc/release-notes/v{V}.md`）であること。違えば止まり、出力を示す。その後、`doc/release-notes/v{V}.md` の全文を開発者に示す。承認を求めない。開発者の答えを待たずに、段 5 へ進む。

## 段 5: main への統合

| Field | Detail |
|-------|--------|
| Intent | 版の更新を、ほかの変更と同じ PR の流れ（squash）で main に入れる |
| Requirements | 9.7, 9.8 |

main へ直接 push しない（9.7）。

1. PR に入る変更が、段 4 の 2 ファイルだけであることを確かめてから、作業ブランチを push する（main ではない）。1 つ目の出力が `Cargo.toml` と `doc/release-notes/v{V}.md` の 2 行だけでなければ、push せずに止まる（ほかのファイルがコミットに混ざっている）。

   ```bash
   git diff --name-only origin/main...HEAD
   git push -u origin HEAD
   ```

2. PR を作る。

   ```bash
   gh pr create --base main --head {BRANCH} --title "chore(release): v{V}" --body "v{V} へ版を更新する。統合の後にリリースタグ v{V} を push し、リリース CI が公開する。"
   gh pr view {BRANCH} --json number -q .number
   ```

   2 つ目の出力を `PR` とする。

3. PR の CI を待たずに、squash で統合する（9.7）。

   ```bash
   gh pr merge {PR} --squash --delete-branch --subject "chore(release): v{V}" --body "v{V} へ版を更新する。"
   gh pr view {PR} --json state,mergeCommit -q '.state + " " + (.mergeCommit.oid // "")'
   ```

   統合の成否は、2 つ目の出力だけで判定する。`MERGED <SHA>` なら成功で、`<SHA>`（統合の結果のコミット）を `TARGET` とする。`gh pr merge` の出すローカルブランチの削除の警告（`'main' already used by worktree` など）は、失敗ではない。

   2 つ目のコマンドを実行できなかったとき（出力が得られなかったとき）だけ、代わりに次で判定する。2 つ目の出力が 1 行なら統合は成功で、その SHA を `TARGET` とする。空なら、統合を確かめられないので止まる。

   ```bash
   git fetch origin main
   git log origin/main -1 --format=%H -F --grep="chore(release): v{V}"
   ```

   `--subject` を指定するので、squash でできたコミットの件名は `chore(release): v{V}` になる。段 6 の手順 1 の検索も、この件名に合う。

4. **失敗のとき**（9.8）。手順 1〜3 のどれかが失敗した、または手順 3 の出力が `MERGED` で始まらない（代わりの判定では、出力が空の）ときは、止まる。リリースタグを作らない。`git push --force`・履歴の書き換え・統合前のブランチの削除を行わない。PR の URL（PR を作る前なら、失敗したコマンド）と失敗の内容を示し、開発者に解消を求める。

5. **main の確認**（10.2）。`git fetch origin main` の後、定型コマンド C を `-Version {V} -Target {TARGET}` で実行する。4 行とも `合格` であること。不合格なら、`不合格` の行を示して止まる。リリースタグを作らない。合格なら段 6 へ進む。

## 段 6: リリースタグの作成と push

| Field | Detail |
|-------|--------|
| Intent | 版を上げた main のコミットに注釈付きのタグを付け、タグだけを push する |
| Requirements | 10.1, 10.2, 10.3, 10.4, 10.5 |

1. **`TARGET` の決定**（10.1）。段 5 から来たときは、段 5 の手順 3 の `TARGET`（統合の結果のコミット）を使う。段 2 の S7 から来たとき（再開）は、次で求める。

   ```bash
   git fetch origin main
   git log origin/main -1 --format=%H -F --grep="chore(release): v{V}"
   ```

   出力が 1 行ならそれを `TARGET` とする。空なら止まり、版の更新のコミットが見つからないことと、`git log origin/main -5 --oneline -- Cargo.toml` の出力を示して、開発者にコミットを確かめる。

   **S7 で見どころだけが不合格のとき**。S7 から来て、手順 2 で定型コマンド C の 4 行目（`見どころ`）だけが `不合格` のときは、`git rev-parse origin/main` の出力を `TARGET` にして、定型コマンド C をやり直す。4 行とも `合格` なら、その `TARGET` で続ける（見どころのファイルが、版の更新より後のコミットで main に入っている）。なお `不合格` なら、リリースタグを作らずに止まり、`doc/release-notes/v{V}.md` が main に無いことを示す。

2. **タグを付ける前の検査**（10.2）。定型コマンド C を `-Version {V} -Target {TARGET}` で実行する。4 行とも `合格` であること。不合格なら、タグを作らずに止まり、`不合格` の行を示す（S7 の見どころだけの不合格は、手順 1 の扱い）。

3. **同名のタグ**（10.3）。2 つとも出力が空であること。

   ```bash
   git ls-remote --tags origin refs/tags/v{V}
   git tag -l v{V}
   ```

   - リモートにある（1 つ目が空でない）: エラーを示して止まり、開発者に扱いを確かめる。既存のタグを自動で削除・付け替えしない（承認を得た付け直し（11.8）だけは、段 8 で扱う）。
   - ローカルにだけある: `git rev-parse 'v{V}^{commit}'` が `TARGET` と同じなら、手順 4 を飛ばして手順 5 へ進む（前回の push の失敗の続き）。違えば、止まって開発者に確かめる。

4. **タグの作成**（10.1）。注釈付きのタグを `TARGET` に作る。

   ```bash
   git tag -a v{V} -m "Release v{V}" {TARGET}
   ```

5. **タグだけを push**（10.4）。ブランチは push しない。

   ```bash
   git push origin v{V}
   git ls-remote --tags origin refs/tags/v{V}
   ```

   2 つ目の出力が空でなければ成功で、段 7 へ進む。push が失敗したら（10.5）、失敗の内容と「main には版の更新が入っていて、タグだけが無い」状態であることを示して止まる。再実行すると、段 2 の S7 から続く。

## 段 7: リリース CI の結果の確認

| Field | Detail |
|-------|--------|
| Intent | タグで起動したリリース CI を最後まで追い、公開先ごとの結果を読む |
| Requirements | 11.1, 11.2, 11.3, 13.1 |

1. **実行の特定**（11.1）。タグの名前とタグのコミットの両方が一致する実行を、最大 3 分待って探す。次のコマンドは、ツールのタイムアウトを 10 分にして実行する。

   ```powershell
   $V = '{V}'; $sha = git rev-parse "v$V^{commit}"
   foreach ($i in 1..18) {
       $run = @((gh run list --workflow release.yml --limit 30 --json databaseId,headBranch,headSha,status,attempt,url,createdAt) -join "`n" | ConvertFrom-Json |
           Where-Object { $_.headBranch -eq "v$V" -and $_.headSha -eq $sha } | Sort-Object createdAt -Descending)
       if ($run.Count) { break }
       Start-Sleep -Seconds 10
   }
   if ($run.Count) { "run | $($run[0].databaseId) | $($run[0].status) | attempt $($run[0].attempt) | $($run[0].url)" } else { 'run | 無し' }
   ```

   `run | 無し` なら、完了と報告せず、リリース CI が起動していないことを示して止まる。それ以外は、ID を `RUN`、attempt を `ATTEMPT` とする。

2. **待機**（11.2）。次のコマンドを、ツールのタイムアウトを 10 分にして実行する。`{ATTEMPT}` は、待つ対象の試行の番号（新しい実行は 1。再実行のたびに 1 増える）。出力が `completed` になるまで繰り返す。終わるまでは完了と報告しない。20 回（約 3 時間）繰り返しても `completed` にならなければ、「リリース CI が実行中」として止まる。

   ```powershell
   $RUN = '{RUN}'; $ATTEMPT = {ATTEMPT}
   foreach ($i in 1..17) {
       $s = (gh run view $RUN --json status,attempt) -join "`n" | ConvertFrom-Json
       if ($s.attempt -ge $ATTEMPT -and $s.status -eq 'completed') { break }
       Start-Sleep -Seconds 30
   }
   "run | $RUN | $(if ($s.attempt -ge $ATTEMPT) { $s.status } else { 'waiting' }) | attempt $($s.attempt)"
   ```

3. **読み取り**（11.3）。定型コマンド B を `-Run {RUN}` で実行し、下の「結果表の作り方」で、4 つの公開先（crates.io の 3 クレートと GitHub Release）の `status`・`reason` と、GitHub Release の URL（`url=` の値）を決める。

4. **完了の判定**（13.1）。次のすべてを満たせば完了で、段 9 へ進む。
   - 定型コマンド B の `run` の行の結論が `success`
   - 結果表の 4 つがすべて `published` か `skipped`
   - 定型コマンド A（`-Version {V}`）の `集計` が、公開済み 4/4・`不明` 0（`GitHub Release` の行の URL を、報告の URL とする）

   1 つ目と 2 つ目を満たすのに 3 つ目が 4/4 でないときは、1 分後に定型コマンド A を 1 度だけ実行し直す。それでも 4/4 でなければ、未完了として止まる。1 つ目か 2 つ目を満たさないときは、段 8 へ進む。

### 結果表の作り方

公開先ごとに、定型コマンド B の `step` の行（step の結論）と `line` の行（ログの `status=` の行）を、次の対応で見る。crates.io の認証は job の先頭で 1 回だけ取り、3 クレートで共有する。

| 公開先 | 認証の step | 公開の step |
|--------|-------------|-------------|
| crates.io `shiori4_abi`・`shiori4`・`shiori4_host`（この順に公開する） | `Auth crates.io`（3 つで共有） | `Publish <クレート>` |
| GitHub Release | なし | `Create GitHub Release` |

規則は公開先ごとに上から当て、最初に当てはまったものを使う。リリース CI の集約の step（`Summarize crates.io results`）と同じ読み方である。値が書かれていない公開先を、推測で `published` にしない（11.3）。

| # | 条件 | `status` | `reason` |
|---|------|----------|----------|
| 1 | 公開の step の結論が `success` | その step の `line` のうち、attempt が最大のものの値（`published` か `skipped`）。`line` が無ければ「`published` か `skipped`（区別できない）」 | なし |
| 2 | 公開の step の結論が `failure`・`cancelled` | `failed` | その step の `line` のうち、attempt が最大のものの `reason=` の値。`line` が無ければ `publish` |
| 3 | 公開の step が走っていない（結論が `skipped` か、`step` の行が無い）。かつ、認証の step の結論が `failure` で、公開の順で前のクレートに `failed` が無い | `failed` | `auth` |
| 4 | それ以外 | `not-run`。ただし、その job の結論が `failure`・`cancelled`・`timed_out` で、その job のどの公開先も `failed` でないときは、その job の先頭の公開先を `failed` にする | 読み替えたものは `不明` |

- 規則 3 は、認証の失敗を 1 か所（最初に走らなかったクレート）にだけ付ける。後のクレートは規則 4 で `not-run` になる。
- 「区別できない」は、完了の判定では `published` か `skipped` として扱う。報告には「区別できない」と書く。
- `not-run` は、上流の失敗の結果である。それ自体は対応の対象にしない。

## 段 8: 失敗への対応

| Field | Detail |
|-------|--------|
| Intent | 失敗の種類ごとに、再実行・案内・停止を判定表で決める |
| Requirements | 11.4, 11.5, 11.6, 11.7, 11.8, 11.9, 11.10, 11.11, 13.4 |

段 7 の結果表と、定型コマンド B の `job` の行を、次の表に上から当てる。最初に当てはまった行に従う。「手順書」は `.github/release-ci-setup.md` である。

| # | 条件 | 区分 | 次の手 |
|---|------|------|--------|
| F0 | 実行の結論が `cancelled` | 取り消された（11.11） | 再実行しない。取り消されていることを示して止まる |
| F1 | job `verify` か、名前が `gate` で始まる job のどれかが `failure`・`cancelled`・`timed_out` | 公開前の段の失敗（11.7） | 再実行しない。定型コマンド A で 0/4 を確かめ、失敗した job と step の名前、失敗のログの末尾、「どの公開先にも公開していない」を報告して止まる（13.4）。0/4 でないときは、「公開していない」と書かず、公開済みの行を示して止まる。続きは「公開前の段の失敗の続き」 |
| F3 | どれかが `failed`・`auth` | セットアップ（11.5） | 先に下の「トークンの期限切れの見分け」を行う。期限切れなら F6 に従う。そうでなければ再実行しない。`shiori4_abi` か、認証の step（`Auth crates.io`）の失敗なら手順書の「名前の対応表」を、2 つ目以降のクレート（`shiori4`・`shiori4_host`）だけなら「2 つ目以降のクレートの認証だけが失敗したとき」を示して止まる。続きは「セットアップを直した後の続き」 |
| F4 | どれかが `failed`・`not-registered` | セットアップ（11.5） | 再実行しない。手順書の「新しいクレートの初回公開」を示して止まる。続きは F3 と同じ |
| F6 | どれかが `failed`・`transient` | 一時的な失敗（11.4） | `N_AUTO` が 3 未満なら、失敗した job を再実行する。3 なら、自動の再実行をやめ、未完了として止まる |
| F7 | どれかが `failed`・`publish`、または `failed`・`不明` | 切り分け（11.10） | `F7_DONE` が「いいえ」で `N_AUTO` が 3 未満なら、`F7_DONE` を「はい」にして、失敗した job を 1 度だけ再実行する。そうでなければ「再実行では直らない失敗」 |

F2・F5 は欠番である（番号を移植元にそろえた。F2 は CI での初回のリリースだけの行、F5 は Release の添付の行で、どちらもこのリポジトリには無い）。

**トークンの期限切れの見分け**（F3）。crates.io の認証は 1 回だけで、トークンは 30 分で切れる。`failed`・`auth` のクレートの `Publish <クレート>` の `step` の行の `期限まで <秒> 秒` が 0 以下なら、認証の設定ではなくトークンの期限切れである。F6 と同じく、失敗した job を再実行する（新しいトークンを取り直して続く）。`期限まで - 秒` のとき、その `step` の行が無いとき（認証の step の失敗）、1 以上のときは期限切れではなく、F3 のまま止まる。

**失敗した job の再実行**（F6・F7）

```bash
gh run rerun {RUN} --failed
```

終了コード 0 なら `N_AUTO` を 1 増やし、`{ATTEMPT}` を 1 増やして、段 7 の手順 2 へ戻る。再実行は、公開済みのものを飛ばす。自動の再実行は、1 回の `/release` の中で合わせて 3 回までである（11.4）。

**失敗のログの末尾**（F1・F7 の報告に付ける）

```bash
gh run view {RUN} --log-failed | tail -n 60
```

**再実行では直らない失敗**（F7 の 2 度目。11.10）。定型コマンド A（`-Version {V}`）を実行する。

- 公開済みが 0/4 で、`GitHub Release` が `無し`: F1 と同じ報告をして止まる（何も公開していない。修正の後にタグを付け直せる。11.7）。
- それ以外: タグを付け直さない。公開済みの公開先と未公開の公開先を示し、版を上げて出し直す必要があることを報告して止まる（11.9）。

**セットアップを直した後の続き**（11.6）。開発者が設定を直したと伝えたら、同じ会話なら上の「失敗した job の再実行」から続ける。新しい会話なら、`/release` の再実行で段 2 の S5 に入り、同じ失敗が残っていれば、また F3・F4 で止まる。そのときは、開発者の「直した」という指示を受けて、失敗した job を再実行する。

**公開前の段の失敗の続き**（11.7・11.8）。開発者が次のどちらかを指示する。

- 「そのまま再実行」: 上の「失敗した job の再実行」を行う（`N_AUTO` は増やさない）。
- 「修正を main に入れた。タグを付け直してよい」: 下の「タグの付け直し」を行う。

**タグの付け直し**（11.8）。開発者の承認が明示されているときだけ行う。承認が無ければ、付け直さない。

1. 定型コマンド A（`-Version {V}`）で、公開済みが 0/4・`不明` が 0・`GitHub Release` が `無し` であることを確かめる。満たさなければ、付け直さずに止まる（11.9）。
2. `git fetch origin main` の後、`git rev-parse origin/main` の出力を `TARGET` とする。定型コマンド C を `-Version {V} -Target {TARGET}` で実行し、4 行とも `合格` であることを確かめる。不合格なら、付け直さずに `不合格` の行を示して止まる。
3. タグを付け直して push する。この 2 つは許可の一覧に入れない。実行のたびに、開発者が許可の確認に答える。

   ```bash
   git tag -f -a v{V} -m "Release v{V}" {TARGET}
   git push -f origin v{V}
   ```

4. 新しい実行が起動する。`{ATTEMPT}` を 1 に戻し、段 7 の手順 1 から続ける。

## 段 9: 完了の報告

| Field | Detail |
|-------|--------|
| Intent | 完了を、取り違えようのない形で報告する |
| Requirements | 13.2, 13.5 |

段 7 の手順 4 を満たしたときだけ、次の形で報告する（13.2）。`<...>` は結果表と定型コマンド A・B の出力から埋める。

```
リリース完了: v{V}
- crates.io: shiori4_abi <status> / shiori4 <status> / shiori4_host <status>
- GitHub Release: <URL>
- リリース CI の実行: <URL>
- 再実行の回数: <最後の attempt から 1 を引いた数>
- 手順との食い違い: <なし / 手順どおりに動かなかった箇所>
```

手順書・スキルの記述と実際の挙動が食い違ったときは（13.5）、この実行では文書を直さない。食い違った段・コマンド・期待した出力・実際の出力を「手順との食い違い」に書く。止まるときの報告（「リリース未完了」）にも、同じ項目を付ける。

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

- `期限まで` は、3 クレートで共有するトークン（30 分 = 1800 秒）の残りである。`Auth crates.io` かその step の完了の時刻が無いと数えられず、`期限まで - 秒` と出る（段 8 の「トークンの期限切れの見分け」では期限切れと見なさない）。
- `line` は、公開の step（`Publish <クレート>`・`Create GitHub Release`）のログの行だけを、attempt ごとに出す。

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
