# Requirements Document

## Project Description (Input)
**誰が困っているか**: 開発者（ekicyou）。3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を crates.io に公開したいが、`cargo publish` が断るうえ、PR で壊れたことに気付く仕組みも無い。

**現状**: 各クレートの `version = "0.1.0"` がばらばらに直書きされ、内部依存は path だけ（version なし）。`repository` などのメタデータとクレートごとの README が無い。`license = "MIT OR Apache-2.0"` なのに LICENSE は MIT だけ。C ヘッダ `shiori4_abi/include/shiori4_abi.h` がパッケージに入るか未確認。`.github/` が無く CI も無い。

**何を変えるか**: pasta の Cargo.toml の流儀に揃え、版を `[workspace.package] version = "0.0.1"` と `[workspace.dependencies]` の内部依存の `version` に一本化し、メタデータ・各クレートの README・`license = "MIT"` を整え、3 クレートで `cargo package` が依存の順に通るようにする。あわせて `.github/workflows/build.yml`（ubuntu / windows / macos で `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings`、cargo-deny、`workflow_call: {}`）と `deny.toml` を置き、PR と main への push で検査する。実際の公開・release.yml・`/release` スキルは対象外（release-workflow spec が持つ）。

## Introduction
shiori4 ワークスペースの 3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を、crates.io に出せる形（公開の直前まで）に整え、その形を PR ごとの CI で守る。
利用者は crates.io / docs.rs 上で各クレートの役割と入口を知ることができ、開発者は手元と CI の両方で「公開すれば通る」ことを確かめられる。版は 1 か所（と内部依存の版要求）だけに置き、後続の release-workflow spec がその場所を書き換えて公開する。

> 本書では Cargo.toml のキー名（`[workspace.package]`・`[workspace.dependencies]`）・`cargo package`・cargo-deny・clippy・OS 名を名指しする。これらは開発者が決定済みの事項であり、かつ release-workflow spec との継ぎ目（版の置き場所・gate として呼ぶ CI）なので、要件の契約として扱う。どう書くか（ファイルの中身・job の構成）は設計で決める。

## Boundary Context
- **In scope**:
  - ルートと各クレートの Cargo.toml（版の一本化・内部依存・メタデータ・`license = "MIT"`）
  - 各クレートの README（crates.io の顔）。ルート README はワークスペースの入口として残す
  - C ヘッダがパッケージに入ること、3 クレートの `cargo package` が通ることの確認（手元と CI）
  - PR / main への push で走る CI（3 OS の test・clippy、cargo-deny、パッケージ化の検証、他ワークフローからの呼び出し）と、その検査の設定
  - steering（`tech.md`・`structure.md`）の版・メタデータ・CI・`Cargo.lock` 方針の記述
- **Out of scope**:
  - crates.io への実際の公開（初回の手動公開を含む）
  - タグで動くリリース CI・公開スクリプト・`/release` スキル・リリースノート
  - クレートの API・ABI・挙動の変更
  - `.kiro/steering/workflow.md` の「前提: 完成の形」・DoD・リモート同期の節の書き直し（release-workflow が持つ。OPEN QUESTION 8 参照）
- **Adjacent expectations**:
  - release-workflow は、本 spec の CI を入力なしで gate として呼び出し、その成否で公開を止める
  - release-workflow の `/release` は、本 spec が決めた版の置き場所（ワークスペースの版 1 か所＋内部依存の版要求）だけを書き換えれば版を上げられることを前提にする
  - 公開の順は依存の順（`shiori4_abi` → `shiori4` → `shiori4_host`）。本 spec はその順で検証が通ることまでを保証する

## Requirements

### Requirement 1: 版の一本化
**Objective:** As a 開発者, I want 3 クレートの版を 1 か所で管理したい, so that 版の更新で書き漏らしや食い違いが起きない

