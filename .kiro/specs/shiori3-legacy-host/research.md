# ギャップ分析: shiori3-legacy-host

## Summary
- **Feature**: `shiori3-legacy-host`
- **Discovery Scope**: Extension（`shiori4_host` に x86 の Windows だけの口を足す。発見の口・dir の正規化・`LoadedGhost::Legacy` は上流の abi-conventions で揃っている。新しいのは Win32 の FFI・HGLOBAL の受け渡し・i686 の CI）
- **Key Findings**:
  - 継ぎ目はそろっている。発見の口（`shiori4_host/src/discover.rs` の `load_ghost_with`）が `LoadedGhost::Legacy { entry, library }` を返し、dir の検査と正規化（`lib.rs` の非公開の `normalize_dir`）は Windows で `\` の区切り・末尾に `\` 1 つの UTF-8 を作る。足すのは「Legacy のライブラリを受けて `loadu`／`load` を呼ぶ実行時の値」と、その下の HGLOBAL の受け渡しだけ。形の手本は `V3Runtime`（`shiori4_host/src/v3.rs`。`!Send`・`ManuallyDrop` の instance・Drop で unload → ライブラリの解放・UTF-8 でない応答は `SHIORI_E_INVALID_UTF8`）。
  - **ダイアログの抑止は今の `libloading` では足りない**。`libloading` 0.8.9 の `Library::new` は開く間だけ `SetThreadErrorMode(SEM_FAILCRITICALERRORS)` に**置き換える**（`SEM_NOOPENFILEERRORBOX` を落とす）。外から自分の設定で包んでも、開く瞬間は上書きされる。決定 6 を守るには、自分で `LoadLibraryW` を呼んで `libloading::Library::from_raw` で包む（`from_raw` は公開されている）必要がある（要件 7.1）。
  - **i686 でのテストは今そのままでは通らない**。`cargo test --target i686-pc-windows-msvc` を手元で走らせると、DLL を読むテストがすべて `LoadLibraryExW` のエラー 193（有効な Win32 アプリケーションではない）で落ちる。`test_dll::dll_path()` が `CARGO_BUILD_TARGET` を外し `--target` を付けずに testghost をビルドするので、ホストの arch（x64）の DLL ができるため。ビルド（`cargo build --workspace --all-targets --target i686-pc-windows-msvc`）は今も通る。
  - Win32 の関数は、外部の crate を足さずに `#[link(name = "kernel32")] extern "system"` で宣言できる（`lib.rs` の doc の例に前例がある）。要るのは `GlobalAlloc`・`GlobalFree`・`WideCharToMultiByte`・`SetThreadErrorMode`・`LoadLibraryW` で、すべて kernel32。`windows-sys` を足す道もある（要件 1.2・12.2・tech.md の「`shiori4_host` も `libloading` だけ」に触れる）。
  - 文書の書き換えは、申し送りの 2 か所（`migration.md` 51 行・試験用のホストの `run_legacy` と `testhost.md`）のほかに、**`doc/textbook/distribution.md` 214 行**にも「旧来の DLL を読む口は `shiori4_host` にまだ無い」がある（申し送りに無い）。`LoadedGhost::Legacy` の doc（「この spec では初期化しない」）と crate doc の例（`println!("従来の口 {entry:?} はこのクレートでは読まない")`）も直す対象になる。

## 1. Current State Investigation

