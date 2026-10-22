# Requirements Document

## Project Description (Input)
**誰が困っているか**: shiori4 の開発者（ekicyou）。3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を crates.io に公開したいが、手元からの `cargo publish` では、公開前の検査（タグと版の一致・main からの到達・CI 通過・依存の順）を人の注意に頼ることになる。

**現状**: crate-publish-ready により、3 クレートは `cargo package --workspace` が通り、build.yml が PR で 3 OS の検査をしている（`workflow_call` でも呼べる）。しかし、タグで動くワークフロー・公開スクリプト・`/release` スキル・リリースノートの置き場所は無い。`.kiro/steering/workflow.md` は公開の運用を持たない。pasta（`C:\home\maz\git\pasta`）には実際に動いている同じ仕組み（release.yml・`.github/scripts/release/*.ps1`・release-ci-setup.md・`.claude/skills/release/`・`doc/release-notes/`）がある。

**何を変えるか**: pasta の仕組みを 3 クレート用に縮めて移植する。`vX.Y.Z` のタグ push で release.yml が走り、タグと `[workspace.package] version` の一致・main からの到達・見どころファイルの存在を検査し、build.yml を gate として通したうえで、`shiori4_abi` → `shiori4` → `shiori4_host` の順に Trusted Publishing で公開し、GitHub Release を作る（数え方は 3 クレート ＋ GitHub Release = 4/4）。同じタグの再実行は公開済みを `skipped` にして残りだけ進める。開発者は `/release [X.Y.Z]` で、版の決定 → 版の更新の PR（squash）→ タグの push → CI の見届け・失敗した job の再実行 → 報告までを、実際の状態から続きを判定しながら行う。初回セットアップ（environment `release`・Trusted Publisher 登録・初回の手動公開・`trustpub_only`）の手順書を用意し、以後は `/release` だけで公開できるようにする。`Cargo.lock` はコミットしないので `--locked` は使わない。

## Introduction

本書は、shiori4 の 3 クレートを crates.io に公開する仕組みの要件を定める。仕組みは 3 つに分かれる（brief の Boundary Candidates）。

- **リリース CI**: リリースタグの push だけで起動し、公開前の検査・crates.io への公開・GitHub Release の作成・結果の報告を行う。
- **リリーススキル（`/release`）**: 開発者の指示で、版の決定から版の更新の統合・リリースタグの push・リリース CI の見届け・報告までを行う。
- **セットアップ手順書**: 開発者が一度だけ行う準備（environment・Trusted Publisher・初回の手動公開・Trusted Publishing のみへの切り替え）を定める。

あわせて、リリースノートの見どころの約束と、steering（`workflow.md`・`tech.md`）・エージェントの操作の許可を、公開の運用に合わせる。

brief で確定できなかった点は要件ディスカッションで確定し、末尾の「確定事項」にまとめる。

用語:

- **リリースタグ**: `v` の後に `X.Y.Z`（ASCII 数字 3 つ。プレリリースの形を含まない）が続くタグ。例: `v0.0.1`。
- **ワークスペースの版**: ルート `Cargo.toml` の `[workspace.package] version` の値。内部依存の版要求 2 か所（`[workspace.dependencies]` の `shiori4_abi`・`shiori4`）も同じ値にそろっている（crate-publish-ready の契約）。
- **公開先**: crates.io の 3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）と GitHub Release の 4 つ。
- **公開済み**: その版が公開先に実際に存在する状態。GitHub Release は、下書きでなく公開されている状態だけを公開済みとする。
- **公開先ごとの結果**: `published`（今回公開した）・`skipped`（公開済みのため飛ばした）・`failed`（失敗した。理由 `reason` を伴う）・`not-run`（前の段の失敗で行わなかった）のいずれか。
- **見どころのファイル**: `doc/release-notes/vX.Y.Z.md`。その版の GitHub Release の本文の先頭に置く文章。
- **版の更新**: ワークスペースの版の表記 3 か所を新しい版にそろえ、見どころのファイルを加える変更。
- **作業ブランチ**: Claude Code ハーネスが供給するワークツリーの、デフォルトではないブランチ。

## Boundary Context