#### Acceptance Criteria
1. The shiori4 ワークスペース shall 3 クレートの版を `[workspace.package]` の `version` 1 か所で定め、その値を初版 `0.0.1` とする
2. The shiori4 ワークスペース shall 各クレートの Cargo.toml に版の数字を直書きせず、ワークスペースの版を受け継がせる
3. The shiori4 ワークスペース shall クレート間の依存（`shiori4` → `shiori4_abi`、`shiori4_host` → `shiori4`）の版要求を `[workspace.dependencies]` の内部依存の項目にだけ持ち、各クレートはそれを受け継いで使う
4. The shiori4 ワークスペース shall 内部依存を、ワークスペース内のビルドでは手元のソースを参照し、公開されたパッケージでは crates.io 上の同じ版を参照する形で宣言する
5. When 開発者が版を上げる, the shiori4 ワークスペース shall ワークスペースの版 1 か所と内部依存の版要求 2 か所を書き換えるだけで、3 クレートの版と内部依存の版要求がそろう状態にする
6. If 内部依存の版要求がワークスペースの版を満たさない（例: 版を `0.0.2` に上げたのに版要求が `0.0.1` のまま）, then the shiori4 ワークスペース shall ビルドまたはパッケージ化を失敗させ、CI で検出できるようにする（版要求が新しい版を満たしてしまう書き漏らし、例: `0.1.0` の要求で `0.1.1` を受け入れる場合は、この基準では検出しない）

### Requirement 2: メタデータとライセンスの整合
**Objective:** As a クレートの利用者, I want crates.io 上で各クレートの出どころ・ライセンス・ドキュメントの場所を正しく知りたい, so that 安心して依存に加えられる

#### Acceptance Criteria
1. The 各クレート shall ライセンスの表明を `MIT` だけとし、`Apache-2.0` を含めない
2. The 各クレート shall リポジトリ（`https://github.com/ekicyou/shiori4`）・ホームページ・ドキュメントの場所・作者を、ワークスペースの共通メタデータから受け継いで表明する（値は OPEN QUESTION 1・2 の前提に従う）
3. The 各クレート shall 現在の `description`（クレートの役割の 1 文）を保つ
4. The shiori4 ワークスペース shall edition 2024・`rust-version = "1.85"` を変えない
5. When 開発者が各クレートをパッケージ化する, the パッケージ化 shall メタデータの欠落（documentation・homepage・repository など）に関する警告を出さない
6. **（前提・OPEN QUESTION 3）** The 各クレートのパッケージ shall MIT の著作権表示と許諾文（ルートの LICENSE と同じ本文）を含む。ルートの LICENSE の本文は変えない

### Requirement 3: 各クレートの README
**Objective:** As a クレートの利用者, I want crates.io のクレートのページで役割と使い始め方を知りたい, so that どのクレートを使えばよいか、何を実装すればよいかがすぐ分かる

#### Acceptance Criteria
1. The 各クレート shall クレートのディレクトリに README を持ち、それがパッケージに含まれて crates.io のクレートのページに表示される
2. The 各クレートの README shall 少なくとも、クレートの役割・想定する利用者（SHIORI 作者 / ベースウェア作者 / ABI を直接使う実装者）・使い始めの入口（実装する trait や読み込みの入口）・他の 2 クレートとの関係・ライセンスを示す
3. The 各クレートの README shall 規約・設計方針の詳細を重複して書かず、クレート冒頭のドキュメント（docs.rs）へ誘導する（steering の「README は使い方の入口だけ」に従う）
4. If README がリポジトリ内の他のファイルや他のクレートを参照する, then the README shall crates.io 上でも辿れる形（絶対 URL など）で参照する
5. The ルート README shall ワークスペースの入口として残り、各クレートの README（または crates.io のページ）へ辿れる
6. **（前提・OPEN QUESTION 4）** The 各クレートの README shall 日本語で書く

### Requirement 4: パッケージの内容と検証の通過
**Objective:** As a 開発者, I want 公開の前に 3 クレートが公開できる形であることを確かめたい, so that 公開の当日に `cargo publish` に断られない