### 関係する資産
| 場所 | 中身 | この spec との関係 |
|------|------|--------------------|
| `shiori4_host/src/discover.rs` | `EntryKind`（V4・V3・Loadu・Load）・`LoadedGhost`（V4・V3・Legacy）・`load_ghost`／`load_ghost_with`。dir の検査 → `libloading::Library::new` → v4 → v3 → `legacy()`（`loadu`・`load` の有無だけを `unsafe extern "C" fn()` で見る） | 初期化の入口の上流（要件 2.1・2.6）。ライブラリを開く所が要件 7（ダイアログ・絶対パス）の変更点。`Legacy` の doc を直す |
| `shiori4_host/src/lib.rs` の `normalize_dir` | 絶対パスでないか UTF-8 でないと `InvalidPath`。Windows で `/` → `\`、末尾に `\` 1 つ（`\\?\` は置き換えない） | そのまま要件 3.1 に使える。`load` 用の CP_ACP の変換はこの後に足す（要件 3.3） |
| `shiori4_host/src/lib.rs` の `LoadError`・`FromLibraryError` | `Library`・`NotShiori4`・`Unsupported`・`InvalidPath`・`LoadFailed`。`#[non_exhaustive]` ではない | 要件 2.3・2.5・3.1・12.1。値を足すと網羅の match が壊れる（試験用のホストの `load_error_name` も網羅している。Q2） |
| `shiori4_host/src/v3.rs` の `V3Runtime` | `load`／`from_library`／`from_static`・`request(&str) -> Result<String, ShioriStatus>`・`is_poisoned`・Drop で unload → `_lib` の解放 | 従来の DLL の実行時の値の手本（要件 2.7・5.3・6.1） |
| `shiori4_host/src/lib.rs` の crate doc（`//!`） | 「DLL の選び方と読み方」の節と例。`LoadedGhost::Legacy { entry, .. } => println!("従来の口 {entry:?} はこのクレートでは読まない")` | 要件 11.1。例は `no_run`（ビルドだけ）で、x86 だけの口を例に書くと他の環境でビルドできない（要件 11.7） |
| `shiori4_host/src/lib.rs` の doc の Win32 の例（`PostThreadMessageW` ほか） | 外部の crate を足さずに `#[link(name = "user32")]`・`#[link(name = "kernel32")] unsafe extern "system"` で宣言 | Win32 の FFI を依存なしに書く前例（要件 1.2・1.4） |
| `shiori4_host/src/dist.rs` | `TargetOs`・`TargetArch`（X64・Arm64 だけ）・`shiori_file_candidates`・`machine_matches` | x86 では `TargetArch::CURRENT` が `None`。i386 の PE も確かめられない（Q17。要件の外） |
| `shiori4_host/src/test_dll.rs` | `dll_path()`（`cargo build` で testghost を作る。`CARGO_BUILD_TARGET` を外し `--target` を付けない）・`lock()`・`set_mode`・テスト `dll_has_four_exports` | **Constraint**: i686 の `--target` で走らせると x64 の DLL ができて落ちる（下の Research Log） |
| `testdata/testghost/src/lib.rs` | `shiori_get_api`（モードで v4/v3）・`testghost_set_mode`・ダミーの `loadu`／`load`（`(*mut c_void, isize) -> i32`。呼ばれない前提）。`request`・`unload` は無い | 従来の口のテスト用の DLL の候補（要件 8.1）。今は `unload`・`request` が無いので、要件 2.3 では「読めない DLL」になる |
| `shiori4_host/examples/shiori4-testhost.rs` | `run_legacy(library)` が表示して `EXIT_LEGACY`（4）。`--entry` は `v3` だけ。`load_error_name` は `LoadError` を網羅 | 要件 10 の差し替え所。`--entry legacy`（Q14）の追加、終了の状態の表（Q15） |
| `doc/textbook/testhost.md` | 引数・表示・終了の状態の表（4 = 従来の口だけ）・出力の例・CI の例 | 要件 10.6・11.4 |
| `doc/textbook/migration.md` | 51 行「旧来の DLL を読む口は `shiori4_host` にまだ無い」。18・38 行に `load` の dir は「既定の OEM コードページ」 | 要件 11.2・11.6。OEM と CP_ACP の食い違い（Q5） |
| `doc/textbook/distribution.md` | 214 行「旧来の DLL を読む口は `shiori4_host` にまだ無い」 | 要件 11.3（申し送りに無かった所） |
| `.github/workflows/build.yml` | test（3 OS）・deny・package・release-scripts・textbook（3 OS。testghost と試験用のホストと C の例をビルドし、終了の状態で往復を確かめる） | 要件 9。i686 の job か matrix の行を足す。**Constraint**: CI を変える PR は CI を見届けてから統合（workflow.md） |
| `deny.toml` | ライセンスの許可（MIT・Apache-2.0・ISC）・`multiple-versions = "warn"` | 依存を足すなら通る必要がある（要件 12.2） |
| `.kiro/steering/tech.md` | 「`shiori4_host` も `libloading` だけ」・CI の job 5 種の記述 | 要件 11.5 |

### 守るべき規約・パターン
- `unsafe` は ABI に近い層に閉じ込め、上位には安全な型だけを見せる（tech.md）。HGLOBAL の生のハンドルは公開の API に出さない。
- `shiori4_host` は役割ごとの別ファイル（`mod`）に分け、テスト専用の補助は `#[cfg(test)] mod`（structure.md）。従来の口は `v3.rs`・`discover.rs` と同じく 1 ファイルにまとまる見込み。
- コメントとドキュメントは日本語。テストは同じファイルの `mod tests`。DLL を使うテストは `test_dll::lock()` を取る。
- 芯は中身を解釈しない（tech.md の Key Technical Decisions・brief の方針）。
- 版は上げない（`/release` が上げる）。

## 2. Requirements Feasibility Analysis