- **In scope**:
  - リリースタグの push で動くリリース CI（`.github/workflows/release.yml`）と、その補助スクリプト（`.github/scripts/release/`）。補助スクリプトのテストを build.yml の job として足すこと。
  - 一度だけの準備の手順書（`.github/release-ci-setup.md`）。初回の手動公開と、初版 0.0.1 のリリースの手順を含む。
  - 見どころの約束（`doc/release-notes/README.md`）と、見どころのファイルの書き方。初版の見どころのファイル `doc/release-notes/v0.0.1.md`。
  - リリーススキル（`.claude/skills/release/`）と、エージェントの操作の許可（`.claude/settings.json`）のうちリリースに使うもの。
  - steering `workflow.md`（前提: 完成の形・DoD・リモート同期・ドキュメント保守）と `tech.md`（公開とバージョン・CI）の更新。
- **Out of scope**:
  - Cargo.toml のメタデータ・各クレートの README・build.yml の既存の検査（crate-publish-ready が持つ）。build.yml には補助スクリプトのテストの job を足すだけで、既存の job・トリガ・入力を変えない。
  - 初回セットアップと初回の手動公開の**実行**（開発者が手順書に従って行う。スキルも CI も代行しない）。
  - リリースの自動化（定期実行・main への push での自動リリース）。タグの push は人が `/release` で起こす。
  - 初版 0.0.1 のリリースが済んだ後の、初版の経路の撤去（後続の spec `release-bootstrap-cleanup` が持つ）。
  - pasta 固有の部分: 配布物のビルド（dll・nar・vsix）、VSCode Marketplace と Azure の認証一式、セットアップの確認ワークフロー、マニュアルの CI、版の出どころのうち package.json・マニュアル・Marketplace、見どころの口調の検査。
  - プレリリースの版、クレートごとに異なる版、crates.io 以外のレジストリ、バイナリの配布。
  - クレートの API と中身。
- **Adjacent expectations**:
  - crate-publish-ready: build.yml が `workflow_call`（入力・秘密なし）で呼べ、3 OS の test・clippy・cargo-deny・`cargo package --workspace` を行う。版の置き場所はルート `Cargo.toml` の 3 か所だけで、`Cargo.lock` はコミットしない。本仕様はこれらを前提にし、変えない。
  - GitHub: リポジトリ `ekicyou/shiori4` は公開リポジトリで、environment の保護規則を使える。手元の `gh` は認証済みである。
  - crates.io: Trusted Publishing は新しいクレートを作れない（初回は API トークンで手動公開が要る）。Trusted Publishing のトークンは短命（30 分）である。
  - steering `workflow.md` のリモート同期（PR・squash・CI を待たない統合）に、版の更新の PR も従う。

## Requirements

### Requirement 1: リリース CI の起動とタグの検査

**Objective:** As a 開発者, I want リリースタグの push だけで公開が始まり、公開してはいけないタグでは何も公開されないようにしたい, so that 取り消せない公開を誤ったコミットや版で始めない

#### Acceptance Criteria

1. When リリースタグがリポジトリに push されたとき, the Release CI shall 起動する。
2. The Release CI shall ブランチへの push・PR・リリースタグの形でないタグの push では起動しない。
3. When 起動したとき, the Release CI shall タグがリリースタグの形であること、タグの版とタグのコミットのワークスペースの版が一致すること、タグのコミットが main から到達できること、タグのコミットにタグの版の見どころのファイルがあり空白と改行だけでないことを検査する。
4. If 検査のいずれかが失敗したとき, the Release CI shall どの公開先にも公開せずに止まり、失敗した検査の項目と「どの公開先にも公開していない」ことを実行の summary に書く。
5. While 同じタグの実行が進んでいるとき, the Release CI shall 同じタグの新しい実行を、進んでいる実行を取り消さずに後に並べる。

### Requirement 2: 公開前の関門

**Objective:** As a 開発者, I want 公開の前に、PR の CI と同じ検査がタグのコミットで通ることを確かめたい, so that main の CI の結果を自分で見に行かなくても、検査に落ちるコミットを公開しない

#### Acceptance Criteria

1. When タグの検査が通ったとき, the Release CI shall タグのコミットで、PR の CI（build.yml）と同じ検査をすべて行う。
2. If 関門の検査のいずれかが失敗したとき, the Release CI shall どの公開先にも公開せずに止まる。
3. The Release CI shall 関門で、リリースの補助スクリプトのテストを行う（テストは build.yml の job として足すので、PR の CI でも同じ job が走る）。
4. The Release CI shall 補助スクリプトのテストを、ネットワーク・GitHub の認証・crates.io を使わずに行い、リポジトリの作業ツリーを変えない。

