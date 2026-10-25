# Requirements Document

## Project Description (Input)
伺かのゴーストの SHIORI は、ほとんどが従来の SHIORI/3.0 の DLL である（HGLOBAL の `load`・`loadu`・`unload`・`request` を書き出し、x86 の Windows で動く）。ベースウェアの作者が `shiori4_host` を使っても、今は v4（と v3 互換モード）の DLL しか読めず、従来の DLL を読みたければ HGLOBAL の受け渡しを自分で書くしかない。areka は自前の 32bit の helper（`shiori_proxy.rs`）でそれを書いており、SHIORI の作者（pasta）は自分の HGLOBAL の入口を CI で試す手段を持っていない。

いまの `shiori4_host` は `libloading` で DLL を開き、発見の口（`load_ghost`／`load_ghost_with`）が `shiori_get_api` → `loadu` → `load` の順に口を探す。従来の口は書き出しの有無だけを見て呼ばず、`LoadedGhost::Legacy { entry, library }` でライブラリごと返す（abi-conventions）。試験用のホスト `shiori4-testhost` は、その枝で何も呼ばずに終了の状態 4 で終わる（abi-textbook）。

この spec では、x86 の Windows に限って、`shiori4_host` を従来の SHIORI/3.0 の DLL のホストとして使えるようにする。DLL に触る芯（HGLOBAL の受け渡し・loadu と load の選び方・dir の渡し方・ダイアログの抑止・unload の順）だけを置き、プロセスの起こし方・IPC・待ち時間・強制終了・文字コードの方針はホストに残す。areka の helper はこの芯に乗り換えられ、pasta は自分の HGLOBAL の入口を CI で試せるようになる。

## Introduction
この spec は、x86 の Windows（`cfg(all(windows, target_arch = "x86"))`）でだけ有効な、従来の SHIORI/3.0 の DLL を読んで往復させる口を `shiori4_host` に足す。発見の口が `LoadedGhost::Legacy` で返したライブラリを、HGLOBAL の決まり（SHIORI/3.0 の DLL 共通仕様）どおりに初期化し、要求を往復させ、unload して解放する。芯の口はバイト列を受け渡し、その上に UTF-8 であることだけを確かめる文字列の口を重ねる。中身（ヘッダ・Charset・語彙）は解釈しない。

あわせて、確保と解放を数える試験、i686 の Windows の CI、試験用のホストのつなぎ、文書の書き換えを行う。

要件の根拠は brief.md の「決まっていること」1〜10 と「開発者が裁定した論点」a〜d で、各要件の目的の末尾に「（決定 N）」「（裁定 x）」で示す。brief.md で決め切れていなかった所は要件ディスカッション（2026-10-25）で決め、本文に「（議題 N）」で示す。決めた中身と理由は末尾の「要件ディスカッションの決定」に置く。

## Boundary Context
- **In scope**:
  - x86 の Windows だけで有効な、従来の SHIORI/3.0 の DLL を読む口: 書き出しの解決、`loadu`／`load` による初期化、`load` に渡す dir の変換、`request` の HGLOBAL の受け渡し、`unload` と解放の順、ライブラリを開くときのエラーのダイアログの抑止。
  - 芯のバイト列の口と、その上の文字列の便利な口（裁定 a・b）。
  - 確保と解放を数える試験と、従来の口を持つテスト用の DLL。
  - i686 の Windows での CI（ビルド・テスト・試験用のホストの往復）。
  - abi-textbook の試験用のホストが、x86 の Windows で従来の DLL を読んで往復するつなぎ。
  - 文書（どの環境で使えるか・ホストに残すもの・1 つのモジュールに 1 インスタンス・移行の手引きと試験用のホストの章の書き換え）。