### 要件と資産の対応
| 要件 | 必要なもの | 今あるもの | ギャップ |
|------|-----------|-----------|---------|
| 1 有効な環境 | `cfg(all(windows, target_arch = "x86"))` の mod と公開・x86 だけの依存 | cfg で分けた前例（`#[cfg(test)] mod test_dll`・doc の `#[cfg(windows)]`） | **Missing**: mod と cfg。**Constraint**: 依存を足すなら `[target.'cfg(all(windows, target_arch = "x86"))'.dependencies]` で他の環境の木に入れない。足さずに kernel32 を直接宣言する道もある |
| 2 解決と初期化 | Legacy のライブラリを受け、`loadu`／`load`・`unload`・`request` の書き出しを解決し、片方を 1 回だけ呼ぶ | `legacy()` が `loadu`・`load` の有無を見る。`EntryKind` が解決した口を持つ | **Missing**: 実行時の値と入口、`unload`・`request` の必須の検査、BOOL（i32）の読み方。**Unknown**: 入口の形（Q1）、必須の書き出しが無いときの値（Q2）、load 失敗のときの unload（Q3） |
| 3 dir | 正規化した dir・UTF-8 か CP_ACP のバイト列・表せない文字の検出と知らせ | `normalize_dir`（UTF-8 の `String`） | **Missing**: CP_ACP への変換（`MultiByteToWideChar` は不要。`OsStr` → UTF-16 → `WideCharToMultiByte(CP_ACP)`）と検出。**Research Needed**: 検出には `lpUsedDefaultChar` と `WC_NO_BEST_FIT_CHARS` の併用が要る（best-fit で黙って似た文字に写るのを防ぐ）。**Unknown**: 知らせ方（Q6）、CP_ACP か OEM か（Q5） |
| 4 request の芯 | `GlobalAlloc(GMEM_FIXED)` に書く・`request(h, &mut len)`・NULL の検査・長さで写す・`GlobalFree` | 無い | **Missing**: すべて。**Constraint**: 呼び出し規約は ukadoc のとおり `__cdecl`（x86 の Rust の `extern "C"` と同じ）。`long` は Windows で 32bit（`i32`。testghost のダミーは `isize` で、x86 では幅は同じ）。**Unknown**: 失敗の表し方（Q7）、負の長さ（Q8） |
| 5 文字列の口 | 末尾の NUL を落とす・UTF-8 の検査 | `V3Runtime::request` の UTF-8 の検査と `SHIORI_E_INVALID_UTF8` | **Missing**: NUL を落とす所（1 行）。**Unknown**: 落とす数（Q9） |
| 6 寿命 | Drop で `unload()` → ライブラリの解放・2 回 load できない形 | `V3Runtime` の Drop の順・`ManuallyDrop` | **Missing**: 実装。入口がライブラリを値で受け取れば、2 回呼ぶ手段は型で消える（要件 6.2）。同じファイルを 2 回 `LoadLibrary` すると同じモジュールになる点は文書で守る（要件 6.3） |
| 7 ダイアログとパス | 開く間だけ `SEM_FAILCRITICALERRORS \| SEM_NOOPENFILEERRORBOX`・`SetDllDirectory` を使わない・絶対パス | `libloading::Library::new`（`SEM_FAILCRITICALERRORS` だけ・相対パスは OS の探索の順） | **Missing**: 自分で `LoadLibraryW` を呼んで `Library::from_raw` で包む道。**Constraint**: 開く所は発見の口の中なので、x86 の Windows では v4・v3 の DLL の開き方も変わる（Q10）。相対パスを断ると x86 の既存の利用者の振る舞いが変わる（Q11） |
| 8 数える試験 | 従来の 4 つの口を本当に持つテスト用の DLL と、確保・解放の数 | testghost（ダミーの `loadu`・`load`） | **Missing**: 本物の `loadu`・`load`・`request`・`unload`（`GlobalFree`・`GlobalAlloc` を呼び、数を返す書き出し）と、ホスト側の数（`#[cfg(test)]` の数え口）。**Unknown**: 数を公開するか（Q12） |
| 9 i686 の CI | i686 の Windows で test・clippy・往復 | 3 OS の test と textbook | **Missing**: job。**Constraint**: `test_dll::dll_path()` が i686 の DLL を作らない（下の Research Log）。textbook の「Round trips」も testghost を x64 で作る |
| 10 試験用のホスト | x86 で Legacy を往復・`--entry legacy`・終了の状態 | `run_legacy`（継ぎ目）・`v3_request`・`v3_status`・`finish` | **Missing**: x86 の枝。`v3_request` と `v3_status` をそのまま使える。**Unknown**: Q13〜Q16 |
| 11 文書 | crate doc・`migration.md`・`distribution.md`・`testhost.md`・tech.md | 各所に「まだ無い」の文 | **Missing**: 書き換え。**Unknown**: docs.rs の頁（Q18）。x86 だけの例の検査の仕方（要件 11.7） |
| 12 互換と検証 | 既存の型を変えない・deny・4 環境のテスト | 3 OS のテスト・deny | **Constraint**: Q1・Q2 の決め方しだいで `LoadedGhost`・`LoadError` に触れる |

### 複雑さの兆し
- 外部とのつなぎ（Win32 の FFI・32bit のメモリの受け渡し）が中心。ロジックは小さい（解決・1 回だけ呼ぶ・長さで写す）。
- 危ないのは、他の arch の CI（i686）と、ダイアログの抑止の細部（`libloading` の上書き）。

## 3. Implementation Approach Options

### Option A: 既存のファイルを伸ばす
- `discover.rs` に x86 の cfg で、従来の口の実行時の値と HGLOBAL の受け渡しを足す。
- ✅ 新しいファイルが無い。発見の口と同じ所で `Legacy` を扱える
- ❌ `discover.rs`（発見の順だけの純粋な役割）に unsafe の FFI と cfg が混ざる。structure.md の「役割ごとの別ファイル」に反する

