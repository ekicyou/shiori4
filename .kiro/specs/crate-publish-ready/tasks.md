# Implementation Plan

- [x] 1. 版とメタデータをワークスペースに一本化する
- [x] 1.1 ワークスペースに版・共通メタデータ・内部依存の置き場所を作る
  - ワークスペースの版を初版 0.0.1 とし、edition 2024・rust-version 1.85 は今の値のまま共通メタデータに置く
  - 作者・MIT だけのライセンス・リポジトリとホームページ（GitHub の URL）・keywords（shiori・ghost・ukagaka）を共通メタデータに置き、documentation・categories・license-file は書かない
  - 内部依存 2 つ（`shiori4_abi`・`shiori4`）を、手元のソースの場所と版要求 0.0.1 の両方を持つ形でワークスペースの依存に置く。外部依存 libloading はここに移さない
  - 完了時: ルートのマニフェストに design.md の WorkspaceManifest の契約どおりの値があり、この時点でも `cargo build` が通る
  - _Requirements: 1.1, 1.3, 1.4, 1.5, 2.1, 2.2, 2.4, 2.7, 2.8_

- [x] 1.2 3 クレートのマニフェストをワークスペースの値の受け継ぎに切り替える
  - 各クレートは名前と今の description だけを自前で持ち、版・edition・rust-version・作者・ライセンス・リポジトリ・ホームページ・keywords を受け継ぐ
  - `shiori4` は `shiori4_abi` を、`shiori4_host` は `shiori4` をワークスペースの内部依存から受け継ぐ。`shiori4_host` の libloading 0.8 はそのまま残す
  - readme・include・exclude は書かない
  - 完了時: 各クレートのマニフェストに版の数字が無く、`cargo metadata` で 3 クレートとも版 0.0.1・ライセンス MIT と出て、`cargo build` が通る
  - _Requirements: 1.2, 1.3, 2.2, 2.3, 2.8, 7.3_

- [x] 2. パッケージに入る LICENSE と README を用意する
- [x] 2.1 (P) 各クレートにルートの LICENSE の写しを置く
  - ルートの LICENSE を 3 クレートのディレクトリへバイト単位で写す。ルートの本文は変えない
  - 完了時: 3 つの写しがルートの LICENSE と `cmp` で一致する
  - _Boundary: LicenseCopies_
  - _Requirements: 2.6, 4.4_

- [x] 2.2 (P) 各クレートの README と、ルート README からの導線を書く
  - 3 クレートそれぞれに日本語だけの README を置き、役割・想定する利用者・使い始めの入口・他の 2 クレートとの関係・詳しいドキュメント（docs.rs）・ライセンスの節をこの順に書く
  - 規約の本文と使用例の全文は書かず docs.rs へ誘導する。他クレート・docs.rs・リポジトリ内のファイルへの参照はすべて絶対 URL にする
  - ルート README のクレート表の各行から、各クレートの README と crates.io のページへ辿れるようにする
  - 完了時: 3 つの README に 6 つの節がそろい、相対リンクが 1 つも無く（ルート README を除く）、ルート README から 3 クレートの README と crates.io のページへのリンクがある
  - _Boundary: CrateReadmes, RootReadme_
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [x] 3. CI と依存検査の設定を置く
- [x] 3.1 (P) 依存検査の許可の設定を置く
  - 許可するライセンスを MIT・Apache-2.0・ISC だけにし、ワイルドカードの版要求・未知のレジストリ・未知の git を拒否し、取得元を crates.io の索引だけにする
  - 完了時: 手元の `cargo deny check` が advisories・bans・licenses・sources とも ok で、警告を出さない
  - 手元に cargo-deny が無ければ `cargo install cargo-deny` で入れる
  - _Boundary: DenyConfig_
  - _Depends: 1.2_
  - _Requirements: 5.4, 5.5, 7.3_

- [x] 3.2 (P) PR・push・手動実行・他ワークフローからの呼び出しで同じ検査を回すワークフローを置く
  - トリガは main への push・main への PR・手動実行・入力も秘密も無い呼び出しの 4 つ。権限は読み取りだけ、トリガで分岐する条件は持たない
  - 3 OS の行列（1 つの失敗で他を打ち切らない）でテストと clippy を別ステップで回すジョブ、ubuntu の依存検査ジョブ、ubuntu のパッケージ化ジョブを、互いに待たせずに並べる
  - パッケージ化ジョブは 3 クレートのパッケージ化と検証の後に、`shiori4_abi` のパッケージに C ヘッダがあることと、3 つの LICENSE がルートと一致することを確かめ、欠けたクレートと中身をエラーとして出して非ゼロで終わる
  - 完了時: `actionlint` がエラーを出さず、パッケージ中身の検査スクリプトを手元の bash で流すと成功し、LICENSE の写しを 1 つ一時的に変えると該当クレート名を出して失敗する（確認後に戻す）
  - 手元に actionlint が無ければ入れる。入れられないときは CI の初回実行で確かめる残りとして報告する
  - _Boundary: BuildWorkflow_
  - _Depends: 1.2, 2.1, 2.2_
  - _Requirements: 1.6, 2.6, 4.1, 4.2, 4.3, 4.5, 4.6, 5.1, 5.2, 5.3, 5.6, 5.7, 5.8, 6.1, 6.2, 6.3, 6.4, 7.2, 7.4_

