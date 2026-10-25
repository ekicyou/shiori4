# Implementation Plan

- [x] 1. i686 の Windows のビルドの環境を整える
  - 手元の toolchain に `i686-pc-windows-msvc` の target を足し、ワークスペースを i686 で作れることを確かめる
  - 既存のコード（testghost を含む）が i686 でそのままビルドでき、警告が出ないことを確かめる
  - 完了の状態: `cargo build --workspace --target i686-pc-windows-msvc` と `cargo build --manifest-path testdata/testghost/Cargo.toml --target i686-pc-windows-msvc` が通る
  - _Requirements: 9.1, 12.4_

- [x] 2. 土台: テスト用の DLL と HGLOBAL の受け渡し
- [x] 2.1 (P) 従来の口を持つテスト用の DLL を作る
  - 外部の crate に依存しない cdylib として、`loadu`・`load`・`request`・`unload` の本物の口を持たせる。入力は記録してから DLL の側で解放し、応答は長さ＋1 バイトを固定のグローバルメモリで確保して末尾に NUL を置く
  - dir に `fail` を含むと初期化は 0 を返し、それ以外は設定した戻り値（既定 1）を返す
  - 応答の種類（OK・NULL・負の長さ・受けた要求をそのまま返す）を切り替えられ、呼ばれた口の数・解放した入力の数・確保した応答の数・最後に受けた入力を、テスト用の書き出しで読める。状態を既定に戻す書き出しも持つ
  - `shiori_get_api` を持たず、公開しない（`publish = false`・自前の workspace。testghost と同じ形）
  - 完了の状態: i686 で DLL がビルドでき、4 つの従来の口と 5 つのテスト用の書き出しが export されている
  - _Boundary: LegacyGhost_
  - _Requirements: 8.1, 8.2, 8.4_

- [x] 2.2 (P) x86 の Windows だけのモジュールの骨組みと HGLOBAL の受け渡しを作る
  - x86 の Windows でだけビルドに入るモジュールを足し、公開の型の再輸出の場所を作る。ほかの環境のビルドと API には何も現れない
  - kernel32 の 4 関数を、外部の crate なしに直接宣言する
  - 要求の失敗の列挙（確保の失敗・NULL・負の長さ・UTF-8 でない）の値だけをここで足す
  - 入力を `max(len, 1)` バイトの固定のグローバルメモリに NUL を付けずに写す処理と、応答を NULL・負の長さ・正常で分けて長さだけ写して解放する処理を作る。確保と解放の数はテストのときだけ数える
  - 完了の状態: 自前で確保したグローバルメモリを使う単体テストで、正常・NULL・負の長さ（解放される）・長さ 0 の扱いが i686 で通り、x64 のビルドは変わらない
  - _Boundary: HglobalTransfer, LegacyRequestError_
  - _Requirements: 1.1, 1.2, 3.5, 4.1, 4.2, 4.5, 4.6_

- [x] 2.3 テスト用の DLL をテストと同じ target で作る補助を整える
  - テスト用の DLL のビルドの共通部分を 1 つにまとめ、testghost と新しい DLL で使い回す
  - x86 の Windows のテストでは target の三つ組を付けて作り、出力の場所に三つ組を入れる。ほかの環境の出力の場所と振る舞いは変えない
  - 新しい DLL の場所と、そのテスト用の書き出し（数・最後の入力・応答の種類・初期化の戻り値・既定に戻す）を呼ぶ薄い補助を、x86 の Windows のテストにだけ足す
  - 2.1 の DLL が要る（2.2 とは独立）
  - 完了の状態: x64 の `cargo test` が今のまま通り、i686 の `cargo test` で testghost と新しい DLL が三つ組の付いた場所に作られる
  - _Requirements: 8.1, 9.1_

- [ ] 3. 芯: 従来の DLL の実行時の値
- [ ] 3.1 load に渡す dir の変換を作る
  - `loadu` には正規化した dir の UTF-8、`load` には CP_ACP のバイト列を作る。CP_ACP は OS の変換の機能に任せ、best-fit をせず、表せない文字があったかを返す
  - ACP が 65001 のときは UTF-8 をそのまま CP_ACP のバイト列として使い、表せない文字は無いとする
  - 完了の状態: ASCII は UTF-8 と同じで表せない文字なし、サロゲート対の絵文字は（ACP が 65001 でなければ）`?` に置き換わって表せない文字ありになる、`loadu` 用は UTF-8 そのもの、の単体テストが i686 で通る
  - _Requirements: 1.4, 3.2, 3.3, 3.4_

