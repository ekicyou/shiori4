# Research & Design Decisions — abi-conventions（ギャップ分析）

## Summary
- **Feature**: `abi-conventions`
- **Discovery Scope**: Extension（既存の 3 クレートの規約の本文と振る舞いを締め、`shiori4_host` に読み込みの口を足す）
- **対象のコード**: `shiori4_abi/include/shiori4_abi.h`（194 行）、`shiori4_abi/src/lib.rs`（961 行）、`shiori4/src/lib.rs`（511 行）、`shiori4_host/src/lib.rs`（722 行）。外部の依存は `shiori4_host` の `libloading` だけ。
- **Key Findings**:
  - 規約の本文（要件 1〜2・4〜10）は、ほぼ注釈の書き換えで済む。ただし C ヘッダと Rust の doc はすでに 1 か所ずれている（Rust にだけ規約 6「catch_unwind」がある）。
  - 振る舞いの変更で重いのは 2 つ。**Poster の無効化**（要件 4）は `ShioriHostPoster` が `Copy` で生ポインタを持つ今の形では実現できず、`shiori4_abi` の load／unload の入口に手が入る。**-7 の検出**（要件 3）は `Bridge` が今は呼び出しのスレッドも期間も知らないので、`GhostRuntime` との間に状態を足す必要がある。
  - 新しい口（要件 11〜14）は `shiori4_host` への純粋な追加で、既存の `with_api(.., lib: Option<Library>)` と `ShioriApi::load_v3` がそのまま土台になる。機械の種類の確認は std だけで書ける（依存を足さない）。

## 1. 現状の調査

### 1.1 規約の置き場と、ずれ
| 場所 | 中身 | ずれ・古い記述 |
|---|---|---|
| `shiori4_abi.h` 冒頭（1〜42 行） | 版・ABI 規約・戻り値・スレッド規約 1〜5・post の完了通知 | 規約 2 が「処理中に呼んでよい」だけで、スレッドの限定が無い。規約 5 に「post を呼ぶ自分のスレッドをすべて止めてから返る」（手段の指定）。log・release のスレッド、notify の後回し、load の失敗、dir の形、カレントディレクトリ、v4 の Charset、別プロセス、複数のインスタンスは記述なし |
| `shiori4_abi.h` の構造体の注釈 | `ShioriHostApi.request`「SHIORI のスレッドから、load / request / notify の処理中に限り呼べる」、`post`「任意のスレッド」、`log` はスレッドの記述なし、`ShioriApi.load` の dir は「UTF-8 のゴーストディレクトリ」だけ | 要件 2・5・8 に合わせて直す |
| `shiori4_abi/src/lib.rs` 冒頭 `//!`（1〜48 行） | C ヘッダとほぼ同文 ＋ **規約 6（catch_unwind）** | 規約 6 は C ヘッダに無い（要件 1.4・未決 4） |
| `shiori4_abi/src/lib.rs` の `Shiori` trait（622 行付近） | 「Drop では、ShioriHostPoster を渡した別スレッドを止めて join すること」 | 要件 4.6 と食い違う |
| `shiori4_abi/src/lib.rs` の `ShioriHostPoster`（868 行付近） | 「unload の後は使ってはならない（Shiori の Drop で、持ち出したスレッドを join すること）」 | 同上 |
| `shiori4/src/lib.rs` の `Ghost` trait（155 行） | 「ShioriHostPoster を渡したスレッドは Drop で止めて join すること」 | 同上 |
| `shiori4_host/src/lib.rs` 冒頭 `//!`（44〜49 行） | 「規約 5（unload）: Drop で、shutdown → 未配送の破棄 → unload → ライブラリ解放」 | 順は保てる。-7 と dir の記述を足す |
| `shiori4_host` の `Host` trait（81〜84 行） | 「`property` / `request` / `log` : SHIORI のスレッド」 | `log` は任意のスレッド（要件 5.4） |
| `GhostRuntime` の Drop（588 行） | 「unload（SHIORI は自分のスレッドを止めてから返る）」 | 要件 4.1 に合わせる |
| `.kiro/steering/tech.md` のスレッド規約 | 「unload は Drop で表す。SHIORI は post するスレッドを止めて join してから返る」 | steering も古くなる（未決 12） |

