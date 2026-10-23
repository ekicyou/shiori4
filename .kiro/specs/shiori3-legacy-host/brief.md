# Brief: shiori3-legacy-host

## Problem
伺かのゴーストの SHIORI は、ほとんどが従来の SHIORI/3.0 の DLL である。HGLOBAL の `load`・`unload`・`request` を書き出し、x86 の Windows で動く。

ベースウェアの作者が shiori4_host を使っても、今は v4 の DLL しか読めない（v3 互換モードを読む口は abi-conventions の論点 16 で足す）。従来の DLL を読みたければ、HGLOBAL の受け渡しを自分で書くしかない。areka は自前の 32bit の helper で、それを書いている。SHIORI の作者（pasta）も、自分の HGLOBAL の入口を CI で試す手段を持っていない。

## Current State
- `shiori4_host` は、`libloading` で DLL を開き、`shiori_get_api` を探す。従来の SHIORI/3.0 の書き出しは扱わない。
- 3 つの spec が上流になる。
  - abi-conventions: ファイル名の決まり（論点 12）、両方を持つ DLL の見る順（論点 13）、`from_library`（論点 15）、v3 の口（論点 16）、読み込みの持ち場（論点 17）を決めた。
  - host-protocol: v4 の語を決める。
  - abi-textbook: 試験用のホストを作る。
- areka の helper は、従来の DLL の作法を実地で持っている（下の「areka の helper の作法」）。

## Desired Outcome
- x86 の Windows で、shiori4_host が従来の SHIORI/3.0 の DLL のホストとして使える（従来型の SHIORI/3.0 のエンドポイント）。
- areka の helper は、DLL に触る芯をこれに乗り換えられる。
- pasta は、自分の HGLOBAL の入口を CI で試せる。

## Approach
DLL に触る芯だけを shiori4_host に置く。プロセスの起こし方・IPC・待ち時間・強制終了・文字コードの方針は、ホストごとの作りとして、ホストに残す（areka の推奨）。

この口は `cfg(all(windows, target_arch = "x86"))` のときだけビルドに含める（開発者の方針）。ほかの環境では、ビルドにも API にも現れない。

## 決まっていること（2026-10-23 のヒアリングから）
1. **対象の環境**（開発者）: x86 の Windows だけ。ほかの環境ではビルドに含めない。
2. **loadu と load の選び方**（P・A が一致）:
   - `GetProcAddress` で `loadu` を先に見て、無ければ `load`。どちらも無ければ失敗とする。`unload` と `request` は必須。
   - 呼ぶのは片方を 1 回だけ。`loadu` が失敗しても `load` へは戻らない。
   - dir は、`loadu` には UTF-8、`load` には CP_ACP（`WideCharToMultiByte`）で渡す。
   - CP_ACP で表せない文字があるときは、`log` か警告で知らせてそのまま渡す（loadu を持たない DLL には、それ以上の手は無い）。
3. **HGLOBAL の持ち主**（SHIORI/3.0 の約束。P・A が一致）:
   - 入力（load・loadu・request の引数）は、ホストが `GlobalAlloc(GMEM_FIXED)` で確保し、ちょうど len バイトを書く（NUL 終端なし）。解放するのは DLL で、ホストは解放しない。
   - request の戻り値は、NULL なら失敗とする。それ以外は len バイトを写してから、ホストが `GlobalFree` する。NUL は探さず、長さで読む。
   - 試験のために、確保と解放を数えられるようにする（P）。