- [ ] 3.2 初期化の入口を作る
  - 発見の口が返すライブラリと口の種類と dir を受け、口・`unload`・`request` の解決、dir の検査と変換、入力の確保を済ませてから、`loadu` か `load` を 1 回だけ呼ぶ。どこかで失敗したら DLL を呼ばずにライブラリを解放する
  - 口の種類が従来の口でないか、必須の書き出しが揃わなければ `NotShiori4`。dir が不正なら `InvalidPath`。変換・確保の失敗と BOOL が 0 なら `LoadFailed`（`unload` は呼ばない）。BOOL は 32bit で 0 以外を成功と読む
  - 値を作らずに dir の表せない文字を確かめる関連関数を足す（dir の検査は初期化と同じ）
  - 完了の状態: 新しい DLL を使う結合テストで、`loadu` を選ぶ・`load` だけを許すと `load` を選ぶ・`fail` の dir で `loadu` 1 回だけで `load` も `unload` も呼ばれない・testghost では `NotShiori4`・BOOL の 2・0x100・-1 が成功で 0 が失敗・`load` に CP_ACP の dir（ASCII でない文字を含む）、`loadu` に UTF-8 の dir が届く・関連関数が ASCII の dir で偽を返す、が i686 で通る
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.4, 6.2, 8.3, 8.4_

- [ ] 3.3 後始末とスレッドの制約と型の doc を作る
  - 値は別のスレッドへ送れず、手放すと `unload` を 1 回呼んでからライブラリを解放する。手放した後は関数を呼べない
  - 型の doc に、1 つのモジュールに 1 インスタンス（2 体目は別のファイルか別のプロセス）・ホストに残すもの・発見の口が返した信頼できる DLL だけを渡すこと・`ghost_dir` には発見の口と同じ dir を渡すことを書く。この型の doc は 3.3 が持ち、7.1 は書き直さない
  - `!Send` を `compile_fail` の doc の例で確かめる
  - 完了の状態: 別のハンドルを持って手放すと `unload` が 1 回、持たずに手放すとモジュールが外れる結合テストと、`compile_fail` の doctest が i686 で通る
  - _Requirements: 2.7, 6.1, 6.3, 6.4, 8.3_

- [ ] 3.4 要求の往復（芯のバイト列の口と文字列の口）を作る
  - 要求の失敗の列挙に英語 1 行の Display とエラーの trait を足す
  - 芯の口は要求のバイト列をそのまま固定のグローバルメモリに写して渡し、応答を長さのまま返す。確保できなければ DLL を呼ばない。失敗の後も値は使い続けられる
  - 文字列の口は芯の口の上で、末尾に続く NUL をすべて落としてから UTF-8 を確かめるだけで、Charset を見ない
  - 完了の状態: 発見の口から `Legacy { entry: Loadu }` に解決して初期化し `SHIORI/3.0 200 OK` と `Value: legacyghost` が返る・NULL・負の長さ（応答の確保と解放の数が一致）・NUL を含む応答（芯は 7 バイトのまま、文字列の口は末尾の NUL だけ落ちる）・Shift_JIS（芯はそのまま、文字列の口は UTF-8 の失敗）・空の要求・初期化と数回の往復と手放しの後に入力と応答の確保と解放の数がそれぞれ一致・Display の文、の結合テストが i686 で通る
  - _Requirements: 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 5.1, 5.2, 5.3, 5.4, 8.2, 8.3_

- [ ] 4. x86 の Windows の発見の口を、絶対パスだけを開く形にする
  - x86 の Windows でだけ、ライブラリのパスが絶対パスでなければ開かずに `InvalidPath` を返し、絶対パスなら今の開き方（開く間のダイアログの抑止は libloading に任せる）で開く。探索の場所を変える設定は使わない。ほかの環境の開き方は変えない
  - `InvalidPath` の doc を「dir かライブラリのパスが絶対パスでない」に広げ、Display の文は x86 の Windows でだけ広げる（`InvalidPath` の doc と Display はこのタスクが持つ）
  - 既存のテストで OS のライブラリを相対の名前で読んでいる所を、絶対パスに直す
  - 完了の状態: i686 で、相対のパスが `InvalidPath`・在らない絶対パスが `Library`・絶対パスの kernel32 が `NotShiori4`、のテストが通り、x64・ubuntu・macos の既存のテストが変わらず通る（発見から往復までのテストは 3.4 が持つ）
  - _Requirements: 1.3, 2.6, 7.1, 7.2, 7.3, 12.1_

- [ ] 5. 試験用のホストの x86 の Windows の枝で、従来の口を往復させる
  - x86 の Windows でだけ `--entry legacy` を受け付けて許可する口を `loadu`・`load` にする。ほかの環境では今のまま引数の誤り
  - 従来の口に解決したら初期化し、v3 互換モードと同じ要求の全文を 1 回往復させ、末尾の NUL を落として読めないバイトを置換文字にした応答から状態を取って表示し、手放して `unloaded` を出す
  - 初期化の失敗は終了の状態 3、要求の失敗は stdout に `request error: <名前>: <説明>` を出して 2、2xx は 0、それ以外は 1。x86 の Windows では 4 を出さず、使わない定数は cfg で外す。ほかの環境の枝は今のまま 4
  - タスク 4 の後なので、x86 の Windows では SHIORI のファイルを絶対パスで渡す（相対は `InvalidPath` で 3）
  - 冒頭の `//!` と使い方の文を新しい引数と振る舞いに合わせる
  - 完了の状態: i686 で新しい DLL を絶対パスで読むと終了の状態 0 で `Value: legacyghost` を表示し、`fail` の dir で 3、testghost に `--entry legacy` で 3、相対のパスで 3 になる。引数の読み取りのテスト（x86 では `--entry legacy` が `loadu`・`load` を許可する）が通り、x64 の既存の振る舞いは変わらない
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6_