### 1.2 関係するコードの形
- **`ShioriHostPoster`**（`shiori4_abi`、`shiori4` が `pub use shiori4_abi::*` で再エクスポート）: `#[derive(Clone, Copy)]`、中身は `ctx: *mut c_void` と `post` の関数ポインタだけ。`unsafe impl Send/Sync`。`ShioriHostHandle::poster()` が値を作る。共有の状態を持たないので、無効化の印を置く場所が無い。
- **`shiori_load<S, V>` / `shiori_unload<S>`**（`shiori4_abi`）: `Box<S>` を `*mut c_void` として返し、unload で `drop(Box::from_raw)`。ここが「unload を始めた時点」を知る唯一の場所。
- **`ShioriHostHandle`**: `NonNull` を持つので `!Send`。`log` はこちらにしか無い。別スレッドから log を呼ぶ高水準の口は無い（要件 5.6）。
- **`PendingTags::post(&ShioriHostPoster, ..)`**（`shiori4`）: Poster を借用で受ける。`Copy` を外しても、`Clone` があればこの呼び出しは壊れない。
- **`Bridge<H>`**（`shiori4_host`、非公開）: `host: Arc<H>`、`tx`、`shutdown: Arc<AtomicBool>`。`ShioriHostBox` に入り、ctx として SHIORI に渡る。`request` は呼び出しのスレッドも期間も見ていない。`log` は `Host::log` にそのまま渡す（`Host: Send + Sync` なので任意スレッドから呼んでも健全）。
- **`GhostRuntime::load`**: `ghost_dir.as_ref().to_str()` をそのまま渡す。絶対パス化・末尾の区切りの付与はしていない。`from_static`（テスト用）は `&str` をそのまま。
- **`GhostRuntime::with_api(api, dir, host, lib: Option<Library>)`**: 非公開。`from_library`（要件 12）の中身になる。`load` は v4 だけを試し、NULL なら `Unsupported`。
- **`ShioriApi::load_v3`**（`shiori4_abi`）: v3 のテーブルで host なしの load を呼べる。`ShioriInstance::request` は v3 では body に全文が入る。v3 の口（要件 13）の土台になる。
- **`LoadError`**: `Library` / `NotShiori4` / `Unsupported` / `InvalidPath` / `LoadFailed`。ライブラリを返す変種は無い。
- **Charset**: `shiori4::v3::response` だけが `Charset: UTF-8` を補う（v3 のみ）。v4 の `Adapter` は Charset を付けず、ヘッダを Ghost にそのまま渡す（付いていても拒否しない）。`shiori4_host` の `Bridge`・`Completer` も Charset を付けない。**要件 9 は doc の追記とテストだけで満たせる見込み**。
- **release**: `ShioriBytes::from_vec` 系の `release_vec` はスレッドに依存しない（Box の解放）。`ShioriOwnedBytes`／`ShioriOwnedResponse` は生ポインタを持つので `!Send`（要件 5・未決 7）。
- **再エクスポート**: `shiori4_host` は `SHIORI_E_*` を名前で列挙して再エクスポートしている（64〜70 行）。`SHIORI_E_WRONG_THREAD` を列に足す必要がある（`SHIORI_E_INVALID_UTF8` も今は漏れている）。
- **テスト**: 各 `lib.rs` 末尾の `mod tests`。ホスト側は `GhostRuntime::from_static` で DLL なしに往復させる。DLL を実際に読むテスト（発見の順・許可・`from_library`・v3 の口）には、今のところテスト用の cdylib が無い。

## 2. 要件ごとの対応表（Missing / Unknown / Constraint）

