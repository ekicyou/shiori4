# 実装計画

- [ ] 1. 基盤: ABI の定数・型、テスト用 DLL、`shiori4_host` のファイル分割
- [x] 1.1 ABI に -7 の定数を足し、受け取ったバイト列を別スレッドへ渡せるようにする
  - C ヘッダと Rust に、規約 2 に反する `host->request` を表す値 -7 のエラーを足す（既存の値・レイアウトの検査は変えない）
  - 受け取ったバイト列と応答を `Send` にする（`Sync` は付けない）。理由はスレッド規約 6（release は任意スレッド）であることを SAFETY のコメントに書く
  - `shiori4_abi` に初めてのテストのモジュールを作り、-7 の値と既存の値が変わらないこと、自分が確保したバイト列を別スレッドへ送って解放できること（`Send` のコンパイル時の確認を含む）を確かめる
  - 完了: `cargo test -p shiori4_abi` で新しいテストが通り、`SHIORI_STATIC_ASSERT` と `const _` の検査もそのまま通る
  - _Boundary: AbiConstants, OwnedSend_
  - _Requirements: 1.6, 2.2, 5.5, 5.7_

- [x] 1.2 (P) DLL を読むテストのためのテスト用 cdylib と、それを作って場所を返すテスト専用の仕組みを用意する
  - ワークスペースの外（自前の `[workspace]`、公開しない）に、`shiori4` だけに path で依存する cdylib を置く
  - その DLL は自前の `shiori_get_api` で v4／v3 のテーブルをモードによって出し分け、モードを設定する書き出しと、呼ばれないダミーの `loadu`・`load` の書き出しを持つ。Ghost の load は dir に `fail` を含むと失敗する
  - `shiori4_host` のテスト専用モジュールが、テストの中で cargo を呼んで 1 テストバイナリにつき 1 回だけ DLL を作り、場所を返す（`--target-dir` を明示し、子の cargo から `CARGO_BUILD_TARGET` を外す）。DLL のモードを守るロックも持つ
  - 完了: `shiori4_host` のテストから DLL の場所を得て `libloading` で開き、4 つの書き出し（`shiori_get_api`・モードの設定・`loadu`・`load`）が見つかることをテストで確かめられる
  - _Boundary: TestFixture_
  - _Requirements: 15.5_

- [x] 1.3 `shiori4_host` を、読み込みの口と純粋な関数のファイルに分ける骨組みを作る
  - v3 互換モードの口・発見・配布の決まりの 3 つのモジュールを宣言し、それぞれに役割を述べるモジュールの doc だけを置く（中身は後のタスク）
  - steering の `structure.md` の「1 クレートは `src/lib.rs` 1 ファイル」を、`shiori4_host` が読み込みの口と純粋な関数を別ファイルに分けたことに合わせて直す
  - 完了: ワークスペースが警告なしでビルドでき（clippy の `-D warnings` を含む）、`structure.md` が新しいファイルの分け方を述べている
  - _Depends: 1.2_
  - _Requirements: 1.3_

- [ ] 2. SHIORI 側の規約の守り方（`shiori4_abi`）
- [x] 2.1 unload を始めたらそのインスタンスの Poster を無効にし、実行中の post・log を待つ門を作る
  - v4 の load がインスタンスと門をまとめた箱を作り、request・notify はその中のインスタンスを使う。v3 の箱は門を持たない
  - unload は「門を閉じる → 実行中の post・log が 0 になるまで待つ → インスタンスを Drop → 箱を解放」の順に行う
  - Poster は門を見てから ctx に触れ、閉じていれば ctx を読まずに `SHIORI_E_SHUTDOWN` を返す。Poster に任意スレッドから呼べる log を足し、閉じた後は何もしない。Poster から `Copy` を外し `Clone` を残す
  - ワークスペースの中で Poster を暗黙にコピーしている所を `.clone()` に直す。`Shiori`・`ShioriHostPoster`・`ShioriHostHandle` の doc から join を求める記述を消し、Drop の中で使えるのは log だけ（request は -7 になりうる）と書く
  - 完了: テストで (a) unload の後の post が `SHIORI_E_SHUTDOWN` を返しホストの post が呼ばれない、(b) unload 前に入った post が返るまで unload が返らない、(c) unload の後も動き続けるスレッドの post・log が安全、(d) unload の後の Poster の log がホストに届かない、がすべて通り、ワークスペース全体がビルドできる
  - _Requirements: 1.3, 4.3, 4.4, 4.5, 4.6, 4.7, 5.6_

