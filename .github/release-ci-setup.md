# リリース CI の一度だけの準備

リリース CI（`.github/workflows/release.yml`）が crates.io へ認証して公開できるようにするための、設定の手順書。節の順が実施の順になっている。ほかの文書と CI の案内は、この手順書の節を番号でなく見出しの名前で指す。

## 位置づけと全体の流れ

- ここに書く設定は、リリース CI のワークフローの外で **1 度だけ** 行う。リリースのたびに行うものではない（毎回のリリースは `release` スキル、`/release`）。準備が済めば、以後は `/release` だけで公開できる。
- 設定をやり直すとき（リポジトリの改名・移管、新しいクレートの追加など）や、設定の食い違いを疑うときにも、この手順書を使う。
- 設定に入れる名前（リポジトリ・ワークフローのファイル名・environment・クレート名と公開の順）は、末尾の「名前の対応表」にまとめてある。各設定画面には、表の値をそのまま写す。
- **認証情報・ID の値はリポジトリに書かない。** このリポジトリは公開されている。API トークンの値・crates.io や GitHub の画面に出る ID を、ファイル・Issue・PR・コミットメッセージに書かない。
- GitHub の secrets は使わない。crates.io へは Trusted Publishing（GitHub の OIDC から 30 分のトークン）で認証し、長期の認証情報をどこにも置かない。クレートを手で公開するときに使う API トークンも、後片付けで失効させる。

全体の流れ:

1. GitHub に公開専用の environment `release` を作る（節「GitHub: environment `release`」）
2. 公開対象のクレートが crates.io にまだ無ければ、手で公開する（節「新しいクレートの初回公開」）
3. クレートに Trusted Publisher を登録する（節「crates.io: Trusted Publisher」）
4. 後片付けをする（節「後片付け（必須）」）

リポジトリの改名・移管で設定をやり直すときは 1 と 3 を、新しいクレートを足すときは 2〜4 を行う。

## GitHub: environment `release`

**必須**: リポジトリ `ekicyou/shiori4` の Settings →「Environments」→「New environment」で、environment `release` を作る。

1. 名前を `release` にする（`release.yml` の publish-crates の `environment:` と完全に同じ）。
2. 「Deployment branches and tags」を「Selected branches and tags」にする。
3. 「Add deployment branch or tag rule」で Ref type を Tag にし、パターン `v*` を足す。ブランチの規則は足さない。

- `release` をタグからの実行に限ることで、crates.io のトークンを得られるのはリリースタグの push で動く実行だけになる。
- environment の secrets と variables は使わない。
- environment の保護規則は、GitHub Free では public リポジトリでだけ使える。

**推奨**: main のブランチ保護（Settings →「Branches」または「Rules」で、main への変更に PR を必須にする）。リリースタグを付けるコミットは main から到達できることをリリース CI が確かめるので、main を PR 経由でしか変えられないと、公開される中身がレビューを通ったものに限られる。

タグの作成を制限するルールセットは求めない（リリースタグの形・版・main からの到達・見どころは、リリース CI の verify が確かめる）。

## crates.io: Trusted Publisher

3 クレートそれぞれで、crates.io のクレートの設定画面（Settings →「Trusted Publishing」）から GitHub の Trusted Publisher を追加する。クレートが crates.io に在ってからでないと登録できないので、手動公開の後に行う。

| クレート（公開の順） |
|----------------------|
| `shiori4_abi` |
| `shiori4` |
| `shiori4_host` |

3 件とも、次の値を入れる。

| 欄 | 値 |
|----|----|
| Repository owner | `ekicyou` |
| Repository name | `shiori4` |
| Workflow filename | `release.yml`（パスを付けず、ファイル名だけ。完全一致で照合される） |
| Environment | `release` |

- environment を入れたので、environment `release` 以外からの実行ではトークンを得られない。
- owner は数値の ID で照合される。リポジトリを改名・移管したら、3 件とも作り直す。
- 設定が合っているかは、CI から公開するまで確かめきれない。3 件の設定画面の値を「名前の対応表」と目で照合しておく。

## 後片付け（必須）

API トークンでクレートを手で公開した後（新しいクレートの初回公開・緊急時の手動公開）、Trusted Publisher を登録したら、次を **必ず** 行う。手元に残った長期の認証情報での公開の経路を閉じるためのものである。エージェントは行わず、開発者が行う。

1. **Trusted Publisher を照合する。** crates.io の画面で、対象のクレートの Trusted Publisher の値を「名前の対応表」と照合する（値が合っているかは、CI から公開するまで確かめきれないため）。
2. **「Trusted Publishing のみ」を有効にする。** 3 クレートそれぞれの crates.io の設定画面（Trusted Publishing の項）で、Trusted Publishing だけで公開を受け付け、トークンでの公開を拒む設定（Require trusted publishing for all new versions）を有効にする。
3. **API トークンを失効させる。** 手動公開に使った API トークンを、crates.io の「Account Settings」→「API Tokens」で失効（Revoke）させる。
4. **手元から消す。** 開発機の環境変数 `CARGO_REGISTRY_TOKEN` を消し、`cargo login` で保存していたなら `cargo logout` で消す。