- **Out of scope**:
  - プロセスの起こし方、IPC、待ち時間、強制終了、カレントディレクトリの決め方（ホストが持つ。決定 10）。
  - SHIORI/2.x の文字列の形（要求の組み立て・応答の読み分け・版の見分けと切り替え）。spec `shiori2-helpers` が便利関数として持つ（裁定 c を 2026-10-25 に改めた）。芯は中身を解釈しないので、SHIORI/2.x の要求もバイト列のまま運ぶ。
  - x86 の Windows 以外の環境で従来の DLL を読むこと。
  - 本文の文字コードの方針（Charset を見ての変換・切り替え）。ホストが持つ（裁定 a）。
  - 中身の解釈（要求・応答のヘッダ・Charset・語彙。方針「中身の通信には関与しない」）。
  - DLL の中の例外・クラッシュ・応答しない DLL からの回復（別のプロセスで読む・強制終了するのはホスト。決定 10）。
  - v4 と v3 互換モードの口そのもの・発見の順・許可する口の種類の決まり（abi-conventions が持つ。この spec は前提にする）。
  - SHIORI の側の不具合の取り繕い（Rust の `bool` を返す `load` の上位 24bit など。裁定 d）。
  - 版を上げること（`/release` が上げる）。
  - x86 の配布の決まり（`TargetArch` に x86 を足すこと・`machine_matches` で i386 の PE を確かめること）。x86 のホストはファイルを自分で選んで渡す（議題 11）。
  - ダイアログの抑止を x86 の Windows 以外（x64 の Windows・`GhostRuntime::load`・`V3Runtime::load`）へ広げること（議題 7）。
- **Adjacent expectations**:
  - abi-conventions: 発見の口（`load_ghost`／`load_ghost_with`）・`EntryKind`・`LoadedGhost::Legacy`・dir の検査と正規化（`InvalidPath`）・`LoadError` の意味を、この spec は変えずに前提にする。`shiori_get_api` を持つ DLL は、失敗しても従来の口へフォールバックしない（決定 9）。
  - host-protocol: 従来の DLL のホストは `Host` trait を要しない（従来の ABI にはホストを呼ぶ口が無い）。
  - abi-textbook: 試験用のホストの終了の状態・引数・表示は下流の CI の契約で、変えるときは `doc/textbook/testhost.md` と CI の job `textbook` も直す。移行の手引きの「旧来の DLL を読む口は `shiori4_host` にまだ無い」を、この spec の口を指す文に書き換える（abi-textbook 要件 11.3）。
  - 下流: areka の helper（`shiori_proxy.rs` の大半を置き換える）、pasta の HGLOBAL の入口の CI、ほかのベースウェアの作者。

## Requirements

### Requirement 1: 有効な環境とビルドの範囲
**Objective:** As a ベースウェア実装者・3 クレートの利用者, I want 従来の DLL を読む口が x86 の Windows でだけ現れ、ほかの環境のビルドと API を変えないこと, so that 32bit の helper では従来の DLL を読め、それ以外の環境の利用者は何も変わらない（決定 1・Constraints）

#### Acceptance Criteria
1. Where ビルドの対象が x86 の Windows である, the shiori4_host shall 従来の SHIORI/3.0 の DLL を読む口を公開する。
2. Where ビルドの対象が x86 の Windows でない, the shiori4_host shall 従来の DLL を読む口と、そのためだけの依存を、ビルドにも公開の API にも含めない。
3. Where ビルドの対象が x86 の Windows でない, the shiori4_host shall 発見の口・`LoadedGhost::Legacy`・`LoadError` を含む既存の API の形と振る舞いを、この spec の前と同じに保つ。
4. The shiori4_host shall 本文の文字コードの変換器（ANSI のエンコーダー）を自分で持たず、変換が要る所（要件 3 の `load` の dir）は OS の変換の機能に任せる。

### Requirement 2: 従来の口の解決と初期化
**Objective:** As a ベースウェア実装者, I want 発見の口が従来の口で解決したライブラリを、DLL を読み直さずに `loadu` か `load` で 1 回だけ初期化できること, so that 二重の初期化を起こさずに、従来の DLL を SHIORI/3.0 の約束どおりに動かせる（決定 2・8・9・裁定 d）