### Option B: 新しいファイル `legacy.rs`（x86 の Windows だけの mod）
- `#[cfg(all(windows, target_arch = "x86"))] mod legacy;` に、kernel32 の宣言・HGLOBAL の受け渡し（入力の確保・応答の写しと解放）・dir の CP_ACP の変換・実行時の値（`V3Runtime` の形を手本に `!Send`・Drop で unload → 解放）・バイト列と文字列の `request` を置く。
- `discover.rs` は、x86 の Windows でだけライブラリの開き方を「ダイアログを止めて `LoadLibraryW` → `Library::from_raw`」に替える（要件 7）。`LoadedGhost::Legacy` の形は変えない。
- 試験は testghost に本物の従来の口を足す（`unload`・`request` も足すので、要件 2.3 の「無いと読めない」の試験には別のモードか別の cdylib が要る）か、従来の口だけを持つ cdylib を `testdata/` に新しく作る。
- ✅ v3.rs・discover.rs と同じ並び。cfg が 1 か所に閉じ、他の環境のビルドに一切現れない
- ✅ 依存を足さずに書ける（kernel32 の宣言）
- ❌ 発見の口の開き方の変更だけは discover.rs に cfg の枝が残る

### Option C: 混合（芯は B、開き方の共通化は後で）
- B と同じく `legacy.rs` を足し、ダイアログの抑止は x86 の Windows で発見の口だけに入れる。x64 の Windows や `GhostRuntime::load`・`V3Runtime::load` への広げ（Q10）は、要件ディスカッションの決定しだいで別の段に回す。
- ✅ 決定 1（x86 だけ）を守りつつ、ダイアログの抑止を他の環境に広げる余地を残す
- ❌ 開き方が環境と入口で分かれる期間ができる

### テスト用の DLL の選択肢
| 選択肢 | 中身 | 長所 | 短所 |
|--------|------|------|------|
| testghost を伸ばす | ダミーの `loadu`・`load` を本物にし、`request`・`unload` と数の書き出しを足す（kernel32 の宣言は `cfg(windows)`） | 1 本で済む。`dll_path()`・`lock()` をそのまま使える | `shiori_get_api` を持つので、従来の口の試験は許可する口で `Loadu`・`Load` に絞って読む。`request`・`unload` が無い DLL の試験にモードが要る。他の OS のビルドにダミーを残す |
| 従来の口だけの cdylib を足す | `testdata/legacyghost/`。`loadu`・`load`・`request`・`unload` と数の書き出しだけ（依存なし） | `shiori_get_api` が無いので発見の口の既定の順で Legacy に解決する（試験用のホストの CI にも素直） | ビルドの補助（`dll_path` の相当）がもう 1 つ要る |

## 4. Research Log

### i686 での `cargo test`
- **Context**: 要件 9.1 を今の仕組みで満たせるか。
- **Sources Consulted**: 手元で `cargo build --workspace --all-targets --target i686-pc-windows-msvc`（成功）と `cargo test -p shiori4_host --target i686-pc-windows-msvc --lib`（DLL を読むテストが全滅）。i686 の target は手元に入っている。
- **Findings**: `test_dll::dll_path()` は外の `--target` を引き継がず、ホストの arch の testghost を作る。i686 のテストの実行ファイルから x64 の DLL を開いて `LoadLibraryExW` のエラー 193 になる。DLL を読まないテストは通る。
- **Implications**: (a) CI で i686 の toolchain をホストとして入れる（`stable-i686-pc-windows-msvc`。`--target` を付けない）、または (b) `dll_path()` がテストの実行ファイルと同じ target で testghost を作る（target の三つ組を cfg から作って `--target` を渡し、出力の場所を `target/testghost/<三つ組>/debug` にする）。(a) はコードを変えないが、手元で x64 の toolchain から `--target` で試す人には落ちる。textbook の job の testghost のビルドも同じ扱いが要る。

### `libloading` とエラーのダイアログ
- **Context**: 要件 7.1（決定 6）。
- **Sources Consulted**: `libloading-0.8.9/src/os/windows/mod.rs`（`Library::new` → `load_with_flags(filename, 0)` → `ErrorModeGuard::new()` → `LoadLibraryExW`）。
- **Findings**: `ErrorModeGuard` は `SetThreadErrorMode(SEM_FAILCRITICALERRORS, &prev)` で値を**置き換え**、Drop で戻す。`SEM_NOOPENFILEERRORBOX` は開く間に落ちる。`Library::from_raw(HMODULE)`・`into_raw` は公開されている。`libloading` は Windows で `windows-link` に依存する（`windows-link::link!` で kernel32 を宣言）。
- **Implications**: 自分で `SetThreadErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)` → `LoadLibraryW`（絶対パス）→ 戻す → `Library::from_raw` で包む。`Library::new` を外から包むだけでは足りない。