| 要件 | 既存の資産 | ギャップ | 種別 |
|---|---|---|---|
| 1 規約の正本と一致 | C ヘッダ・`//!`・各 trait の doc | 規約 6 のずれ、join の記述 4 か所、steering | Missing（文書） |
| 2 規約 2 と -7 | 定数の並び（-1〜-6） | `SHIORI_E_WRONG_THREAD` の定数（C・Rust）、注釈、`shiori4_host` の再エクスポート | Missing（小） |
| 3 -7 の検出 | `Bridge`、`GhostRuntime` の `&mut self` | 呼び出し中かつ同じスレッドかを `Bridge` が知る仕組みが無い。load の最中（`with_api` の中）も期間に含める必要 | Missing |
| 4 unload と Poster の無効化 | `shiori_load`／`shiori_unload`、`ShioriHostPoster` | 無効化の状態の置き場、実行中の post との競合の扱い、`Copy` の扱い | Missing・Unknown（未決 1・2） |
| 5 log と release | `Bridge::log`、`release_vec` | 注釈。持ち出せる log の口が無い | Missing（小）・Unknown（未決 6・7） |
| 6 notify の後回し | — | 注釈と `Ghost::notify` の doc | Missing（文書） |
| 7 load の失敗と複数インスタンス | `shiori_load` は Err を NULL にするだけ | v4 の失敗の自動 log（仮定）。複数インスタンスは `Box<S>` ごとなので既に受けられる（テストで確かめる） | Missing（小）・Unknown（未決 5） |
| 8 dir の形 | `to_str` での UTF-8 検査 | 絶対パス化・区切りの正規化・末尾の区切り | Missing・Unknown（未決 8） |
| 9 Charset | v4 は既に付けない | doc とテスト | Constraint（現状で満たす） |
| 10 別プロセス | — | 注釈だけ | Missing（文書） |
| 11 発見の順と許可 | `load` の中の `lib.get(SHIORI_GET_API_SYMBOL)` | 許可の集合の型、`loadu`／`load` の探索、従来の口で解決したときの返り値 | Missing・Unknown（未決 9） |
| 12 `from_library` | 非公開の `with_api` | 公開の入口、ライブラリを返すエラーの形 | Missing |
| 13 v3 の口 | `ShioriApi::load_v3`、`ShioriInstance` | ランタイムの型、UTF-8 の検査、後始末の順 | Missing |
| 14 ファイル名と機械の種類 | — | 候補を並べる関数、PE・ELF・Mach-O の読み取り、文書の置き場 | Missing・Unknown（未決 10・11） |
| 15 持ち場・互換・検証 | 依存は `libloading` だけ | テスト用の DLL（cdylib）が無いので、DLL を読む口のテストの方法 | Constraint・Research Needed |

## 3. 実装の選択肢

### 3.1 Poster の無効化（要件 4）
| 案 | 中身 | 長所 | 短所 |
|---|---|---|---|
| A. Arc の門 | `shiori_load` が門（閉じた印 ＋ 実行中の数）を `Arc` で作り、`ShioriHostHandle` と `ShioriHostPoster` に持たせる。`shiori_unload` は門を閉じ、実行中の post が返るのを待ってから `S` を Drop する | 安全で素直。門の寿命を Arc が管理する | `ShioriHostPoster` が `Copy` でなくなる（`Clone` は残る）。0.0.x の破壊的な変更 |
| B. 漏らす門 | 門をインスタンスごとに 1 つヒープに置いて解放せず、Poster は生ポインタで指す | `Copy` を保てる | load ごとに数十バイトを漏らす。読み直しの多いホストで積もる |
| C. 高水準層だけで包む | `shiori4` 側に別の Poster 型を作り、`Ghost` にはそれを渡す | `shiori4_abi` に触れない | `Ghost::load` は `ShioriHostHandle`（`shiori4_abi` の型）を受けるので、結局 `poster()` の生の値が漏れる。ABI 層の `Shiori` を直接使う人は守られない |

- 推奨の候補は A（未決 1 で `Copy` を外してよいかを決める）。`shiori_unload` の中の「実行中の post が返るのを待つ」は、post がホストのキューに積むだけで返る（規約 4）ので短い。これを join と区別して認めるかが未決 2。
- 「unload の後に動き続けるスレッド」は、閉じた門を見て `SHIORI_E_SHUTDOWN` を返すだけになる。ホストの ctx（`ShioriHostBox`）は unload の後に解放されるので、**門を見る前に ctx に触れない**順が肝要。

### 3.2 -7 の検出（要件 3）
| 案 | 中身 | 長所 | 短所 |
|---|---|---|---|
| A. 持ち主のスレッド ＋ 呼び出し中の印 | `Bridge` に「持ち主のスレッド ID」と「呼び出し中か」を持たせ、`GhostRuntime` が load / request / notify の前後で印を立てる | 素直。インスタンスごとに自然に分かれる | `Bridge` は Sync なので印は原子的な型か Mutex。印の立て忘れ（`with_api` の load の最中）に注意 |
| B. スレッドローカルの「いま呼んでいるインスタンス」 | `GhostRuntime` が呼び出しの間だけ、スレッドローカルに自分の `Bridge` の印を積み、`Bridge::request` はそれを見る | スレッドと期間を 1 回で判定できる。同じスレッドで複数のインスタンスを入れ子にしても正しい | スレッドローカルの扱いに慣れが要る |

- どちらも Host 実装者に新しい手続きを求めない（要件 3.5）。
- 呼び出しの外で同じスレッドから呼ばれたときの扱い（未決 3）で、判定の条件が変わる。