#### Acceptance Criteria
1. When ホストが発見の口で従来の口（`loadu`・`load`）に解決したライブラリを初期化する, the shiori4_host shall そのライブラリを開き直さずに、発見の口が解決した口（`loadu` を先に見て、無ければ `load`。許可する口の種類に従う）を使う。初期化の入口は、発見の口が返す `LoadedGhost::Legacy` のライブラリを受けて従来の DLL の実行時の値を作る形だけにし、ライブラリのパスから直接読む別の入口は作らない（決定 9 の「別の入口は作らない」。議題 1）。
2. The shiori4_host shall 初期化のとき、`loadu` か `load` のどちらか片方だけを 1 回だけ呼ぶ。
3. If ライブラリが `unload` か `request` を書き出していない, the shiori4_host shall `loadu`・`load` を呼ばずに `LoadError::NotShiori4`（許可した口が見つからない）を返す。必須の書き出しがそろわない従来の口は口として成り立たないと読み、`LoadError` に値を足さない（議題 2）。
4. When `loadu`・`load` が戻る, the shiori4_host shall 戻り値を 32bit の整数として読み、0 以外を成功、0 を失敗とする（元の C ABI の BOOL）。上位のビットを捨てて下位 1 バイトだけで読む守りは持たない（裁定 d）。
5. If `loadu`・`load` が失敗を返す, the shiori4_host shall `LoadError::LoadFailed` を返し、ほかの口・ほかの版へはフォールバックしない（`loadu` が失敗しても `load` へは戻らない）。このとき `unload` は呼ばずにライブラリを解放する（議題 3）。
6. The shiori4_host shall 発見の順（`shiori_get_api` → `loadu` → `load`）と、`shiori_get_api` を持つ DLL を従来の口で読まないことを、abi-conventions の決まりのまま保つ。従来の口だけで読みたいホストは、許可する口の種類で `loadu`・`load` だけを許可して読む（決定 9）。
7. The shiori4_host shall 従来の DLL を作ったスレッドからだけ `load`・`request`・`unload` を呼べる形にする。v3 互換モードの口と同じく、別のスレッドへ送れない値（`!Send`）にする（議題 4）。

### Requirement 3: load に渡す dir
**Objective:** As a 従来の SHIORI の作者・ベースウェア実装者, I want `loadu` と `load` に渡す dir が決まった形と文字コードで届くこと, so that `dir + "file"` と書く古い SHIORI も壊れず、`loadu` を持たない DLL も今までどおり動く（決定 2・7）