### SHIORI/3.0 の DLL 共通仕様（ukadoc）
- **Context**: HGLOBAL の決まりと文字コードの確認。
- **Sources Consulted**: ukadoc「DLL共通仕様」（spec_dll。伺かの技術文書の検索で取得）、pasta の内部設計の「SHIORI 層」（`loadu`・`load` の入口）。
- **Findings**: 入力はホストが `GlobalAlloc(GMEM_FIXED)` で確保して書き、モジュールが `GlobalFree` する。応答はモジュールが `GMEM_FIXED` で確保し、ホストが使った後に `GlobalFree` する。NUL 終端は保証されず `len` で読む（応答は「len より 1 バイト多く確保してゼロ終端することが望ましい」）。関数は `__cdecl`。`loadu` は UTF-8 で SSP 2.6.92 から。`load` は「デフォルトの OEM codepage（日本語環境では CP932）」。`loadu` と `load` の両方を持つ DLL は「loadu で初期化済みのときに load も呼ばれたら無視するのが望ましい」（ベースウェアが両方を呼ぶ場合への備え。この spec は片方だけを呼ぶ）。`unload` の戻り値は使われていない。load が失敗したときに unload を呼ぶかは書いていない。
- **Implications**: 要件 3.3 の CP_ACP は ukadoc の「OEM」と字面が食い違う（Q5）。load の失敗のときの unload は仕様に無く、areka の helper の今の作法を確かめる必要がある（Q3）。

## 5. Implementation Complexity & Risk
- **Effort**: M（3〜7 日）。芯のロジックは小さいが、Win32 の FFI・テスト用の DLL・i686 の CI（test と textbook の両方）・試験用のホストの枝・文書の 5 か所が揃って要る。
- **Risk**: Medium。手元で確かめきれない i686 の CI と、ダイアログの抑止の細部（`libloading` の上書き・`from_raw` の道）が新しい。ロジックそのものと発見の口の継ぎ目は既にあり、Q1・Q2 の決め方で既存の API に触れないで済む。

## 6. Recommendations for Design Phase
- **推す道**: Option B（`legacy.rs` を x86 の Windows だけの mod にする）。依存は足さず kernel32 を直接宣言する（tech.md の「`libloading` だけ」を保てる）。発見の口は x86 の Windows でだけ、ライブラリの開き方を自前の `LoadLibraryW` ＋ `Library::from_raw` に替える。
- **設計で決めること**: 初期化の入口の形と名前（Q1）・失敗の値（Q2・Q7）・CP_ACP で表せない文字の知らせ方（Q6）・テスト用の DLL を testghost に足すか新しく作るか・i686 のテストの DLL の作り方（toolchain かコードか）。
- **持ち越す調べもの（Research Needed）**:
  - `WideCharToMultiByte(CP_ACP)` で表せない文字を確実に見分ける旗（`WC_NO_BEST_FIT_CHARS` と `lpUsedDefaultChar`）の振る舞い。
  - `GlobalAlloc(GMEM_FIXED, 0)`（空の要求・空の dir）の返り値の扱い。
  - areka の helper（`shiori_proxy.rs`）の今の作法: load の失敗のときに unload を呼ぶか・スレッドの使い方・CP_ACP の知らせ方（Q3・Q4・Q6）。
  - `dtolnay/rust-toolchain` で i686 の toolchain をホストとして入れる書き方と、`Swatinem/rust-cache` の鍵の分け方。
  - docs.rs で i686 の Windows の頁を出す設定（Q18）。

---

# 設計フェーズの調査と決定（2026-10-25）

ギャップ分析（上）の「6. Recommendations for Design Phase」と、要件ディスカッションで設計に回した事項を調べて決めた記録。発見の種類は **Extension**（light discovery）。使ったスキルは kiro-spec-design（規則 design-principles・design-discovery-light・design-synthesis・design-review-gate）と kiro-validate-design（design-review）だけで、ほかの分野のスキルは当てはまらない（UI・アクセシビリティの面が無い）。

## Summary（設計フェーズ）
- **Discovery Scope**: Extension。継ぎ目（発見の口・`LoadedGhost::Legacy`・`normalize_dir`）は上流で揃っている。新しいのは x86 の Windows だけの FFI と HGLOBAL の受け渡し、テスト用の DLL、i686 の CI。
- **Key Findings**:
  - `libloading::Error` は外から作れない（`WindowsError` のフィールドが `pub(crate)`）。自前の `LoadLibraryW` が失敗したとき、`LoadError` に値を足さずに `LoadError::Library` を返すには、libloading にもう一度開かせてエラーを得るしかない。
  - `WideCharToMultiByte(CP_ACP, 0, ..)` は best-fit で黙って似た文字に写す（手元の CP932 で `é` → `e`、`lpUsedDefaultChar` は 0）。`WC_NO_BEST_FIT_CHARS` を付けると `?` に置き換え、`lpUsedDefaultChar` が 1 になる。ACP が 65001（UTF-8）のプロセスではこの旗と `lpUsedDefaultChar` が使えない（`ERROR_INVALID_PARAMETER`）ので、分岐が要る。
  - cargo 1.99 は `--target i686-pc-windows-msvc` でも doctest を走らせる（手元で 22 件が通った）。x86 だけの例は i686 の CI の `cargo test` で検査される。

## Research Log（設計フェーズ）