#### Acceptance Criteria
1. When 開発者がワークスペースのルートで 3 クレートのパッケージ化と検証（`cargo package` 相当、`cargo publish --dry-run` 相当）を実行する, the shiori4 ワークスペース shall 3 クレートすべてを依存の順（`shiori4_abi` → `shiori4` → `shiori4_host`）に成功させる
2. While 3 クレートのいずれも crates.io に未公開である, the shiori4 ワークスペース shall 受け入れ基準 4.1 のパッケージ化と検証を成功させる
3. The `shiori4_abi` のパッケージ shall C ヘッダ `include/shiori4_abi.h` を含む
4. The 各クレートのパッケージ shall そのクレートのソース・マニフェスト・README（と受け入れ基準 2.6 のライセンス文）を含み、開発用のファイル（`.kiro/`・`.claude/`・`.github/`・`target/`）を含まない
5. If パッケージ化または検証が失敗する, then the 検証 shall 失敗したクレートと理由を表示して非ゼロで終わる
6. The 検証 shall crates.io へのアップロードを一切行わない

### Requirement 5: PR と main への push での CI 検査
**Objective:** As a 開発者, I want PR と main への push ごとに 3 OS で検査が走ってほしい, so that 壊れた変更や問題のある依存をマージ前に気付ける

#### Acceptance Criteria
1. When main に向けた PR が作成または更新される, the CI shall ubuntu-latest・windows-latest・macos-latest の 3 OS それぞれで、ワークスペース全体のビルドと全テスト（`cargo test --all`）と、警告をエラーとする lint（`cargo clippy --all-targets --workspace -- -D warnings`）を実行する
2. When main に push される, the CI shall 受け入れ基準 5.1 と同じ検査を実行する
3. If いずれかの OS で検査が失敗する, then the CI shall 全体を失敗として PR に示し、他の OS の検査は途中で打ち切らずに最後まで結果を出す
4. When CI が走る, the CI shall 依存クレートのライセンス・既知の脆弱性勧告・取得元を、リポジトリに置いた許可の設定に照らして検査する（cargo-deny）
5. If 依存に許可していないライセンス・脆弱性勧告・許可していない取得元が見つかる, then the CI shall 検査を失敗させ、該当する依存と理由を表示する
6. When CI が走る, the CI shall 要件 4 のパッケージ化と検証を実行し、失敗すれば検査を失敗させる
7. The CI shall 開発者が PR 上で、どの OS・どの検査（test・clippy・依存検査・パッケージ化）が失敗したかを見分けられる単位で結果を示す
8. The CI shall crates.io への公開・タグの作成・GitHub Release の作成を行わず、秘密のトークンを必要としない

### Requirement 6: 他のワークフローからの gate としての呼び出し
**Objective:** As a release-workflow の作者, I want 本 spec の CI をリリースの関門としてそのまま呼びたい, so that 公開前の検査を二重に書かずに済む

#### Acceptance Criteria
1. When 同じリポジトリの他のワークフローが入力なしで本 CI を呼び出す, the CI shall 要件 5 と同じ検査（3 OS の test・clippy、依存検査、パッケージ化）を実行する
2. If 呼び出された CI の検査が 1 つでも失敗する, then the CI shall 呼び出し元に失敗を返す
3. The CI shall 呼び出しに入力・秘密情報を要求しない
4. The CI shall 呼び出された場合も、PR・push で走る場合と同じ検査項目・同じ合否の基準を使う

### Requirement 7: 変えないもの
**Objective:** As a クレートの利用者と開発者, I want 公開準備がクレートの中身に影響しないことを保証したい, so that 公開準備の変更を安心してマージできる

#### Acceptance Criteria
1. The 本 spec の変更 shall 3 クレートの公開 API・ABI（構造体のレイアウト・関数テーブル・定数）・C ヘッダの内容・実行時の挙動を変えない
2. When 本 spec の変更を入れた後に開発者がワークスペースのルートで全テストと clippy を実行する, the shiori4 ワークスペース shall 既存のテストをすべて通し、clippy の警告を出さない
3. The 本 spec の変更 shall 外部依存を増やさない（`shiori4_abi`・`shiori4` は依存ゼロ、`shiori4_host` は `libloading` だけのまま）
4. The 本 spec の変更 shall crates.io へ何も公開しない