#### Acceptance Criteria
1. The shiori4_host shall 従来の口に渡す dir を、v4・v3 互換モードと同じ検査と正規化（絶対パス・`\` の区切り・末尾に `\` が 1 つ）に通し、絶対パスでないか UTF-8 で表せない dir は `LoadError::InvalidPath` で断る。
2. When `loadu` で初期化する, the shiori4_host shall 正規化した dir の UTF-8 のバイト列を渡す。
3. When `load` で初期化する, the shiori4_host shall 正規化した dir を、OS の既定の ANSI コードページ（CP_ACP）のバイト列に変えて渡す。古い SHIORI は dir を `fopen`・`CreateFileA` などの `A` 系の API に渡し、それらが読むのは CP_ACP なので、ukadoc の DLL 共通仕様の字面（「既定の OEM コードページ」）ではなく CP_ACP に従う（決定 2・議題 5）。
4. If dir に CP_ACP で表せない文字がある, the shiori4_host shall そのことをホストが知ることのできる形（log か警告）で知らせ、変換した dir のまま `load` を呼ぶ（初期化を止めない）。知らせ方（どこへ・どの形で）は設計で決める。
5. The shiori4_host shall dir を、要求と同じ固定のグローバルメモリ（要件 4.1）にちょうどその長さだけ NUL 終端を付けずに書いて渡し、長さの引数にはそのバイト数を渡す。

### Requirement 4: request の受け渡し（芯のバイト列の口）
**Objective:** As a ベースウェア実装者, I want 要求のバイト列を渡すと応答のバイト列が返り、HGLOBAL の確保と解放を SHIORI/3.0 の約束どおりに shiori4_host が持つこと, so that HGLOBAL の受け渡しを自分で書かずに、Shift_JIS を含むどんな本文でも往復させられる（決定 3・裁定 a・b・方針）

#### Acceptance Criteria
1. When ホストが要求のバイト列を渡す, the shiori4_host shall 要求をちょうどその長さだけ書いた固定のグローバルメモリ（`GlobalAlloc(GMEM_FIXED)`）を確保し、NUL 終端を付けずに、その長さと一緒に `request` に渡す。
2. The shiori4_host shall `load`・`loadu`・`request` に渡した入力のメモリを、自分では解放しない（解放するのは DLL）。
3. If 入力のメモリを確保できない, the shiori4_host shall DLL を呼ばずに失敗を返す（初期化なら読めなかったゴースト、要求なら要求の失敗）。
4. If `request` が NULL を返す, the shiori4_host shall 要求の失敗を返す。失敗の表し方（ABI のエラーの値を使うか、別の型にするか）は設計で決める。
5. When `request` が NULL でない応答を返す, the shiori4_host shall 返された長さのバイト数だけを写し、写した後にその応答のメモリを解放する（`GlobalFree`）。NUL を探さず、長さだけで読む。
6. If `request` が返した長さが負である, the shiori4_host shall 応答を写さずに要求の失敗を返し、応答のメモリは解放する（壊れた応答として扱う。議題 6）。
7. The shiori4_host shall 芯の口で、応答を長さのまま（末尾の NUL があってもそのまま）返す（裁定 b）。
8. The shiori4_host shall 要求と応答の中身（開始行・ヘッダ・Charset・語彙）を解釈も変換もせず、原文のまま運ぶ（方針「中身の通信には関与しない」）。

### Requirement 5: 文字列の便利な口
**Objective:** As a UTF-8 の SHIORI を読むベースウェア実装者, I want バイト列の口の上に `&str` を渡して `String` を受け取れる口があること, so that UTF-8 の SHIORI なら変換を書かずに往復でき、ほかの文字コードの本文は芯の口で自分で扱える（裁定 a・b）

#### Acceptance Criteria
1. The shiori4_host shall 芯のバイト列の口の上に、要求を文字列で渡し応答を文字列で受け取る任意の口を持つ。
2. When 文字列の口で応答を受け取る, the shiori4_host shall 応答の末尾の NUL を落としてから UTF-8 であることを確かめる。落とすのは末尾に続く NUL のすべてとする（議題 6）。
3. If 応答が UTF-8 でない, the shiori4_host shall 壊れた文字列を作らずにエラーを返す（v3 互換モードの口の `SHIORI_E_INVALID_UTF8` と同じ扱い）。
4. The shiori4_host shall 文字列の口で、Charset を見ての変換や切り替えをしない。Shift_JIS などの本文を扱うホストは芯の口を使う。

### Requirement 6: unload と寿命・1 つのモジュールに 1 インスタンス
**Objective:** As a ベースウェア実装者, I want 後始末の順が決まり、同じモジュールで load を 2 回呼ばない形になっていること, so that 解放後の呼び出しや二重の初期化を起こさない（決定 4・8）

#### Acceptance Criteria
1. When ホストが従来の DLL の実行時の値を手放す, the shiori4_host shall `unload()` を 1 回呼び（戻り値は見ない）、その後にライブラリを解放する。
2. The shiori4_host shall 1 つの開いたライブラリに対して `loadu`・`load` を 2 回以上呼ぶ手段を公開しない（初期化の入口はライブラリを受け取って手放させる）。
3. The shiori4_host shall 「従来の ABI にはハンドルが無いので、1 つのモジュールに 1 インスタンスだけ。2 体目は別のファイルか別のプロセスで読む」ことを、従来の DLL の口の doc に書く。
4. The shiori4_host shall `unload` の後に、そのライブラリの関数を呼ばない。

### Requirement 7: ライブラリを開くときのダイアログの抑止とパス
**Objective:** As a ベースウェア実装者（無人の helper を含む）, I want DLL を開くときに OS のエラーのダイアログが出ず、決まった場所の DLL だけが読まれること, so that 依存の DLL が無いときにも helper が止まらず、探索の順で別の DLL を拾わない（決定 6）

#### Acceptance Criteria
1. While x86 の Windows で発見の口がライブラリを開いている, the shiori4_host shall OS の重大なエラーのダイアログ（依存の DLL が無いときなど。`SEM_FAILCRITICALERRORS`）をそのスレッドで止め、開き終えたら元の設定に戻す。止めるのは `libloading` が開く間に立てる旗に任せてよい。`SEM_NOOPENFILEERRORBOX` は `OpenFile` 関数にしか効かないので要件にしない（設計の議題 1。brief の決定 6 の旗の組み合わせを、効く所に絞った）。止めるのは x86 の Windows で発見の口がライブラリを開く間だけとし、v4・v3 の DLL を開くときも同じに扱う（開く時点では口の種類が分からないため）。ほかの環境と、x86 の Windows の `GhostRuntime::load`・`V3Runtime::load` の扱いは変えない（議題 7）。
2. The shiori4_host shall DLL の探索の場所を変える OS の設定（`SetDllDirectory`）を使わない。
3. If x86 の Windows で発見の口に渡されたライブラリのパスが絶対パスでない, the shiori4_host shall ライブラリを開かずに `LoadError::InvalidPath` を返し、`InvalidPath` の doc を「dir かライブラリのパスが絶対パスでない」に広げる。相対パスを OS の探索の順に任せると別の DLL を拾うおそれがあるため、決定 6「DLL は絶対パスで読み」を、相対パスを断る要件として読む（議題 8）。

### Requirement 8: 確保と解放を数える試験
**Objective:** As a shiori4 の保守者・SHIORI の作者, I want 従来の口の HGLOBAL の確保と解放が数で確かめられていること, so that 漏れと二重解放が無いことを CI で守れる（決定 3・P の要望）

#### Acceptance Criteria
1. The shiori4_host shall 従来の口（`loadu`・`load`・`request`・`unload`）の本物の書き出しを持つテスト用の DLL を読み、初期化・往復・unload を確かめるテストを持つ。
2. The shiori4_host のテスト shall ホストが確保した入力のメモリの数と DLL が解放した数、DLL が確保した応答のメモリの数とホストが解放した数が、往復の後にそれぞれ一致することを確かめる。数える手段はテストの中だけ（テスト用の DLL とテスト用の内部の口）に置き、公開の API と試験用のホストの表示には足さない（DLL が入力を解放したかはホストからは数えられないため。議題 9）。
3. The shiori4_host のテスト shall `loadu` を先に選ぶこと・`loadu` が無いと `load` を選ぶこと・`loadu` が失敗しても `load` へ戻らないこと・`unload`／`request` が無いと初期化しないこと・`load` に CP_ACP の dir、`loadu` に UTF-8 の dir が届くこと・`request` が NULL のときの失敗・応答を長さで読むこと（NUL を含む応答）・文字列の口の末尾の NUL と UTF-8 の検査・unload の後にライブラリを解放することを確かめる。
4. The shiori4_host のテスト shall BOOL の戻り値が 0 以外なら成功と読むこと（1 以外の 0 でない値を含む）を確かめる。

### Requirement 9: i686 の Windows の CI
**Objective:** As a shiori4 の保守者, I want i686 の Windows で従来の口がビルドされ、テストされること, so that 他の環境では含まれない口も、毎回の変更で壊れていないと分かる（Scope・Constraints）

#### Acceptance Criteria
1. The CI（`.github/workflows/build.yml`） shall i686 の Windows で、ワークスペースのビルド・`cargo test`・clippy（`-D warnings`）を走らせる。
2. The CI shall i686 の Windows で、試験用のホストが従来の口のテスト用の DLL を読んで往復することを、終了の状態で確かめる。
3. The CI shall 既存の ubuntu・windows（x64）・macos の job を、この spec の前と同じに通す。
4. The 開発の手順 shall この spec の変更の PR が CI を変えるので、PR の CI が通るのを見届けてから統合する（workflow.md）。

### Requirement 10: 試験用のホストのつなぎ
**Objective:** As a SHIORI の作者（pasta）・ベースウェア実装者, I want 試験用のホストが x86 の Windows で従来の DLL を読んで往復すること, so that 自分の HGLOBAL の入口を CI で試せる（Desired Outcome・abi-textbook の申し送り）

#### Acceptance Criteria
1. Where 試験用のホストのビルドの対象が x86 の Windows である, when 発見の口が従来の口に解決する, the 試験用のホスト shall その口で DLL を初期化し、SHIORI/3.0 の要求の全文を 1 回往復させ、結果を表示して unload する。要求の全文は v3 互換モードと同じ（`GET SHIORI/3.0`・`Charset: UTF-8`・5 つのヘッダ・`-H` のヘッダ・空行）とし、ステータスは応答の開始行から取る（議題 10）。
2. Where 試験用のホストのビルドの対象が x86 の Windows でない, the 試験用のホスト shall 従来の口の枝を今のまま（`entry: loadu`／`entry: load` と「従来の口は読まない」を表示して終了の状態 4）に保つ。
3. Where 試験用のホストのビルドの対象が x86 の Windows である, the 試験用のホスト shall 従来の口だけを許可して試すオプションを持つ（`shiori_get_api` と従来の口の両方を持つ DLL の、HGLOBAL の入口を試すため）。名前は `--entry legacy`（`loadu`・`load` を許可）とする（議題 10）。
4. When 従来の口の往復が終わる, the 試験用のホスト shall v3 互換モードと同じ終了の状態（2xx は 0・2xx 以外は 1・初期化の失敗は 3）で終わる。`request` の失敗（NULL など）は 2 で終わり、`abi error:` の代わりの行で理由を表示する。終了の状態 4 は x86 の Windows では出ない（議題 10）。
5. When 応答が UTF-8 でない, the 試験用のホスト shall 往復を失敗にせず、応答を表示用に置き換えて（読めないバイトを置換文字にして）表示する（議題 10）。
6. The 試験用のホスト shall 引数・表示・終了の状態を変えたら、`doc/textbook/testhost.md` の表と例、example の冒頭の `//!`、CI の job `textbook` を同じに直す。