### `libloading` のエラーの型と `from_raw`
- **Context**: 要件 7.1（ダイアログを両方止める）と 12.1（`LoadError` を変えない）を両立できるか。
- **Sources Consulted**: `libloading-0.8.9/src/error.rs`・`src/os/windows/mod.rs`・`src/safe.rs`。0.9.0 も同じ所を確かめた。
- **Findings**:
  - `pub struct WindowsError(pub(crate) std::io::Error)`（0.9.0 は `pub(crate) i32`）。`Error::LoadLibraryExW { source: WindowsError }` は外から作れない。`From` の実装も無い。
  - `ErrorModeGuard::new()` は `SetThreadErrorMode(SEM_FAILCRITICALERRORS, &prev)` で置き換え、Drop で `prev` に戻す（0.9.0 も同じ）。外で両方の旗を立てても、`LoadLibraryExW` の間は `SEM_FAILCRITICALERRORS` だけになる。
  - `os::windows::Library::from_raw(handle: isize)` は公開。`impl From<os::windows::Library> for Library` がある。
  - MS の文書では `SEM_NOOPENFILEERRORBOX` は `OpenFile` 関数のダイアログを止める旗で、`LoadLibrary` が出す窓（正しくないイメージ・依存の DLL が無い）は `SEM_FAILCRITICALERRORS` が止める。
- **Implications**: 自前で `SetThreadErrorMode(両方)` → `LoadLibraryW` → 戻す → `from_raw` で包む。失敗したときだけ、同じ旗の中で `libloading::Library::new` にもう一度開かせ、そのエラーを `LoadError::Library` にする（設計の `open_library`）。代わりに「libloading を両方の旗で包むだけ」でも窓の実害は同じだが、要件 7.1 の字面（開く間に両方を止める）から外れる（設計の Open Question 3）。

### `GlobalAlloc(GMEM_FIXED, 0)`
- **Context**: 空の要求（0 バイト）を渡すとき、確保が NULL を返して失敗と取り違えないか。
- **Sources Consulted**: MS の `GlobalAlloc` の文書（0 バイトについては `GMEM_MOVEABLE` のときの「破棄済みのハンドル」しか書いていない）。手元の Windows 11 で PowerShell から P/Invoke して確かめた。
- **Findings**: `GlobalAlloc(0, 0)` は NULL でないポインタを返し、`GlobalSize` は 1。`GMEM_FIXED` で 0 バイトのときの振る舞いは文書に保証が無い。
- **Implications**: 確保の大きさは `max(len, 1)` にし、長さの引数には本当の長さ（0）を渡す。文書に頼らずに済み、実際の振る舞い（1 バイト）とも同じになる。dir は正規化で必ず 1 バイト以上あるので、0 になり得るのは要求だけ。