### Requirement 3: crates.io への公開

**Objective:** As a 開発者, I want 3 クレートが依存の順に、秘密のトークンをリポジトリに置かずに公開され、再実行しても二重に公開しようとしないようにしたい, so that 途中で失敗しても、失敗した job の再実行だけで残りを出せる

#### Acceptance Criteria

1. When 関門が通ったとき, the Release CI shall `shiori4_abi` → `shiori4` → `shiori4_host` の順に、1 クレートずつタグの版を crates.io に公開する。
2. The Release CI shall crates.io の認証を Trusted Publishing の短命なトークンだけで行い、リポジトリ・secrets に長期の認証情報を置かない。
3. The Release CI shall 公開の認証を、リリースタグから起動した実行の、公開専用の environment（`release`）の中でだけ得られるようにする。
4. When クレートのその版が crates.io に公開済みのとき, the Release CI shall そのクレートを公開せずに `skipped` とし、次のクレートへ進む。
5. If crates.io への問い合わせが、存在する・存在しないのどちらとも判定できない応答（障害・ネットワークの失敗・時間切れ）で終わったとき, the Release CI shall 公開済みと見なさず、そのクレートを `failed`（`reason`: 一時的な失敗）とする。
6. If クレート自体が crates.io に無いとき, the Release CI shall 公開を試みず、そのクレートを `failed`（`reason`: 未登録）とし、手順書の初回の手動公開の節を案内する。
7. If 認証が得られないとき, the Release CI shall そのクレートを `failed`（`reason`: 認証）とし、手順書の名前の対応表との照合を案内する。
8. When 下位のクレートを同じ実行で公開したとき, the Release CI shall 上位のクレートの公開の前に、下位のクレートの新しい版が依存として解決できるようになるのを、上限のある時間だけ待つ。
9. If あるクレートの公開が失敗したとき, the Release CI shall それより後のクレートを公開せず `not-run` とする。
10. The Release CI shall `Cargo.lock` の固定（`--locked`）を前提にせず公開し、公開済みの版の取り消し（yank）・上書き・検証の省略を行わない。
11. When 公開を終えたとき（成否を問わず）, the Release CI shall クレートごとの結果・`reason`・所要時間・次の手を実行の summary に書く。

### Requirement 4: GitHub Release の作成

**Objective:** As a クレートの利用者, I want 版ごとに、見どころと変更の一覧がそろった GitHub Release を読みたい, so that その版で何が変わったかを 1 か所で知れる

#### Acceptance Criteria

1. When 3 クレートがすべて `published` か `skipped` になったとき, the Release CI shall リリースタグの GitHub Release を作り、公開する。
2. If 3 クレートのいずれかが `published` でも `skipped` でもないとき, the Release CI shall GitHub Release を作らない。
3. The Release CI shall GitHub Release の本文の先頭に見どころのファイルの中身を置き、末尾の空白と改行の形のほかは書き換えない。
4. The Release CI shall 1 つ前のリリースタグ（無ければ履歴の先頭）からリリースタグまでの変更を種類ごとに分類した詳しいリリースノートを作り、内部の作業（Requirement 6.5）を載せない。
5. The Release CI shall 詳しいリリースノートを GitHub Release の本文の見どころの後に直接置き、添付を持たない。
6. When 同じタグの GitHub Release がすでに公開済みのとき, the Release CI shall 作り直さずに `skipped` とする。
7. When 同じタグの GitHub Release が下書きのまま残っているとき, the Release CI shall その下書きを公開する。公開済みの Release を削除・上書きしない。
8. The Release CI shall 書き込みの権限を GitHub Release を作る段にだけ与え、リリースタグを作ったり動かしたりしない。

### Requirement 5: 結果の報告

**Objective:** As a 開発者, I want 1 回の実行の結果を、公開先ごとに 1 枚の表で見たい, so that どこまで出て何が残っているかを取り違えない

#### Acceptance Criteria