- [ ] 6. i686 の Windows の CI の job を足す
  - windows の runner に i686 の target と clippy を入れ、ワークスペースのテスト（doctest を含む）と clippy（`-D warnings`）を i686 で走らせる
  - テスト用の 2 つの DLL と試験用のホストを i686 で作り、既存の job と同じ形で、絶対パスを渡して終了の状態を確かめる（新しい DLL で 0・`--entry legacy` で 0・`fail` の dir で 3・testghost で 0・testghost に `--entry legacy` で 3）。往復の出力に `Value: legacyghost` があることも確かめる
  - 既存の job は変えない
  - 完了の状態: build.yml に新しい job があり、同じ手順を手元の i686 で流して全部が期待どおりの終了の状態になる
  - _Requirements: 9.1, 9.2, 9.3, 10.6, 11.7, 12.4_

- [ ] 7. 文書
- [ ] 7.1 (P) コードの doc と docs.rs の設定を直す
  - モジュールの `//!` に、x86 の Windows でだけあること・ほかの環境では現れないこと・入口（発見の口 → `Legacy` → 初期化）・ホストに残すもの・`load` の dir が CP_ACP である理由を書き、例は `no_run` で i686 の doctest でビルドされる形にする（型の doc は 3.3 のまま）
  - crate doc の「DLL の選び方と読み方」に従来の DLL の口の項目を足し、例の `Legacy` の枝の文を x86 の Windows では読めることに合わせる。全環境でビルドされる所に x86 だけの型への intra-doc link を書かない
  - `LoadedGhost::Legacy` と発見の口の doc を、x86 の Windows の初期化と開き方（相対のパスは `InvalidPath`）に合わせる（`InvalidPath` 自体の doc はタスク 4 で済んでいる）
  - docs.rs の `targets` に既定の対象と `i686-pc-windows-msvc` を挙げる
  - 完了の状態: x64 と i686 の両方で `cargo doc` と doctest が警告なしに通り、`Cargo.toml` に 2 つの target がある
  - _Boundary: Docs（コードの doc・Cargo.toml）_
  - _Depends: 4_
  - _Requirements: 11.1, 11.7, 11.8_

- [ ] 7.2 (P) 教科書と steering を直す
  - 移行の手引きの `load` の「既定の OEM コードページ」を CP_ACP に直して ukadoc の字面と違う理由を添え、「旧来の DLL を読む口はまだ無い」の項目をこの口（使える環境・入口の名前）を指す文に書き換える
  - 配布の決まりの章の同じ趣旨の文と、例の `Legacy` の枝の文を書き換える。配布の決まりの写しの検査に掛かる所には触れない
  - 試験用のホストの章に、x86 の Windows の振る舞い（`--entry legacy`・`request error:`・終了の状態 2 と 4 の意味・SHIORI のファイルは絶対パス・出力の例）を、ほかの環境と分けた節で足す
  - steering の tech.md の CI の job の一覧・Testing の節・`shiori4_host` の構成を、この spec の後の形に合わせる
  - 完了の状態: 4 つの文書が書き換わり、教科書の検査のテスト（`shiori4_abi/tests/textbook.rs`）が通る
  - _Boundary: Docs（教科書・steering）_
  - _Depends: 5, 6_
  - _Requirements: 10.6, 11.2, 11.3, 11.4, 11.5, 11.6_

- [ ] 8. 全環境の互換と依存を確かめる
  - x64 の Windows で `cargo test --all`・clippy（`-D warnings`）・`cargo deny check`・`cargo package` が通る
  - i686 で `cargo test --all --target i686-pc-windows-msvc` と clippy（`-D warnings`）が通り、タスク 6 の往復の手順が手元で期待どおりの終了の状態になる
  - `cargo tree` の依存の木（i686 以外）と 3 クレートの版が変わっていない
  - 統合の関門: この PR は CI を変えるので、PR の CI（新しい job を含む）が通るのを見届けてから統合する
  - 完了の状態: 上の各コマンドの結果が揃って成功している
  - _Requirements: 1.2, 1.3, 9.3, 9.4, 12.1, 12.2, 12.3, 12.4_

## Implementation Notes
- testdata/legacyghost は kernel32 を直接リンクし cfg(windows) の guard を持たないので、Windows でだけビルドする（2.3 の補助も Windows に限る）。clippy・test はルートの workspace に入らないので manifest を指定して走らせる。
- test_dll.rs の legacyghost の応答の種類の定数は RESPONSE_ECHO だけ。OK/NULL/NEGATIVE（0/1/2）は使うタスクで足す。LegacyStats は DLL の struct の手写しなので両方を揃えて直す。