### `WideCharToMultiByte` と best-fit
- **Context**: 要件 3.3・3.4（CP_ACP のバイト列・表せない文字を知らせる）。
- **Sources Consulted**: MS の `WideCharToMultiByte` の文書。手元（ACP 932）で P/Invoke して確かめた。
- **Findings**（`cchWideChar` に長さを渡し NUL を出さない形で）:

  | 入力 | 旗 | 出力 | `lpUsedDefaultChar` |
  |------|----|------|---------------------|
  | `C:\é\` | 0 | `43 3A 5C 65 5C`（`e` に写る） | 0 |
  | `C:\é\` | `WC_NO_BEST_FIT_CHARS` | `43 3A 5C 3F 5C`（`?`） | 1 |
  | `C:\☃\` | 同 | `?` | 1 |
  | `C:\😀\`（サロゲート対） | 同 | `??` | 1 |
  | `C:\①\` | 同 | `87 40`（CP932 にある） | 0 |

  - 文書: CP_UTF8（65001）では `dwFlags` は 0 か `WC_ERR_INVALID_CHARS` だけで、`lpDefaultChar`・`lpUsedDefaultChar` は NULL でなければならない。ACP は、システムの「ベータ: ワールドワイド言語サポートで Unicode UTF-8 を使用」か、ホストの exe のマニフェストの `activeCodePage` が UTF-8 のとき 65001 になる。
- **Implications**: `GetACP() == 65001` なら正規化した dir の UTF-8 をそのまま渡す（表せない文字は無い）。それ以外は `WC_NO_BEST_FIT_CHARS` と `lpUsedDefaultChar` で変換と検出を 1 度に行う。best-fit を許すと、別の（在るかもしれない）ディレクトリの名前に黙って写るので許さない。

### areka の helper（`shiori_proxy.rs`）の今の作法
- **Context**: ギャップ分析の持ち越し（Q3・Q4・Q6）。areka が乗り換えるときの差。
- **Sources Consulted**: `areka/crates/shiori-host32-helper/src/shiori_proxy.rs`（手元の写し）。
- **Findings**:
  - `LoadLibraryW` を `SetThreadErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)` で包む（`load_library_quiet`）。依存は `windows` crate。
  - `loadu` → `load` の順に引き、続けて `unload`・`request` を必須で引く。確立の途中の失敗は `unload` を呼ばずに `FreeLibrary`（議題 3 と同じ）。BOOL は 0 以外を成功（`unload` の戻りは `u8` と宣言し、捨てる）。
  - `load` の dir は `WideCharToMultiByte(CP_ACP, 0, ..)`（best-fit を許す）で変換し、`MultiByteToWideChar` で戻して元と比べて表せない字を見つける。見つけたら `eprintln!` で警告を 1 行出す。
  - 初期化の結果の型は helper の自前の `ProxyError`。
- **Implications**: 振る舞いの芯（選び方・必須の書き出し・失敗のときの後始末・BOOL）は同じ。違うのは (1) best-fit を許すか（この設計は許さない）、(2) 知らせ方（helper は stderr。ライブラリの shiori4_host は stderr に書かず、ホストが読める値で返す）。乗り換えた helper は、返った値を見て自分で 1 行を出せばよい。

### i686 の doctest
- **Context**: 要件 11.7（x86 だけの例をどこで検査するか）。
- **Sources Consulted**: 手元で `cargo test -p shiori4_host --doc --target i686-pc-windows-msvc`（cargo 1.99.1）。
- **Findings**: 22 件の doctest がビルドされ、走って通った（cross の doctest が既定で走る）。
- **Implications**: `legacy.rs` の doc の例は、i686 の CI の `cargo test --all --target i686-pc-windows-msvc` でビルド（`no_run`）と検査がされる。ほかの環境ではモジュールごと無いので検査されない。crate doc の例は全環境でビルドされるので、x86 だけの型を書かない。

### CI と docs.rs
- **Context**: 要件 9・11.8。
- **Sources Consulted**: `dtolnay/rust-toolchain` の入力（`targets`・`components`）、`Swatinem/rust-cache`（既定で job の id を鍵に含む）、docs.rs の `[package.metadata.docs.rs]`（`targets` を書くと、その中だけを作る。`default-target` が無ければ最初の要素が既定）。
- **Findings**: x64 の runner に x64 の toolchain と `i686-pc-windows-msvc` の target を入れて `--target` で作るのが、手元の開発（x64 の toolchain）と同じ形になる。i686 の toolchain をホストとして入れる道は、rustup の「ホストでない toolchain」の扱いに左右される。rust-cache は job ごとに鍵が分かれるので `key` は要らない。
- **Implications**: CI は `--target i686-pc-windows-msvc` で回す。そのために `test_dll::dll_path()` が x86 のテストでは testghost を同じ target で作る（設計の Open Question 4）。docs.rs は `targets = ["x86_64-unknown-linux-gnu", "i686-pc-windows-msvc"]`。

## Architecture Pattern Evaluation（設計フェーズ）
| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| B: `legacy.rs`（x86 の Windows だけの mod） | FFI・HGLOBAL・dir の変換・実行時の値・開く口を 1 ファイルに置く。discover.rs は x86 の Windows でだけ開く所を差し替える | cfg が 1 ファイルと 1 か所の呼び出しに閉じる。v3.rs と同じ並び。依存を足さない | discover.rs に cfg の枝が 1 つ残る | 採用（ギャップ分析の推す道） |
| A: discover.rs を伸ばす | 発見の口のファイルに FFI を足す | ファイルが増えない | 発見の順だけの役割に unsafe の FFI が混ざる（structure.md に反する） | 不採用 |
| `windows-sys` を足す | 宣言を crate から取る | 宣言の誤りが減る | tech.md の「`libloading` だけ」が崩れる。依存の木の管理が増える。使う関数は 6 つだけ | 不採用。kernel32 を直接宣言する（lib.rs の doc の例に前例） |

## Design Decisions（設計フェーズ）

### Decision: 初期化の入口の形
- **Context**: 要件 2.1・6.2・12.1（議題 1）。
- **Alternatives Considered**: (1) `LoadedGhost<H>` を丸ごと受ける、(2) `entry`・`library`・dir を受ける、(3) パスから読む入口も足す。
- **Selected Approach**: (2) `LegacyRuntime::from_library(entry, library, ghost_dir)`。`entry` が `Loadu`・`Load` でなければ `NotShiori4`。
- **Rationale**: `LoadedGhost::Legacy` は dir を持たないので dir はもう一度要る。`LoadedGhost<H>` は `H` を持つ型で、従来の口には `Host` が要らない（host-protocol の Adjacent）。ライブラリを値で受けるので 2 回初期化する手段が型で消える。(3) は議題 1 で退けた。
- **Trade-offs**: 利用者が自分で `Library::new` したライブラリも渡せる（そのときダイアログの抑止と絶対パスの検査は効かない）。doc に書く。

### Decision: 要求の失敗の型
- **Context**: 要件 4.3・4.4・4.6・5.3・10.4。
- **Alternatives Considered**: (1) `ShioriStatus` の ABI のエラーの値を使い回す、(2) 新しい値を `SHIORI_E_*` に足す、(3) 専用の列挙を足す。
- **Selected Approach**: (3) `LegacyRequestError { AllocFailed, NullResponse, NegativeLength(i32), InvalidUtf8 }`。
- **Rationale**: NULL の応答・負の長さに合う `SHIORI_E_*` は無い。足すと ABI の規約（shiori4_abi と C ヘッダ）が変わり、この spec の外になる。要件 10.4 も「`abi error:` の代わりの行」を求めている。
- **Trade-offs**: `V3Runtime::request` と型が揃わない（従来の DLL は ABI のエラーを返さないので、揃える意味が薄い）。

### Decision: CP_ACP で表せない文字の知らせ方
- **Context**: 要件 3.4（決定 2）。
- **Alternatives Considered**: (1) `log` crate、(2) コールバックを入口の引数にする、(3) `eprintln!`、(4) 実行時の値に旗を持たせて読ませる。
- **Selected Approach**: (4) `LegacyRuntime::dir_lossy() -> bool`。
- **Rationale**: 依存を足さない（tech.md）。ライブラリが勝手に stderr に書かない。ホストが自分の log に 1 行で流せる。
- **Trade-offs**: `load` が失敗したときは値が返らないので旗も読めない（その失敗が表せない文字のせいかは、ホストが dir を見て判断する）。

### Decision: テスト用の DLL
- **Context**: 要件 8.1〜8.4。
- **Selected Approach**: 依存なしの cdylib `testdata/legacyghost/` を新しく作る（`loadu`・`load`・`request`・`unload` と、テストのための書き出し）。testghost は変えず、「`unload`・`request` が無い DLL」の試験にそのまま使う（ダミーの `loadu`・`load` だけを持つ）。「`loadu` が無い DLL」は、発見の口の「許可しない口は無いものとして扱う」で `EntryKind::Load` だけを許可して作る。
- **Rationale**: `shiori_get_api` を持たないので、既定の発見の順で `Legacy` に解決する（試験用のホストの CI に素直）。1 本で済み、cargo の feature で作り分けない。

### Decision: i686 のテストの DLL の作り方
- **Selected Approach**: `test_dll` が、x86 の Windows のテストでは `--target i686-pc-windows-msvc` を付けて testghost・legacyghost を作り、`target/<名前>/i686-pc-windows-msvc/debug/` から読む。ほかの環境は今のまま。
- **Rationale**: 手元の x64 の toolchain から `cargo test --target i686-pc-windows-msvc` で試せる。CI も同じ形で、toolchain の入れ方に頼らない。

## Synthesis（設計フェーズ）
- **Generalization**: `load`・`loadu`・`request` の入力はどれも「GMEM_FIXED にちょうど len バイトを書き、所有を DLL に渡す」同じ操作なので、1 つの内部の関数（`global_alloc_copy`）にまとめる。応答の受け取り（NULL・負の長さ・写す・解放）も 1 か所。`loadu` と `load` の違いは dir のバイト列だけ（UTF-8 か CP_ACP か）なので、呼ぶ関数の型は 1 つ（`LoadFn`）にする。
- **Build vs. Adopt**: 動的読み込みは libloading を使い続ける（`from_raw` で包むだけ）。Win32 の宣言は `windows-sys`・`windows` を採らず kernel32 を直接宣言する（6 関数・tech.md）。ANSI の変換器は抱えず `WideCharToMultiByte` に任せる（要件 1.4）。
- **Simplification**: 毒（poisoned）の状態は持たない（従来の DLL はパニックを ABI のエラーで知らせない。クラッシュはプロセスごと落ちる）。確保と解放の数は公開しない（`#[cfg(test)]` だけ）。`LegacyRuntime` に `entry()` の読み出しは足さない（呼び出し側が `LoadedGhost::Legacy` で既に知っている）。CI の新しい job は 1 つにまとめ（test・clippy・往復）、C の例の x86 のビルドは足さない（要件に無い）。