1. When 実行が終わるとき（どの段で止まっても）, the Release CI shall 4 つの公開先（3 クレートと GitHub Release）の結果と `reason`、GitHub Release の URL（無ければ「なし」）を 1 枚の表にまとめて実行の summary に書く。
2. If 公開より前の段（タグの検査・関門）で止まったとき, the Release CI shall 止まった段と「どの公開先にも公開していない」ことを表に添える。
3. If 公開先の結果が書かれていないとき, the Release CI shall その段が走らなかった場合だけ `not-run` とし、それ以外は推測せず、その段の summary を参照するよう書く。
4. The Release CI shall 失敗の理由ごとに次の手（認証 → 手順書の照合、未登録 → 初回の手動公開、一時的な失敗 → 失敗した job の再実行、公開の失敗 → ログを読む）を示す。

### Requirement 6: 見どころとリリースノートの約束

**Objective:** As a 開発者・エージェント, I want 見どころの書き方と、リリースノートに載せない変更の決まりが 1 か所に書かれていてほしい, so that `/release` が毎回同じ形の見どころを書け、CI と食い違わない

#### Acceptance Criteria

1. The Release Notes Convention shall 見どころのファイルの置き場所・名前（`doc/release-notes/vX.Y.Z.md`）と、それがリリース CI の検査の対象で、無ければどこにも公開しないことを定める。
2. The Release Notes Convention shall 見どころの書き方（日本語の Markdown、見どころの件数の目安、見どころごとの見出しと元の PR の番号、動きが変わる点をすべて載せること、spec 名やコミットの接頭辞を書かないこと）を定める。
3. The Release Notes Convention shall 見どころのファイルに版・件数・詳しいリリースノートへのリンクを書かない（リリース CI が足す）ことを定める。
4. The Release Notes Convention shall 語り手の口調の決まりを持たない（pasta の口調の検査は移植しない）。
5. The Release Notes Convention shall 詳しいリリースノートに載せない内部の作業を、コミットの件名の種類とスコープで定める（種類 `spec`、スコープ `spec`・`discovery`・`roadmap`・`steering`・`skills`・`cc-sdd`・`release-workflow`・`kiro` または `kiro-` で始まるもの、`chore(release)`）。spec 名をスコープにしたコミットは、`release-workflow` のほかは載せる。決まりを変えるときは、リリース CI の分類と同時に直す。
6. The Release Notes Convention shall タグを打つ前に、見どころの材料（1 つ前のリリースタグから main の先頭までの変更の一覧）を手元で出す方法と、見どころのファイルを確かめる方法を示す。
7. Where 1 つ前のリリースタグが無い初版のとき, the Release Notes Convention shall 材料を履歴の先頭からの変更とすることを示す。

### Requirement 7: リリーススキルの開始条件

**Objective:** As a 開発者, I want `/release` が、リリースと関係のない変更を巻き込まない状態からだけ始まってほしい, so that 版の更新だけが main に入り、リリースの中身が意図どおりになる

#### Acceptance Criteria

1. If 現在のブランチがデフォルトブランチであるとき, the Release Skill shall 何も変えずに止まり、ハーネスのワークツリーでの再実行を求める。
2. When 開始したとき, the Release Skill shall GitHub の認証、PR の作成と squash での統合の権限、リリース CI の実行の閲覧、リモートのタグの読み取りができることを確かめる。
3. If 必要な操作ができないとき, the Release Skill shall 何も変えずに止まり、できない操作を示す。
4. If 作業ブランチに未コミットの変更、または main に無い内容があるとき, the Release Skill shall それらをリリースに含めず、内容を示して止まる。ただし、main に無い内容が版の更新のコミットだけである場合（Requirement 12.5）を除く。
5. When 作業ブランチが main より遅れているとき, the Release Skill shall 版の更新より前に、main を履歴を書き換えずに取り込む。
6. If main の取り込みで衝突が起きたとき, the Release Skill shall 取り込みを中止して止まり、衝突したファイルを示す。
7. The Release Skill shall 手元でのビルド・テスト・公開（crates.io への公開・GitHub Release の作成）を行わず、公開済みのものの取り消し（yank・Release の削除）を行わない。公開前の検査はリリース CI に任せる。
8. The Release Skill shall 進み具合をファイルに書かず、版の更新のほかにコミットを作らない。

### Requirement 8: 版の決定

**Objective:** As a 開発者, I want リリースする版を、すでに出ている版と重ならないように確定したい, so that 取り消せない公開を誤った版で始めない

#### Acceptance Criteria