- [x] 4. steering に版・CI・lock・内部依存・README の方針を書く
  - tech.md に、版の置き場所の契約（3 か所）・共通メタデータ・MIT だけ・CI の 3 種のジョブと他ワークフローからの呼び出し・`Cargo.lock` をコミットしない理由（`--locked` を使わない）を書き、Common Commands に `--workspace` 付きの clippy と `cargo package --workspace`（単独の `-p` は初回公開まで使えない旨）を足す
  - structure.md の内部依存の書き方を「ワークスペースの依存に path と版要求を持ち、各クレートが受け継ぐ」に直し、各クレートの README は crates.io の顔・ルート README は入口と書く
  - workflow.md は「このリポジトリに CI は無い」の 1 文だけを、PR で build.yml が走るがマージの関門は Test Gate で CI を待たない、の趣旨に直す
  - 完了時: steering 3 本の差分が 8.1〜8.5 を満たし、workflow.md の差分がその 1 文だけである
  - _Depends: 1.2, 3.2_
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [ ] 5. 公開の直前まで通ることを手元で確かめる
- [ ] 5.1 パッケージ化・中身・版の食い違いの検出を手元で確かめる
  - 変更をコミットしてから（未コミットの変更があると `cargo package` が断る）、`cargo package --workspace` を流す
  - `cargo package --list -p <crate>` と、解凍した `.crate` のマニフェストを確かめる
  - タスク 3.2 のパッケージ中身の検査スクリプトを、実際のパッケージに対して流す
  - 一時的にワークスペースの版だけを 0.0.2 にして `cargo build` が版要求の解決で失敗することを確かめ、元に戻す
  - 完了時: 3 クレートが依存の順に Packaging → Verifying で通り `warning:` が 0 行、各パッケージに Cargo.toml・README.md・LICENSE・src/lib.rs があり（`shiori4_abi` は C ヘッダも）、開発用ディレクトリが無く、解凍したマニフェストに版 0.0.1・MIT・repository・homepage・authors・keywords があり documentation が無い
  - _Depends: 1.2, 2.1, 2.2, 3.2_
  - _Requirements: 1.1, 1.2, 1.4, 1.6, 2.1, 2.2, 2.5, 2.6, 2.7, 2.8, 3.1, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [ ] 5.2 回帰と変えないものを確かめる
  - Test Gate の 2 コマンドを流し、`cargo deny check` も念のため再実行して（許可の設定そのものの確認はタスク 3.1）、Rust のソースと C ヘッダに差分が無いことを `git diff main --stat` で確かめる
  - ワークフローの定義を読み、入力なしの呼び出しトリガがあること・トリガで分岐する条件が無いこと・権限が読み取りだけ・`pull_request_target` を使わないことを確かめる
  - 完了時: `cargo test --all`・`cargo clippy --all-targets --workspace -- -D warnings`・`cargo deny check` が通り、`src/` と `include/` の差分が 0 で、外部依存は libloading だけのまま。3 OS の実際の CI 実行は最初の PR で確かめる残りとして報告する
  - _Depends: 3.1, 3.2_
  - _Requirements: 6.1, 6.3, 6.4, 7.1, 7.2, 7.3, 7.4_

- [ ] 6. ドキュメント整合性の確認と更新
  - タスク 4・5 の後に、6.1〜6.4 の順に 1 つずつ行う（並行しない）
  - _Depends: 4, 5.1, 5.2_
- [ ] 6.1 クレート冒頭の `//!` - 規約・設計方針の変更を反映（該当する場合）
  - 本 spec はソースを変えないので、`//!` と README の役割の説明が食い違っていないかだけを確かめる
  - 完了時: 食い違いが無いこと、または直した箇所を報告する
  - _Requirements: 3.3, 7.1_
- [ ] 6.2 `shiori4_abi/include/shiori4_abi.h` - ABI の型・定数・規約の変更を Rust 側と揃える（該当する場合）
  - 本 spec は ABI を変えないので、ヘッダに差分が無いことを確かめる
  - 完了時: ヘッダの差分が 0
  - _Requirements: 7.1_
- [ ] 6.3 README.md - クレートの役割・使い方の入口の変更を反映（該当する場合）
  - 3 クレートの README の役割・入口の記述が各クレート冒頭の `//!` と食い違わないことを確かめる（README 同士の整合はタスク 2.2 で済ませた）
  - 完了時: README と `//!` の記述がそろっている
  - _Requirements: 3.2, 3.5_
- [ ] 6.4 steering/* - 該当領域のステアリング更新
  - 最終状態の tech.md・structure.md・workflow.md を、実際のマニフェスト・ワークフロー・deny の設定と読み比べる（読むだけ）
  - ずれが見つかったら、steering はタスク 4 の持ち物なので、タスク 4 の範囲として直す
  - 完了時: steering の記述と実際のマニフェスト・ワークフローが一致する
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