### Requirement 11: 文書
**Objective:** As a ベースウェア実装者・SHIORI の作者, I want 従来の DLL を読む口がどの環境で使え、何がホストに残るかが文書で分かること, so that 32bit の helper を設計するときに、shiori4_host に任せる所と自分で書く所を間違えない（Scope）

#### Acceptance Criteria
1. The shiori4_host の doc（クレート冒頭の `//!` と従来の DLL の口の doc） shall 従来の DLL の口が x86 の Windows でだけ使えること、ほかの環境では現れないこと、入口の名前、ホストに残すもの（プロセスの起こし方・IPC・待ち時間・強制終了・カレントディレクトリ・本文の文字コードの方針）を述べる。あわせて、`LoadedGhost::Legacy` の doc と crate doc の例の「このクレートでは読まない」の記述を、x86 の Windows ではこの口で読めることに合わせて直す。
2. The 移行の手引き（`doc/textbook/migration.md`） shall 「旧来の DLL を読む口は `shiori4_host` にまだ無い」の項目を、この spec で足した口（使える環境・入口の名前）を指す文に書き換える（abi-textbook 要件 11.3）。
3. The 配布の決まりの章（`doc/textbook/distribution.md`） shall 「旧来の DLL を読む口は `shiori4_host` にまだ無い」の文を、同じくこの spec の口を指す文に書き換える。
4. The 試験用のホストの章（`doc/textbook/testhost.md`） shall x86 の Windows での従来の口の振る舞い（表示・終了の状態・オプション）を、ほかの環境の振る舞いと分けて述べる。
5. The steering（`tech.md`） shall `shiori4_host` の依存と CI の job の記述を、この spec の後の形に合わせる。
6. The 文書 shall `load` の dir の文字コード（CP_ACP）の記述を、移行の手引きの SHIORI/3.0 の列と食い違わない形にする。移行の手引きの「既定の OEM コードページ」の記述は CP_ACP に直し、ukadoc の字面と違う理由（要件 3.3）を添える（議題 5）。
7. If 文書の例が x86 の Windows でしかビルドできない, the 文書 shall その例が検査される環境（i686 の CI でビルドされるか、検査しないか）を明示する。
8. The shiori4_host の `Cargo.toml` shall `[package.metadata.docs.rs]` の `targets` に既定の対象と `i686-pc-windows-msvc` を挙げ、従来の DLL の口が docs.rs の i686 の Windows の頁に出るようにする（議題 12）。