1. When 版が指定されているとき, the Release Skill shall 先頭の `v` を外した指定の版を候補にする。
2. When 版が指定されていないとき, the Release Skill shall 版の出どころ（main のワークスペースの版・リモートのリリースタグ・crates.io の 3 クレート・GitHub Release）の最大の版の PATCH を 1 つ上げた版を候補にし、出どころごとの最大の版とともに示して、承認を求めずに進む。
3. The Release Skill shall 版が指定されていないとき、MINOR・MAJOR を自動では上げない。
4. If 候補がリリースタグの版の形（数字 3 つ）でないとき, the Release Skill shall エラーを示し、入力し直しを求める。
5. If 候補の版が、リモートのリリースタグ・crates.io のいずれかのクレート・GitHub Release のどれかにすでにあるとき, the Release Skill shall エラーを示し、別の版の入力を求める（Requirement 12 の再開と Requirement 14 の初版にあたる場合を除く）。
6. If 出どころへの問い合わせが失敗し、版の有無を確かめられないとき, the Release Skill shall 「無い」と見なさず、確かめられなかった出どころを示して止まる。
7. If 候補の版への版の更新の PR が統合されないまま残っているとき, the Release Skill shall 新しい PR を作らず、その PR を示して開発者に扱いを確かめる。

### Requirement 9: 版の更新と main への統合

**Objective:** As a 開発者, I want 版の表記と見どころを 1 つのコミットでそろえ、ほかの変更と同じ PR の流れで main に入れたい, so that リリース CI の検査を通り、main のブランチ運用を崩さない

#### Acceptance Criteria

1. When 版が確定したとき, the Release Skill shall ワークスペースの版の表記 3 か所（`[workspace.package] version` と、`[workspace.dependencies]` の `shiori4_abi`・`shiori4` の `version`）を、確定した版にそろえる。
2. When 版の表記をそろえたとき, the Release Skill shall Requirement 6 の約束に従って見どころのファイルを書き、約束に合うかを確かめる。
3. If 見どころのファイルが約束に合わないとき, the Release Skill shall 合わない点を直して確かめ直し、3 回直しても合わなければ版の更新を元に戻して止まる。
4. The Release Skill shall 版の更新で、ルート `Cargo.toml` と見どころのファイルのほかを変えない（`Cargo.lock` はコミットしない）。
5. If 版の更新の確認（3 か所が同じ版・ほかのファイルを変えていない・見どころが約束に合う）のいずれかが失敗したとき, the Release Skill shall 版の更新を元に戻し、失敗した確認を示して止まる。
6. When 確認がすべて通ったとき, the Release Skill shall 版の更新だけを含む 1 つのコミット（件名 `chore(release): vX.Y.Z`）を作り、見どころのファイルの全文を開発者に示して、承認を求めずに進む。
7. When コミットを作ったとき, the Release Skill shall PR を作り、PR の CI を待たずに squash で main へ統合する。main へ直接 push しない。
8. If PR の作成または統合が失敗したとき, the Release Skill shall リリースタグを作らず、強制 push・履歴の書き換え・統合前のブランチの削除を行わずに止まり、PR と失敗の内容を示す。

### Requirement 10: リリースタグの作成と push

**Objective:** As a 開発者, I want 版を上げた main のコミットにだけリリースタグを付けて push したい, so that リリース CI がタグの検査で落ちず、タグがその版のソースを正しく指す

#### Acceptance Criteria

1. When 版の更新が main に統合されたとき, the Release Skill shall 統合の結果のコミットに注釈付きのリリースタグ `vX.Y.Z` を作る。
2. The Release Skill shall タグを付ける前に、リリース CI のタグの検査（Requirement 1.3）と同じ条件をタグを付けるコミットで確かめ、満たさなければタグを作らずに止まる。
3. If 同じ名前のタグがリモートにあるとき, the Release Skill shall エラーを示して止まり、開発者に扱いを確かめる。既存のタグを自動で削除・付け替えしない（Requirement 11.8 の承認を得た付け直しを除く）。
4. When タグを作ったとき, the Release Skill shall タグだけをリモートへ push する。
5. If タグの push が失敗したとき, the Release Skill shall 失敗の内容と「main には版の更新が入っていて、タグだけが無い」状態であることを示して止まる。

### Requirement 11: リリース CI の見届けと失敗への対応