- [ ] 2.2 v4 の load の失敗を、NULL を返す前にホストの log へ 1 行出す
  - dir の検査の失敗・Ghost の load の `Err`・パニックのどれでも、host が有効なら `SHIORI_LOG_ERROR` でステータスを含む英語の 1 行を出す
  - log の呼び出しも境界の保護の内側で行い、パニックを外へ出さない。host が無効なとき・v3 のときは出さない
  - 完了: テストで `Err` とパニックのそれぞれでホストの log にちょうど 1 行が届き、v3 の load の失敗では log が呼ばれないことが確かめられる
  - _Requirements: 7.5_

- [ ] 3. 規約の本文（C ヘッダの正本と Rust の写し）
- [ ] 3.1 版・ABI 規約・スレッド規約 1〜7・load の規約・ホストのプロセスの節を書き、Rust の doc に同文を写す
  - 設計の「規約の本文の案」を出発点に、C ヘッダの冒頭の注釈を書き直す（v4 の Charset、パニックの規約を言語によらない形で ABI 規約へ、公開シンボルの文の直し、規約 1〜7、load の規約、専用プロセスの注意）
  - `shiori4_abi` のクレートの doc を同じ番号・同じ言葉にする（C の `->` と Rust の `.` の差だけを許す）。旧 Rust の「規約 6（catch_unwind）」を ABI 規約へ移す
  - `ShioriHostApi` の request・log、`ShioriApi` の load の dir の注釈を、C ヘッダと Rust の両方で該当する規約を指すように直す
  - 完了: C ヘッダと `//!` の各節を突き合わせて同文であり、join を求める記述が無く、C ヘッダのコンパイル時の検査と `cargo test -p shiori4_abi` が通る
  - _Requirements: 1.1, 1.2, 1.4, 1.5, 1.6, 2.1, 2.3, 2.4, 4.1, 4.2, 5.1, 5.2, 6.1, 6.2, 7.1, 7.2, 7.3, 7.4, 8.1, 8.2, 9.1, 10.1, 10.2, 10.3_

- [ ] 3.2 DLL の口と発見の順の節と、規約の後の「配布の決まり」の節を書き、Rust の doc に同文を写す
  - 両方の口を持ってよいこと、`shiori_get_api` → `loadu` → `load` の順、フォールバックしないことを述べる
  - 配布の決まりとして、名前の幹、OS と arch ごとの探す順、接尾辞は x64・arm64 だけ、Linux と macOS は `lib` 付き、macOS は接尾辞なし、機械の種類の確認と候補が尽きたときの扱い（SHIORI/3.0 と v4 の両方に使える）を述べる
  - 完了: C ヘッダと `//!` の両方に 2 つの節が同文であり、要件 14.1 の 5 つの探す順がすべて書かれている
  - _Requirements: 11.1, 14.1, 14.2, 14.7_

- [ ] 4. (P) `Ghost` の doc を規約に合わせ、Charset と複数インスタンスの振る舞いをテストで守る
  - `Ghost` の doc の「Drop で止めて join すること」を、unload を始めた時点で Poster が無効になり join は要らない、に置き換える
  - `Ghost::notify` の doc に、即 204 で返して後回しにしてよいが、後回しの処理では `host->request` を使えず post を使うことを書く。`Ghost::load` の doc に、失敗は自動で 1 行 log されること、インスタンスごとに状態を持つことを書く
  - `Adapter` の doc に、v4 は Charset を付けず拒否もしない、v3 は従来どおりと書く（コードは変えない）
  - 完了: テストで (a) v4 の要求に `Charset: Shift_JIS` が付いていても 400 にならず応答に Charset が無い、(b) 同じテーブルから 2 インスタンスを load して状態が混ざらない、(c) Poster を `.clone()` して別スレッドから post できる、が通り、既存の v3 の Charset のテストも通る
  - _Boundary: GhostDocs_
  - _Depends: 2.1_
  - _Requirements: 1.3, 6.3, 7.7, 9.3, 9.4, 9.5_