### Requirement 12: 既存の API との互換と検証
**Objective:** As a 3 クレートの利用者, I want 従来の口が足されても今のコードがどの環境でもそのまま動くこと, so that 版を上げたときに壊れない（Constraints）

#### Acceptance Criteria
1. Where ビルドの対象が x86 の Windows である, the shiori4_host shall 既存の公開の型・関数・列挙の値（`LoadedGhost` の 3 つの値と `Legacy` のフィールド、`LoadError` の値、`EntryKind`）を変えず、従来の DLL の口を足すだけにする（ほかの環境は要件 1.3。議題 1・2）。
2. The shiori4_host shall x86 の Windows でだけ足す依存を、ほかの環境の依存の木に入れず、`cargo deny check` を通す。
3. The 3 クレート shall 版を上げず、同じ版のまま揃える。
4. The 3 クレート shall 既存の環境（ubuntu・windows x64・macos）と i686 の Windows の両方で、テストと clippy（`-D warnings`）を通す。

## 要件ディスカッションの決定
2026-10-25 の要件ディスカッションで、全議題を推奨案で確定した。

1. **初期化の入口の形**（要件 2.1・12.1）: `LoadedGhost::Legacy` のライブラリを受ける入口だけ。パスから直接読む入口は作らない。発見の順を飛ばす道を作らず、DLL を 2 回読まないため。
2. **`unload`／`request` が無いときの失敗**（要件 2.3・12.1）: `LoadError::NotShiori4`。必須の書き出しがそろわない従来の口は口として成り立たず、Display の「no allowed SHIORI entry point found」と意味が合う。値を足さないので、`LoadError` を網羅する match（試験用のホストの `load_error_name` を含む）が壊れない。
3. **load が失敗したときの unload**（要件 2.5）: 呼ばずにライブラリを解放する。v4 で load が失敗したときと同じで、ukadoc にも定めが無い。
4. **スレッド**（要件 2.7）: 作ったスレッドからだけ呼べる値（`!Send`）。`V3Runtime` と揃える。
5. **`load` の dir の文字コード**（要件 3.3・11.6）: CP_ACP。古い SHIORI は dir を `A` 系の API に渡し、それらが読むのは CP_ACP なので。ukadoc の字面（OEM）とは違うので、移行の手引きは CP_ACP に直して理由を添える。
6. **応答の端**（要件 4.6・5.2）: 長さが負の応答は失敗とし、メモリは解放する。文字列の口は末尾に続く NUL をすべて落とす（SHIORI/3.0 の応答は空行で終わり、末尾の NUL は詰め物）。
7. **ダイアログの抑止の範囲**（要件 7.1）: x86 の Windows で発見の口がライブラリを開く間だけ（v4・v3 の DLL を含む）。x64 の Windows などへ広げるのはこの spec の外。
8. **ライブラリのパスが相対のとき**（要件 7.3）: x86 の Windows の発見の口は開かずに `LoadError::InvalidPath` を返し、その doc を広げる。OS の探索の順に任せると別の DLL を拾うおそれがあるため。
9. **確保と解放の数**（要件 8.2）: テストの中だけ。DLL が入力を解放したかはホストからは数えられず、公開しても漏れの検査には足りない。
10. **試験用のホストの x86 の枝**（要件 10）: 要求は v3 互換モードと同じ全文（`Charset: UTF-8`）。従来の口だけを許可するオプションは `--entry legacy`。`request` の失敗は終了の状態 2 で、4 は x86 の Windows では出ない。UTF-8 でない応答は置換文字にして表示し、失敗にしない。
11. **x86 の配布の決まり**（要件の外）: この spec の外。`TargetArch` に値を足すのは破壊的な変更で、x86 のホストはファイルを自分で選んで渡せる。
12. **docs.rs**（要件 11.8）: `[package.metadata.docs.rs]` の `targets` に `i686-pc-windows-msvc` を足す。docs.rs は指定が無いと既定の対象の頁しか作らず、従来の DLL の口が見えないため。

設計フェーズに回した事項（research.md の「6. Recommendations for Design Phase」）: CP_ACP で表せない文字の知らせ方（要件 3.4）・要求の失敗の表し方（要件 4.4）・ダイアログの抑止の作り方（`LoadLibraryW` ＋ `Library::from_raw`）・Win32 の関数の宣言の仕方・テスト用の DLL の形・i686 のテストの DLL の作り方・`GlobalAlloc(GMEM_FIXED, 0)` と `WC_NO_BEST_FIT_CHARS` の調べもの。