**Objective:** As a 開発者, I want タグの push の後、エージェントに公開を最後まで見届けさせ、一時的な失敗は人手なしで進め、人の判断が要る失敗では止まってほしい, so that Actions の画面を見張らずに済み、取り返しのつかない操作の前で必ず止まる

#### Acceptance Criteria

1. When タグを push したとき, the Release Skill shall タグの名前とタグのコミットの両方が一致するリリース CI の実行を特定する。上限の時間内に見つからなければ、完了と報告せずに止まる。
2. While 実行が終わっていないとき, the Release Skill shall 終わるまで結果を追い、完了と報告しない。上限の時間を超えたら「リリース CI が実行中」として止まる。
3. When 実行が終わったとき, the Release Skill shall 4 つの公開先の結果・`reason` と GitHub Release の URL を読み取る。結果が書かれていない公開先は、その段の実行結果から判定し、推測で `published` にしない。
4. If 失敗が一時的な原因によるとき, the Release Skill shall 同じ実行の失敗した job だけを再実行し、結果を確かめ直す。自動の再実行は 1 回の `/release` の中で 3 回までとし、超えたら未完了として止まる。
5. If 失敗の原因が一度だけの準備にあるとき（認証・未登録）, the Release Skill shall 再実行せず、原因と手順書の該当の節を示して止まる。
6. When 準備の失敗の後に、開発者が設定を直したと伝えたとき, the Release Skill shall 失敗した job の再実行から続ける。
7. If 公開より前の段（タグの検査・関門）で失敗し、どの公開先にもその版が無いとき, the Release Skill shall 原因と何も公開していないことを報告して止まる。
8. When 公開より前の段の失敗の修正が main に入り、開発者がタグの付け直しを明示的に承認したとき, the Release Skill shall どの公開先にもその版が無いことを確かめ直してから、同じ版のリリースタグを修正後の main のコミットへ付け直して push する。
9. If 再実行では直らない失敗があり、いずれかの公開先にその版がすでにあるとき, the Release Skill shall タグを付け直さず、公開済みと未公開の公開先を示し、版を上げて出し直す必要があることを報告する。
10. If 原因が特定できない公開の失敗のとき, the Release Skill shall 失敗した job を 1 度だけ再実行して切り分け、なお失敗すれば Requirement 11.7 か 11.9 に従う。
11. If 実行が取り消されていたとき, the Release Skill shall 再実行せず、取り消されていることを示して止まる。

### Requirement 12: 途中からの再開

**Objective:** As a 開発者, I want 途中で止まったリリースを、もう 1 度 `/release` を実行するだけで続きから進めたい, so that セッションが切れても、版を飛ばしたり二重に作業したりしない

#### Acceptance Criteria

1. The Release Skill shall 始める位置を、前回の会話の記録ではなく、実際の状態（main のワークスペースの版・リモートのタグ・crates.io・GitHub Release・リリース CI の実行・未統合の PR）だけで判定する。
2. When main の版のリリースタグがあり、4 つの公開先すべてで公開済みのとき, the Release Skill shall 新しいリリースとして版の決定（Requirement 8）から始める。指定の版がその版と同じなら、リリース済みであることを示して別の版を求める。
3. When main の版のリリースタグがあり、公開済みが 4 つに満たないとき, the Release Skill shall 版の決定・更新・タグの作成を飛ばし、その実行の見届け（Requirement 11）から続ける。タグはあるのに実行が無いときは、起動していないことを示して止まる。
4. When main の版のリリースタグが無く、どの公開先にもその版が無く、版の指定が無いかその版と同じとき, the Release Skill shall 版の決定と更新を飛ばし、リリースタグの作成（Requirement 10）から続ける。指定の版が違うときは、main に未リリースの版があることを示して止まる。
5. When 作業ブランチの main に無い内容が版の更新のコミットだけのとき, the Release Skill shall そのコミットの版を示し、重複の検査と版の更新の確認をやり直したうえで、main への統合（Requirement 9.7）から続ける。
6. If 判定に使う状態のいずれかを確かめられないとき、またはどの判定にも当たらない状態（タグが無いのに一部が公開済みなど。ただし Requirement 14 の初版を除く）のとき, the Release Skill shall 推測で進めず、状態を示して止まる。
7. While main の版のリリースが未完了のとき, when 別の版が指定されたとき, the Release Skill shall 指定の版を始めず、先に main の版の続きを行うことを示す。