- [ ] 5. ホスト側の規約の守り方と `from_library`（`shiori4_host` の lib.rs）
- [ ] 5.1 (P) 規約 2 に反する `host->request` に `SHIORI_E_WRONG_THREAD` を返す
  - ブリッジに持ち主のスレッドと処理中の印を持たせ、ランタイムが SHIORI の load・request・notify を呼ぶ間だけ印を立てる（Drop で下ろす小さな値で）
  - request の先頭の 1 つの判定で、持ち主のスレッドでないか印が立っていなければ -7 と空の out を返し、`Host` のどのメソッドも呼ばない。振り分けの本体には触れない
  - post・log はスレッドを見ない
  - 完了: テストで (a) 別スレッドからの request が -7 で `Host` が呼ばれない、(b) 同じスレッドの呼び出しの外が -7、(c) `Ghost::load` の中の property の取得が成功する、(d) 2 つのランタイムを別スレッドで同時に動かしどちらも成功する、(e) 別スレッドからの Poster の log が `Host::log` に届く、が通る
  - _Boundary: ThreadGuard_
  - _Depends: 1.1, 2.1_
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 5.3_

- [ ] 5.2 dir を検査して見た目だけ正規化し、既存の dir を受ける口に掛ける
  - 絶対パスでない（Windows の `\ghost`・`C:ghost` を含む）か UTF-8 で表せない dir を `InvalidPath` で断る
  - Windows では `/` を `\` にそろえる（`\\?\` で始まるものは置き換えない）。末尾の区切りをちょうど 1 つにする。ファイルシステムには触れない
  - `GhostRuntime::load` と `from_static` が dir を `Path` として受けて検査と正規化を掛ける（`&str` で呼ぶコードはそのままビルドできる）。`InvalidPath` の doc と `Display` を新しい意味に直す
  - 既存のテストの dir を OS ごとの絶対パスに直す
  - 完了: OS ごとの表のテスト（末尾の区切りの有無が同じ結果、`C:/a/b` → `C:\a\b\`、`C:\`・`/`・UNC・`\\?\` の扱い、相対・`\ghost`・`C:ghost`・UTF-8 でない → `InvalidPath`）と既存のテストが通る
  - _Requirements: 8.3, 8.4, 8.5, 8.6, 8.7, 8.8_

- [ ] 5.3 読み終えたライブラリから v4 のランタイムを作る入口と、ライブラリを返すエラーを作る
  - dir の検査 → `shiori_get_api` を探す → 版 4 のテーブルを検査 → load の順で作る。`NotShiori4`・`Unsupported`・`InvalidPath` ではエラーと一緒にライブラリを返し、`LoadFailed` では返さない
  - ライブラリを返すかどうかを型の中（非公開のフィールド）で守り、`LoadFailed` で返さない理由を doc に書く。エラーは `Display`・`Error`・`LoadError` への変換を持つ
  - 既存の `GhostRuntime::load` を、ライブラリを開いてこの入口に渡す形にしてよい（形と結果は変えない）
  - 完了: テスト用 DLL を使うテストで、成功・OS のライブラリで `NotShiori4` とライブラリ・モードで v4 を出さず `Unsupported` とライブラリ・相対の dir で `InvalidPath` とライブラリ・dir に `fail` で `LoadFailed` かつライブラリ無し、が通る。`from_static` で load が NULL のとき `LoadFailed` になるテストも通る
  - _Depends: 1.2_
  - _Requirements: 7.6, 12.1, 12.2, 12.3, 12.4_

- [ ] 5.4 `Host`・`GhostRuntime` の doc を規約に合わせ、エラー定数の再エクスポートの漏れを直し、Charset を付けないことをテストで守る
  - 再エクスポートに -7 と、今漏れている UTF-8 のエラーを足す
  - `Host` の doc で log を「任意のスレッド」に移し、log・`accept_post`・`wake` に「ランタイムを Drop するスレッドを同期で待たないこと」を書く
  - クレートの doc の「規約との対応」に規約 2・規約 5（SHIORI は join しない）・dir の検査と正規化・Charset を付けないことを足し、例の dir を絶対パスを組み立てる形に直す。ランタイムの Drop のコメントを規約 5 の言葉に直す
  - 完了: テストで、組み込みの応答と完了通知の見出しに Charset が無いことが通り、`shiori4_host` を use するだけで全 `SHIORI_E_*` が使え、doc のテストが通る
  - _Requirements: 1.3, 2.5, 5.4, 9.2_

- [ ] 6. DLL を読まずに、ファイル名の候補を並べ、ファイルの機械の種類を確かめる
  - 対象の OS・arch の型と、今のプロセスの OS・arch を表す定数（表に無ければ None）を作る
  - 名前の幹（または `shiori,` の値）から、ディレクトリの部分をそのまま残し、名前の部分にだけ表の規則を当てて、探す順に候補を返す。ファイルにもライブラリにも触れない。doc は C ヘッダの「配布の決まり」を指す
  - ファイル先頭の PE・ELF・Mach-O（単体・ユニバーサル）のヘッダから機械の種類を読み、arch と合うかを返す。読む量に上限を置き、すべての範囲を確かめ、パニックしない。ARM64EC と ARM64X の 2 つの例外を doc に書く
  - 対象の OS・arch の型と 2 つの関数を `shiori4_host` の直下に再エクスポートする（5.4 と同じ lib.rs に触れるので並行しない）
  - 完了: テストで候補の表を全部たどり（`pasta`・`pasta.dll`・`PASTA.DLL`・ディレクトリ付き）、合成したヘッダ（PE x64／arm64／x86、ELF x86_64／aarch64／32bit、Mach-O 単体とユニバーサル、Java の class、空・短い・範囲外）と無いファイルで期待どおりの結果になり、今のテストの実行ファイルが今の arch に合うことが 3 OS で通り、clippy の `-D warnings` も通る
  - _Requirements: 14.3, 14.4, 14.5, 14.6_

- [ ] 7. 新しい読み込みの口
- [ ] 7.1 v3 互換モードの口を作る
  - パス・読み終えたライブラリ・同じプロセスのテーブルの 3 つの入口を持ち、どれも dir に検査と正規化を掛け、host を渡さずに v3 で load する。NULL なら理由なしの `LoadFailed`
  - 読み終えたライブラリの入口は v4 の入口と同じ規則でライブラリを返す（`NotShiori4`・`Unsupported`・`InvalidPath`）
  - 要求の全文（GET も NOTIFY も）を文字列で受け、応答を UTF-8 で検査してから文字列で返す。UTF-8 でなければ壊れた文字列を作らず UTF-8 のエラー、パニックの後は常にパニックのエラー。post と完了通知は持たない
  - 後始末は unload → ライブラリの解放の順で、v4 のランタイムと同じ表し方にする
  - v3 互換モードの口を `shiori4_host` の直下に再エクスポートする
  - 完了: テストで GET の往復・NOTIFY の全文・load の NULL で `LoadFailed`・UTF-8 でない応答でエラー・Drop で unload が 1 回呼ばれる、が通り、テスト用 DLL で v4 の入口が `Unsupported` で返したライブラリをそのまま渡して往復できる。clippy の `-D warnings` も通る
  - _Depends: 1.3, 5.2, 5.3_
  - _Requirements: 8.8, 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 15.3_

- [ ] 7.2 許可する口の種類と発見の順で DLL を読む口を作る
  - 許可する口の種類（V4・V3・Loadu・Load、すべてを表す定数）と、解決した口ごとの結果（v4 のランタイム・v3 互換モードの口・従来の口とライブラリ）を作る
  - dir の検査 → ライブラリを開く → v4 → v3 → `loadu` → `load` の順で、許可された口だけを試す。`shiori_get_api` があり V4・V3 のどちらかを許可しているなら、結果にかかわらず従来の口へ進まない。従来の口は有無だけを見て呼ばない
  - 許可を渡さない口はすべてを許可したものとして扱う。許可された口が 1 つも無ければ `NotShiori4`（doc を「許可された口が見つからない」に広げる）。機械の種類は確かめず、信頼できる DLL だけを渡すことを doc に書く
  - 許可する口の種類・解決の結果・発見の 2 つの関数を `shiori4_host` の直下に再エクスポートする
  - 完了: テスト用 DLL で、既定で V4／v4 を出さないと V3／両方出さないと `Unsupported`（`loadu` があっても進まない）／dir に `fail` で `LoadFailed`／`[V3]` で V3／`[Loadu, Load]` で従来の `Loadu`／`[Load]` で従来の `Load`／`[]` で `NotShiori4`／OS のライブラリで `NotShiori4`／相対の dir で `InvalidPath`、がすべて通り、clippy の `-D warnings` も通る
  - _Depends: 7.1_
  - _Requirements: 8.8, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7, 11.8, 11.9_

- [ ] 8. 統合と仕上げ
- [ ] 8.1 クレートの doc に新しい読み込みの口への案内を足す
  - `shiori4_host` のクレートの doc に、発見の順と許可、読み終えたライブラリからの入口、v3 互換モードの口、配布の決まり（候補と機械の種類）への案内を足す
  - 案内の中の例は、クレート直下の名前だけを使う doc のテストにする
  - 完了: `cargo test -p shiori4_host --doc` で新しい例が通り、利用者が `shiori4_host::` の直下の名前だけで新しい口を使えることが確かめられる
  - _Depends: 5.4, 6, 7.2_
  - _Requirements: 11.2, 12.1, 13.1, 14.3_

- [ ] 8.2 (P) steering の `tech.md` を規約に合わせる
  - スレッド規約の「join してから返る」を結果の約束（unload の後はホストを呼ばない。高水準層が Poster を無効にするので join は要らない）に直し、`host.request` の期間と -7、post と log は任意スレッド、を足す
  - Testing にテスト用 cdylib の使い方を 1 行足す
  - 完了: `tech.md` に join を求める記述が無く、-7 とテスト用 cdylib に触れている
  - _Boundary: SteeringUpdate_
  - _Requirements: 1.3_

- [ ] 8.3 全体の検証と移行の記録
  - ワークスペースで `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings` を通す
  - 依存を確かめる: `shiori4_abi`・`shiori4` は std だけ、`shiori4_host` は `libloading` だけで新しい外部の依存が無く、3 クレートの版が上がっていない。テスト用 cdylib がワークスペースの外にあり、package と clippy の対象にならない
  - リポジトリの doc と steering に、join を求める記述・相対の dir の例が残っていないことを検索で確かめる
  - 動きが変わる 4 点（Poster の `Copy`、dir の `InvalidPath`、-7、エラーの意味の広がり）と移行の仕方を、実装のコミットと PR の本文に書ける形にまとめる
  - 完了: 上のコマンドがローカルで成功し、依存と版の確認と検索の結果に問題が無い
  - _Requirements: 1.3, 15.1, 15.2, 15.3, 15.4, 15.5, 15.6_

## Implementation Notes
- テスト用 DLL のモードの値（MODE_V4=1・MODE_V3=2・既定 3）は `testdata/testghost/src/lib.rs` にだけある。モードを切り替えるタスクは `shiori4_host/src/test_dll.rs` に同じ値の定数を足し、`test_dll::lock()` を取ってから `testghost_set_mode` を呼ぶ
- 既存コードは rustfmt の既定に合わせていない（rustfmt.toml なし）。`cargo fmt` は掛けず、周りの書き方に合わせる
- この環境には C コンパイラが無い。C ヘッダの `SHIORI_STATIC_ASSERT` は読んで確かめる
- 2.1: `ShioriHostPoster` から `Copy` を外した（移行は `.clone()`）。ワークスペースの中に暗黙のコピーは無かった。門は `shiori4_abi` の `PosterGate`（Mutex+Condvar）で、`ShioriHostHandle::new(api, gate)` は `pub(crate)`