### 3.3 新しい読み込みの口（要件 11〜13）
| 案 | 中身 | 長所 | 短所 |
|---|---|---|---|
| A. `lib.rs` に足す | 今の 1 ファイルの構成のまま、`// ====` の区切りで「発見」「v3 の口」「ファイル名と機械の種類」を足す | structure.md の今の形に沿う | `shiori4_host/src/lib.rs` が 722 行から大きく伸びる。host-protocol と同じファイルを触るので衝突しやすい |
| B. モジュールに分ける | `shiori4_host/src/` に、発見・v3 の口・ファイル名と機械の種類のファイルを分ける | 衝突が減る。純粋な関数（ファイル名・機械の種類）を単体で試せる | structure.md は「現状 1 ファイル」と書く。分けるときは層の境界を先に決める、とある（クレートは変えないので守れる） |
| C. 混合 | 規約の振る舞い（-7・dir・Charset）は `lib.rs`、純粋な関数と v3 の口は別ファイル | host-protocol との衝突を最小にしつつ、既存の型の近くに規約の守り方を置ける | 置き場の判断が要る |

- 推奨の候補は C。host-protocol が `Bridge` の振り分けを変えるので、`Bridge` に触れる -7 の検出は先に統合する側を決めておく。

### 3.4 機械の種類の確認（要件 14）
- std だけで足りる。読むのは先頭の数百バイトだけ。
  - PE: `MZ` → `e_lfanew`（0x3C）→ `PE\0\0` → Machine（x64 = 0x8664、arm64 = 0xAA64、x86 = 0x014C）。
  - ELF: `\x7FELF` → EI_CLASS（64bit）→ e_machine（x86_64 = 62、aarch64 = 183）。エンディアンは EI_DATA で決まる。
  - Mach-O: 64bit の magic（0xFEEDFACF）→ cputype（x86_64 = 0x01000007、arm64 = 0x0100000C）。ユニバーサル（0xCAFEBABE、ビッグエンディアン）は arch の表を走査する。0xCAFEBABE は Java の class ファイルと同じ値なので、arch の数が小さいことで見分ける。
- ARM64EC・ARM64X の DLL（Machine は 0xAA64 や 0x8664 を名乗る）の扱いは brief で「除く」とあるだけ。Research Needed。

## 4. 規模と危険度
- **Effort: M（3〜7 日）**。注釈の書き換えは S だが、Poster の門（`shiori4_abi` の load／unload の入口の変更）、-7 の検出、発見の順と許可、`from_library`、v3 の口、純粋な関数 2 つ、DLL を読むテストの用意が重なる。
- **Risk: Medium**。unsafe の境界（unload と実行中の post の競合、ctx の寿命）に触れる。テスト用の DLL が無く、発見の順を本物の DLL で試す方法が未定。host-protocol と同じファイルを触る。新しい技術は無い。

## 5. 設計フェーズへの申し送り
- **推奨の方針**: 規約の本文は C ヘッダを先に書き、Rust の `//!` をそれに合わせて写す。振る舞いは「Poster の門（3.1 A）」「-7 の検出（3.2 A か B）」「新しい口は別ファイル（3.3 C）」を軸に検討する。
- **決めること**（要件の未決の論点と対応）:
  - Poster の `Copy` を外すか（未決 1）、unload の中で実行中の post を待つか（未決 2）。
  - 呼び出しの外の `host->request` の扱い（未決 3）。
  - 従来の口で解決したときの返り値（未決 9）と、許可の集合の型。
  - dir の正規化の細部（未決 8）。`std::path::absolute`（Rust 1.79 で安定。MSRV 1.85 で使える）は `\\?\` を付けず、存在も確かめないので、仮定の振る舞いに近い。
  - ファイル名の決まりを書く場所（未決 10）。
- **Research Needed**:
  - DLL を読む口のテストの方法: ワークスペースにテスト用の cdylib を足すか、`build.rs`／`tests/` で作るか、`cargo test` の 3 OS の CI でどう読ませるか。両方の口を持つ DLL（`shiori_get_api` ＋ `loadu`／`load`）の作り方。
  - `libloading` で `loadu`／`load` の有無だけを見るとき、Windows の 32bit の DLL を 64bit のプロセスで開こうとすると開けない（`Library::new` が失敗する）。発見の前に機械の種類を確かめる順にするか。
  - ARM64EC・ARM64X の PE の見分け方。
  - Windows で `LoadLibrary` 時の DllMain の副作用（発見のためだけに開く場合）。