## Risks & Mitigations（設計フェーズ）
- 失敗した DLL をもう一度開くと、`DllMain` が失敗の原因なら初期化のコードが 2 回動く — 失敗のときだけで、成功の道には無い。doc に書く。Open Question 3 で代わりの道を議論する。
- `LoadLibraryW` は絶対パスでも、依存の DLL を DLL のディレクトリからは探さない（アプリのディレクトリと標準の順）— 今の libloading（フラグ 0）と同じ振る舞いで、この spec では変えない（`SetDllDirectory` も使わない。決定 6）。doc に書く。
- x86 の Windows で相対のライブラリのパスが `InvalidPath` になる — 既存の x86 の利用者の振る舞いが変わる（議題 8 で受け入れた）。discover.rs の既存のテスト `os_library_is_not_shiori4`（`kernel32.dll` を相対で渡す）を絶対パスに直す。
- i686 の CI を手元で確かめきれない — 手元にも i686 の target があり、`cargo test --target i686-pc-windows-msvc` で同じものを走らせられる。CI を変える PR なので見届けてから統合する（workflow.md・要件 9.4）。

## References（設計フェーズ）
- [GlobalAlloc](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-globalalloc) — GMEM_FIXED とハンドル＝ポインタ
- [WideCharToMultiByte](https://learn.microsoft.com/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte) — `WC_NO_BEST_FIT_CHARS`・`lpUsedDefaultChar`・CP_UTF8 の制約
- [SetThreadErrorMode](https://learn.microsoft.com/windows/win32/api/errhandlingapi/nf-errhandlingapi-setthreaderrormode) — `SEM_FAILCRITICALERRORS`・`SEM_NOOPENFILEERRORBOX`
- [Use the Windows UTF-8 code page](https://learn.microsoft.com/windows/apps/design/globalizing/use-utf8-code-page) — マニフェストの `activeCodePage` で ACP が 65001 になる
- ukadoc「DLL 共通仕様」（spec_dll）— HGLOBAL の持ち主・`__cdecl`・`loadu`
- docs.rs の [Metadata](https://docs.rs/about/metadata) — `targets`・`default-target`