### Requirement 13: 完了の判定と報告

**Objective:** As a 開発者, I want リリースが終わったのか途中なのかを、取り違えようのない形で知りたい, so that 一部だけ公開された状態を「完了」と思い込まない

#### Acceptance Criteria

1. The Release Skill shall 実行の結論が成功で、4 つの公開先がすべて `published` か `skipped` で、公開先への問い合わせでも 4 つすべてが公開済みと確かめられたときだけ、リリースを完了と判定する。
2. When 完了したとき, the Release Skill shall 「リリース完了: vX.Y.Z」で始まり、クレートごとの結果・GitHub Release の URL・リリース CI の実行の URL・再実行の回数・手順との食い違いを含む報告を出す。
3. When 完了せずに止まるとき, the Release Skill shall 「リリース未完了」（版が決まる前なら「リリースを開始できない」）で始まり、止まった段と理由・公開済みの公開先・残っている公開先と結果・原因の区分・次に誰が何をするか・再開の方法を含む報告を出す。
4. When 公開より前の段で止めたとき, the Release Skill shall 公開先への問い合わせでその版が 1 つも無いことを確かめたうえで、どの公開先にも公開していないことを報告に含める。
5. If 手順書・スキルの記述と実際の挙動が食い違ったとき, the Release Skill shall その実行では文書を直さず、報告の「手順との食い違い」に書く。

### Requirement 14: 初版 0.0.1 のリリース

**Objective:** As a 開発者, I want 手で公開した初版 0.0.1 にも、リリースタグと GitHub Release をそろえ、CI の配線を確かめたい, so that 以後のリリースを `/release` だけで行える状態から始められる

#### Acceptance Criteria

1. The Setup Guide shall 初版 0.0.1 の 3 クレートを、開発者が API トークンで依存の順に手動で公開する手順を定める（Trusted Publishing は新しいクレートを作れないため）。
2. When main のワークスペースの版の 3 クレートが crates.io に公開済みで、その版のリリースタグと GitHub Release がどちらも無く、その版の見どころのファイルが main にあるとき, the Release Skill shall 初版の状態と判定し、版の決定と版の更新を飛ばして、リリースタグの作成（Requirement 10）から続ける。
3. The repository shall 初版の見どころのファイル `doc/release-notes/v0.0.1.md` を本仕様の実装の変更に含めて main に入れる。初版の `/release` は見どころのファイルを書かず、見どころだけのコミットも作らない。
4. When 初版のリリースタグで実行が終わったとき, the Release Skill shall 3 クレートが `skipped`、GitHub Release が `published` であれば完了と判定する。
5. When 初版の実行が完了したとき, the Release Skill shall 3 クレートの認証が CI から得られたこと（公開そのものは次の版で初めて CI から行われること）を報告に含め、手順書の初回の後片付け（Requirement 15.5）を開発者に案内する。自分では行わない。
6. The Release Skill shall 初版の判定と経路を、ほかの判定・経路に触れずに取り除けるように 1 か所にまとめ、初版のためだけのものであることを明記する（撤去は後続の spec `release-bootstrap-cleanup` が行う）。

### Requirement 15: 一度だけの準備の手順書

**Objective:** As a 開発者, I want 一度だけ行う準備を、手順書の順にたどるだけで済ませたい, so that 準備が済めば以後は `/release` だけで公開でき、設定の食い違いを手順書と照合して直せる

#### Acceptance Criteria