### Requirement 8: steering の更新
**Objective:** As a 後続の spec（release-workflow）と開発者, I want 版の置き場所・CI・`Cargo.lock` の方針が steering に書かれていてほしい, so that 以後の作業がその前提に沿う

#### Acceptance Criteria
1. The `tech.md` shall 版の置き場所（ワークスペースの版 1 か所と内部依存の版要求）・共通メタデータ・ライセンスが MIT だけであること・CI の検査内容（3 OS の test・clippy、依存検査、パッケージ化、他ワークフローからの呼び出し）を記す
2. The `tech.md` shall `Cargo.lock` をコミットするかどうかの方針を、設計で決めた結果に合わせて記し、CI の実際の動きと矛盾させない
3. The `tech.md` の Common Commands shall CI と同じ lint のコマンド（`--workspace` を含む）とパッケージ化の検証のコマンドを示す
4. The `structure.md` shall クレート間の参照の仕方（ワークスペースの内部依存として、手元のソースと版要求の両方を持つこと）と、各クレートの README の位置づけ（crates.io の顔、ルート README は入口）を記す

---

## OPEN QUESTIONS（要件ディスカッションで決める）
brief.md で決めきれなかった事項。上の要件は各「前提」で暫定的に書いてある。

1. **作者（authors）の値**: 前提は pasta と同じ `Dot-Station Master <dot.station@gmail.com>`。別の表記にするか、`authors` 自体を置かないか
2. **homepage / documentation の向き先**: 前提は pasta と同じくワークスペース共通で GitHub リポジトリの URL。ライブラリなので documentation を各クレートの docs.rs にする（ワークスペース共通にできないので各クレートで書く）か、documentation を置かずに crates.io の既定に任せるか
3. **ライセンス文をパッケージに入れるか（受け入れ基準 2.6）**: MIT は複製に許諾文を含めることを求めるが、`license = "MIT"` だけではルートの LICENSE はパッケージに入らない（現状の確認済み）。前提は「各パッケージに入れる」。入れない、という選択も可
4. **README の言語と分量**: 前提は日本語（steering の「ドキュメントは日本語」に合わせる）で、使い方の入口だけ。英語を併記するか
5. **C ヘッダが入っていることを CI で毎回検査するか**: 前提は CI のパッケージ化の検証の中で、パッケージの中身に C ヘッダがあることも確かめる。一度手元で確かめるだけにするか
6. **手動実行（workflow_dispatch）を CI のきっかけに入れるか**: brief は PR・main への push・`workflow_call` だけを挙げる。pasta の build.yml には手動実行がある。前提は pasta に合わせて入れる
7. **MSRV（Rust 1.85）での検査を CI に入れるか**: brief は stable の 3 OS だけを挙げる。前提は入れない（`rust-version` の表明だけ）
8. **workflow.md の「このリポジトリに CI は無い」「CI を待たない」の記述**: 本 spec で CI ができると事実と食い違う。brief はリモート同期の節の書き直しを release-workflow に割り当てている。前提は本 spec では workflow.md に触れない（release-workflow が直す）。事実の 1 行だけ本 spec で直すか
9. **keywords / categories**: brief に無い。前提は本 spec では付けない
10. **依存検査の許可ライセンスの範囲**: brief は「pasta のものを MIT 系の許可リストに合わせて」。前提は MIT と両立する寛容なライセンスに限る（具体的な一覧は設計で決める。現在の依存は MIT・Apache-2.0・ISC だけ）

### 設計に持ち越す決定（質問ではない）
- `Cargo.lock` をコミットするか（brief の Constraints により、本 spec の設計で決めて `tech.md` を直す。release-workflow が `--locked` で公開するかどうかと関わる）