4. **unload**（A）: `unload()`（戻り値は見ない）→ `FreeLibrary` の順。
5. （欠番。BOOL の読み方は、下の「開発者が裁定した論点」の d）
6. **エラーのダイアログ**（A）: `LoadLibraryW` の間だけ `SetThreadErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)` で止める。DLL は絶対パスで読み、`SetDllDirectory` は使わない。
7. **load の dir の形**（A の勧め。abi-conventions 論点 6 に揃える）: 絶対パス・`\` の区切り・末尾に `\`。`dir + "file"` と書く古い SHIORI が壊れないようにする。
8. **1 つのモジュールに 1 インスタンス**（P）: 従来の ABI にはハンドルが無い。同じモジュールで load を 2 回呼ばないことを、文書と口の形で守る。2 体目は、別のファイルか別のプロセスで読む。
9. **発見の順**（開発者の裁定。abi-conventions 論点 13）: `shiori_get_api` → `loadu` → `load` の順に探し、最初に見つかった口で解決する。`shiori_get_api` を持つ DLL は、失敗しても従来の口へフォールバックしない。
   - 従来の口だけで読みたいとき（P の要望）は、abi-conventions 論点 13 の「許可する口の種類」で `loadu`・`load` だけを許可する。別の入口は作らない。
10. **ホストに残すもの**（A）: プロセスを起こすこと、IPC、待ち時間、強制終了、カレントディレクトリの決め方。

## 開発者が裁定した論点（2026-10-23）
- **方針: 中身の通信には関与しない**。このリポジトリが支えるのは、インターフェースの変換と取得までである。やり取りされる文字列の中身（ヘッダ・Charset・語彙）は解釈しない。shiori4_host が求めてよいのは、文字列の層で UTF-8 であることだけ。
- **a. 本文の受け渡し**:
  - 芯の口はバイト列にする（`&[u8]` を渡して `Vec<u8>` を受け取る）。
  - その上に `&str`／`String` の口を、任意の便利な層として重ねる。この層は UTF-8 であることを確かめるだけで、Charset を見ての変換はしない。Shift_JIS などの本文を扱うホストは、芯の口を使う。
  - pasta の「応答の Charset を見て解釈してほしい」は採らない。文字コードの方針はホストが持つ。
  - Win32 の文字コードの変換を使うのは、`load` に渡す dir（CP_ACP）だけになる。
- **b. 応答の末尾の NUL**: 芯の口は長さのまま返す。文字列の層で、末尾の NUL を落とす。
- **c. SHIORI/2.x の見分け**: この spec の外にする（areka の spec `shiori2-protocol` が持つ）。
- **d. load／loadu の戻り値（BOOL）**: 元の C ABI に準拠する。BOOL は 32bit の int で、0 以外を真と読む。
  - Rust の `bool` を返す DLL（直す前の pasta など）は、上位 24bit が不定なので誤読されうる。これは SHIORI の側の不具合で、SHIORI が直す（pasta は自分の spec で i32 に直す予定）。
  - areka と pasta が勧めた「下位 1 バイトで読む」守りは採らない。

## Scope
- **In**:
  - x86 の Windows だけで有効な、従来の SHIORI/3.0 の DLL を読む口（上の 1〜9）。
  - 確保と解放を数える試験。
  - i686 の Windows での CI（ビルドとテスト）。
  - abi-textbook の試験用のホストが、従来の DLL も読めるようにするためのつなぎ。
  - 文書（どの環境で使えるか、ホストに残すもの）。
- **Out**:
  - プロセスの起こし方、IPC、待ち時間、強制終了（ホストが持つ）。
  - SHIORI/2.x。
  - x86 の Windows 以外。
  - 本文の文字コードの方針（Charset による切り替えの決まり）。これはホストが持つ。便利な層の範囲は論点 a で決める。

## Boundary Candidates
- HGLOBAL の受け渡しと、DLL の書き出しの解決（芯）
- dir の変換（UTF-8 と CP_ACP）と正規化
- 文字列の便利な層（論点 a による）
- 試験（数える HGLOBAL と、i686 の CI）

## Out of Boundary
- areka の helper のプロセスと IPC
- ukadoc の Charset の切り替えの方針
- v4 と v3 互換モードの口（abi-conventions）

## Upstream / Downstream
- **Upstream**: abi-conventions（論点 6・12・13・15・17）。abi-textbook の試験用のホスト（つなぐ場合）
- **Downstream**: areka の helper（`shiori_proxy.rs` の大半を置き換える）、pasta の HGLOBAL の入口の CI、ほかのベースウェアの作者

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**:
  - abi-conventions: 同じ `shiori4_host` の読み込みの持ち場に足す。
  - abi-textbook: 試験用のホストと文書。

## Constraints
- 有効なのは `cfg(all(windows, target_arch = "x86"))` だけ。ほかの環境のビルドに、この口と、そのための依存（Win32 の API の crate など）を含めない。
- ANSI のエンコーダーを shiori4_host に抱え込まない。変換が要る所は Win32 の API に任せる。
- CI に i686 の Windows を足す。CI を変える PR なので、CI の完了を見届けてから統合する（workflow.md）。