1. The Setup Guide shall 節の順が実施の順になるように、準備の全体の流れを先頭に示す。
2. The Setup Guide shall GitHub の公開専用の environment（`release`）を作り、実行できる ref をリリースタグに限る手順を必須として定める。main のブランチ保護（PR 必須）は推奨として示し、タグの作成を制限するルールセットは求めない。
3. The Setup Guide shall 3 クレートそれぞれに crates.io の Trusted Publisher（リポジトリ・ワークフローのファイル名 `release.yml`・environment `release`）を登録する手順を、初回の手動公開の後に行う順で定める。
4. The Setup Guide shall 設定に入れる名前（リポジトリ・ワークフローのファイル名・environment・クレート名と公開の順）を 1 枚の対応表にまとめ、リリース CI の認証の失敗の案内がその表を指す。
5. The Setup Guide shall 初版の確認の後に必ず行う後片付けとして、3 クレートの「Trusted Publishing のみ」の有効化と、手動公開に使った API トークンの失効・手元からの削除を定める。
6. The Setup Guide shall 緊急時（CI が長く使えないとき）に「Trusted Publishing のみ」を一時的に戻して手で公開し、元に戻す手順を定める。
7. The Setup Guide shall 新しいクレートを公開対象に加えるときの手順（初回の手動公開・Trusted Publisher の登録・リリース CI とスキルのクレートの一覧の更新）を定める。
8. If 2 つ目以降のクレートの認証だけが失敗したとき, the Setup Guide shall それが認証の取り直しの問題か Trusted Publisher の設定の問題かを見分け、落とし先を示す。
9. The Setup Guide shall 認証情報・ID の値をリポジトリに書かないことを定める。

### Requirement 16: steering と操作の許可の整合

**Objective:** As a 開発者・エージェント, I want 開発の約束と操作の許可が、公開の運用と食い違わないようにしたい, so that 古い記述（「公開・タグ・CI は持たない」）に従って誤った操作をしない

#### Acceptance Criteria

1. The Steering shall `workflow.md` の「前提: 完成の形」を、公開の仕組みができた形（タグ push で CI が公開し、開発者は `/release` で版の更新から見届けまでを行う）に改める。
2. The Steering shall `workflow.md` のリモート同期の節に、版の更新の PR も同じ流れ（PR・squash・CI を待たない）で統合することと、直接 push の禁止の例外がリリースタグの push だけであることを書く。
3. The Steering shall `workflow.md` の DoD の Test Gate に、リリースの補助スクリプトを変えたときは手元で補助スクリプトのテストが通ることを加える。
4. The Steering shall `workflow.md` のドキュメント保守の表に、リリースの仕組み（release.yml・補助スクリプト・公開対象のクレート・版の置き場所・見どころの約束）を変えたときに合わせて直す文書（リリーススキル・手順書・見どころの約束）を加える。CLAUDE.md は Kiro のワークフローの説明なので変えない。
5. The Steering shall `tech.md` の公開とバージョン・CI の節に、リリース CI の段の流れ・公開の順・認証の方式・手元から公開しないことを書く。
6. The repository shall エージェントの操作の許可（`.claude/settings.json`）に、リリーススキルが使う操作（作業ブランチの push・PR の作成と統合・リリースタグの push・リリース CI の閲覧と失敗した job の再実行・main の取り込み）を入れ、手元からの公開・タグの付け直し・強制 push を入れない。
7. The repository shall 本仕様の変更の後も、`cargo test --all` と clippy、および PR の CI が通る状態を保つ。

## 確定事項（要件ディスカッションの結果）

1. **初版 0.0.1 の扱い**（14.2・14.4・14.6）: `/release` に「初版の状態」の判定を足し、版の決定と更新を飛ばしてタグの作成から続ける。初版の経路は 1 か所にまとめ、0.0.1 の完了後に後続の spec `release-bootstrap-cleanup` が撤去する。
2. **`v0.0.1.md` の置き場所**（14.3）: 本仕様の実装の PR に含める。見どころだけのコミットの経路は持たない。
3. **「Trusted Publishing のみ」の有効化の時期**（14.5・15.5）: 初版の実行で認証が通ったことを確かめた後に有効化し、トークンを失効させる。CI からの公開は 0.0.2 で初めて確かめる（失敗しても再実行と緊急時の手順 15.6 で戻せる）。
4. **GitHub Release の添付**（4.5〜4.7）: 添付を持たず、詳しいリリースノートを本文の見どころの後に直接置く。添付の判定・変更不可の失敗の経路は持たない。
5. **内部の作業のスコープ**（6.5）: pasta の一覧に `kiro` を足したもの。spec 名のスコープは `release-workflow` のほかは載せる。
6. **DoD への補助スクリプトのテストの追加**（16.3）: 補助スクリプトを変えたときだけ、手元で pwsh のテストを通すことを Test Gate に加える。
7. **main のブランチ保護・タグの保護**（15.2）: environment の ref の制限だけを必須、main のブランチ保護は推奨、タグのルールセットは求めない。
8. **CLAUDE.md の更新**（16.4）: 変えない。