## 緊急時の手動公開

CI が長く使えず、手で公開するしかないとき。

1. クレートの owner が、crates.io の設定画面で「Trusted Publishing のみ」を一時的に無効に戻す。
2. 新しい API トークンを作り、依存の順に手で公開する（`cargo publish -p <クレート>`。下位のクレートが crates.io の索引に現れてから次を出す）。
3. 済んだらトークンを失効させ、手元の `CARGO_REGISTRY_TOKEN`・`cargo login` の保存を消す。
4. 「Trusted Publishing のみ」を有効に戻す。

手で公開した版にも、後でリリースタグと GitHub Release をそろえられる（`/release` で、3 クレートは `skipped` になる）。

## 新しいクレートの初回公開

Trusted Publishing のトークンでは、crates.io にまだ無いクレートを作れない。新しいクレートを公開対象に加えるときは、初回だけ手で公開してから Trusted Publisher を登録する。リリース CI は、クレートが crates.io に無いことを見つけると公開を試みず、そのクレートを `failed`・`not-registered` にしてこの節を案内する。

1. crates.io で、新しいクレートを公開できる API トークン（スコープ `publish-new`）を一時的に作る。
2. 開発機で、そのトークンを使って新しいクレートを手で公開する（`cargo publish -p <クレート>`）。依存先のクレートの同じ版が crates.io に公開済みである必要がある。
3. 公開できたら、1 のトークンをすぐに失効させ、手元の `CARGO_REGISTRY_TOKEN`・`cargo login` の保存を消す。
4. 新しいクレートに、節「crates.io: Trusted Publisher」と同じ値で Trusted Publisher を登録し、節「後片付け（必須）」の「Trusted Publishing のみ」を有効にする。
5. クレートの一覧を更新する。
   - `release.yml` の publish-crates: そのクレートの publish step を依存の順の位置に足し、job の outputs・集約 step（`Summarize crates.io results`）の一覧・report の一覧にも加える
   - リリーススキルの定型コマンド A（`.claude/skills/release/scripts/release-state.ps1`）のクレートの一覧
   - この手順書の「名前の対応表」と、節「crates.io: Trusted Publisher」のクレートの表
   - `.kiro/steering/tech.md` の公開の順

## 2 つ目以降のクレートの認証だけが失敗したとき

リリース CI は、job の先頭の step `Auth crates.io` で 1 回だけトークンを取り、3 つの publish step に渡す。トークンは 30 分で切れる。`shiori4_abi` が `published` か `skipped` になったのに、2 つ目以降のクレート（`shiori4`・`shiori4_host`）だけが `failed`・`auth` になったときは、次で見分ける。

リリーススキルの定型コマンド B（`.claude/skills/release/scripts/ci-result.ps1 -Run <実行の ID>`）の、そのクレートの `Publish <クレート>` の `step` の行の「期限まで」を見る。

- **`期限まで` が 0 以下**（auth から 30 分を超えた）: 認証の設定ではなく、トークンの期限切れである。失敗した job を再実行する。新しいトークンを取り直し、公開済みのクレートは `skipped` で飛ばして、残りを公開する。
- **0 より大きい**: トークンは有効だったのに、そのクレートの公開が拒まれた。そのクレートの Trusted Publisher の値を「名前の対応表」と照合して直し、失敗した job を再実行する。
- **`期限まで - 秒`**（時刻が取れず数えられない）: 期限切れと見なさず、0 より大きいときと同じに照合する。

`shiori4_abi` から失敗する場合や、step `Auth crates.io` が失敗する場合は、この節ではない。environment `release` と 3 クレートの Trusted Publisher を「名前の対応表」と照合する。

## 名前の対応表

設定画面に入れる名前と、リリース CI の実際の名前の対応。リリース CI の認証の失敗（`reason` が `auth`）の案内は、この表を指す。設定をやり直すときは、この表と設定画面を照合する。

| 項目 | 値 | 使う場所 |
|------|----|----------|
| リポジトリ | owner `ekicyou`・name `shiori4` | crates.io の Trusted Publisher |
| ワークフローのファイル名 | `release.yml`（完全一致） | crates.io の Trusted Publisher |
| 公開用 environment | `release`（保護規則: Selected branches and tags → タグ `v*`） | publish-crates の `environment:`、crates.io の Trusted Publisher の Environment 欄 |
| クレートと公開の順 | `shiori4_abi` → `shiori4` → `shiori4_host` | publish-crates の step の並び・Trusted Publisher の登録（3 件） |
