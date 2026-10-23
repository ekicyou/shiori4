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
  - dir の正規化（議題 7 で確定）: 相対パスは `Path::is_absolute()` で断り（`InvalidPath`）、`std::path::absolute` は使わない。正規化は区切りと末尾の区切りの見た目だけで、ファイルシステムに触れない。`from_static` のテストの `"/ghost"` は Windows で絶対でないので、OS ごとの絶対パスに直す。
  - ファイル名の決まりを書く場所（未決 10）。
- **Research Needed**:
  - DLL を読む口のテストの方法: ワークスペースにテスト用の cdylib を足すか、`build.rs`／`tests/` で作るか、`cargo test` の 3 OS の CI でどう読ませるか。両方の口を持つ DLL（`shiori_get_api` ＋ `loadu`／`load`）の作り方。
  - `libloading` で `loadu`／`load` の有無だけを見るとき、Windows の 32bit の DLL を 64bit のプロセスで開こうとすると開けない（`Library::new` が失敗する）。発見の前に機械の種類を確かめる順にするか。
  - ARM64EC・ARM64X の PE の見分け方。
  - Windows で `LoadLibrary` 時の DllMain の副作用（発見のためだけに開く場合）。

## 6. 設計フェーズの調査と決定（2026-10-23）

### 6.1 使ったスキルと指針
- `kiro-spec-design`（`rules/design-principles.md`・`design-discovery-light.md`・`design-synthesis.md`・`design-review-gate.md`）。種類は **Extension** とし、軽い発見（既存の拡張点・互換・統合の危険）で進めた。新しい外部の依存は無いので、外部の技術の調査は事実の確認（下の 6.2）に限った。
- steering の `product.md`・`tech.md`・`structure.md`、隣の spec の brief（host-protocol・shiori3-legacy-host）を読んだ。

### 6.2 調査の記録

#### Poster の門（要件 4）
- **Context**: `ShioriHostPoster` は `Copy` で生ポインタだけを持ち、無効化の印を置く場所が無い。unload の後に動き続けるスレッドが ctx（解放済みの `ShioriHostBox`）に触れると解放後の呼び出しになる。
- **Findings**:
  - `shiori_load` が返すポインタを `Box<S>` から非公開の箱 `Instance<S> { gate, shiori }` に替えれば、`shiori_unload` が門に届く。テーブルの関数（`shiori_request`／`shiori_notify`）は同じクレートの中なので、形を変えずに箱の中の `S` を使える。ABI の形は変わらない。
  - 待ちの道具は std の `Mutex<{closed, in_flight}>` ＋ `Condvar` にした。`RwLock<bool>`（post は読み、unload は書き）でも書けるが、std の `RwLock` は OS によって書き手が飢えうる（優先の方針は OS 次第と std の doc にある）。閉じた印を先に立てる `Mutex` 方式なら、閉じた後に新しい post が入らないので待ちが必ず終わる。
  - 門を見る前に ctx を読まない順にすれば、ctx の解放後も門（`Arc`）だけを読んで `SHIORI_E_SHUTDOWN` を返せる。
  - `ShioriHostHandle`（`!Send`）は門を見ない。unload の中（`S` の Drop）ではホストの ctx はまだ有効で、Ghost が Drop で同期の request や log をしても安全なため。
- **Implications**: `ShioriHostPoster` から `Copy` が外れる（議題 1 で確定）。Poster に `log` を足す（議題 5）。

#### -7 の検出（要件 3）
- **Context**: 判定には「呼んだスレッド」と「呼び出しの最中か」の 2 つが要る。load の最中（`with_api` の中）も期間に含める。
- **Findings**:
  - `GhostRuntime` は `!Send` なので、作ったスレッド（`with_api` を呼んだスレッド）がそのまま持ち主のスレッドになる。`Bridge` に `owner: ThreadId` を作るときに入れ、`in_call: AtomicBool` を `GhostRuntime` が立て下ろしすれば足りる（案 A）。
  - 持ち主以外のスレッドは `owner` との比較で必ず外れるので、`in_call` の読みの順序は判定に効かない。`Relaxed` で足りる。
  - 案 B（スレッドローカルの「いま呼んでいるインスタンス」の積み上げ）は、入れ子も正しく扱えるが、案 A でも印が `Bridge` ごとにあるので入れ子は正しく分かれる。案 B は不要。
- **Implications**: `Bridge::request` の先頭に 1 つの `if` を置くだけで済み、host-protocol が作り直す振り分けの本体に触れない。

#### dir の正規化（要件 8）
- **Findings**: Rust の `Path::is_absolute` は Windows で「接頭辞と根の両方がある」ときだけ真で、`\ghost`（根だけ）・`C:ghost`（接頭辞だけ）は偽になる（std の doc）。議題 7 の決定どおり、これで相対パスを断れる。`\?\` の形では `/` は区切りでないので、置き換えない。
- **Implications**: `normalize_dir(&Path) -> Result<String, LoadError>` の 1 関数を、dir を受けるすべての口で使う。

#### 機械の種類（要件 14）
- **Sources**: Microsoft の PE フォーマットの文書（Machine の値・ARM64EC と ARM64X の説明）、ELF の仕様（`e_machine`: EM_X86_64 = 62・EM_AARCH64 = 183）、Apple の `mach-o/loader.h`・`mach-o/fat.h`（CPU_TYPE_X86_64 = 0x01000007・CPU_TYPE_ARM64 = 0x0100000C、FAT_MAGIC 0xCAFEBABE・FAT_MAGIC_64 0xCAFEBABF）、`file(1)` の magic（0xCAFEBABE の次の値で Java の class とユニバーサルを見分ける）。
- **Findings**:
  - 0xCAFEBABE の次の 4 バイトは、Java の class では minor<<16 | major（major は 45 以上）なので、Java 8 なら 52 のような小さい値にもなる。gap 分析の「arch の数が小さい」はそのままでは足りず、`file(1)` と同じく 1〜30 に限ってユニバーサルとみなす。
  - ARM64EC の DLL は PE の Machine が 0x8664（x64）を名乗り、arm64 のネイティブのプロセスでは読めない。ARM64X は 0xAA64 を名乗り、arm64 と x64（EC）の両方のプロセスで読める。ヘッダのとおりに判定すると、ARM64EC は X64、ARM64X は Arm64 になる。x64 のプロセスで ARM64X を「合わない」と判定するのは偽の否定（安全側）。brief は ARM64EC を除くとしているので、特別扱いはしない。
  - 64bit のプロセスで 32bit の DLL を `LoadLibrary` すると `ERROR_BAD_EXE_FORMAT`（193）で失敗し、`libloading` は `LoadError::Library` になる。読み込みの前に機械の種類を確かめる順は、配布の決まり（14.7）としてホストの手順にした。
- **Implications**: std の `File`・`Read`・`Seek` だけで書く。読む量は先頭 4 KiB と PE の `e_lfanew` の先 6 バイトに限る。

#### テスト用の DLL（要件 15.5）
- **Context**: 発見の順・`from_library`・`V3Runtime::from_library` は `libloading::Library` を要るので、本物の DLL が要る。
- **Findings**:
  - テストの実行ファイル自身を開く方法（`Library::this()`）は、Rust の実行ファイルが `#[no_mangle]` の関数を動的に書き出さないので使えない。
  - cdylib をワークスペースのメンバーにして dev-dependency にしても、その成果物のパスを安定に得る手段は無い（artifact dependencies は unstable）。
  - テストの中から `env!("CARGO")` で `cargo build --manifest-path testdata/testghost/Cargo.toml --target-dir target/testghost` を 1 回呼ぶのが、3 OS で同じに動く。テスト用の cdylib に自前の `[workspace]` を持たせればルートのワークスペースの外になり、`cargo package --workspace`・clippy・deny・CI の設定に影響しない。
  - DLL の状態（v4／v3 を出すモード）はプロセスに 1 つなので、DLL を使うテストはロックで直列にする。
  - `shiori_get_api` の無いライブラリには OS のライブラリ（`kernel32.dll`・`libc.so.6`・`/usr/lib/libSystem.B.dylib`）を使う。
- **Implications**: 新しいメンバーも CI の変更も要らない。CI の時間は数十秒延びる見込み。

#### 隣の spec との統合
- host-protocol は `Bridge` の組み込みの振り分け・`accept_post` の既定・`Completer` を作り直し、`Headers` などを便利関数のモジュールへ移す。この spec の `lib.rs` の変更は、`Bridge` のフィールドと `request` の先頭の門・再エクスポートの列・doc・テストに限り、新しいコードは別ファイル（`v3.rs`・`discover.rs`・`dist.rs`・`test_dll.rs`）に置く。
- shiori3-legacy-host は、`LoadedGhost::Legacy { entry, library }` と、許可する口の種類（`EntryKind`）・dir の正規化を前提にする。

### 6.3 設計の総合（synthesis）
- **一般化**: 「4 → 3 → SHIORI/3.0」の発見は、`GhostRuntime::from_library` と `V3Runtime::from_library` を順に呼び、`NotShiori4` で返ったライブラリだけを従来の口へ回す合成として書ける。発見の口は新しい読み込みの仕組みを持たず、要件 12・13 の入口の上に載る。dir の正規化も 1 関数を全口で共有する。
- **作るか採るか**:
  - ヘッダの読み取りは `object`／`goblin` を採らず std で書く（新しい依存を足さない制約。読むのは数フィールドだけ）。
  - ライブラリを開くのは既存の `libloading` をそのまま使う。
  - 門は std の `Mutex`＋`Condvar`。外部の同期のクレートは要らない。
- **簡素化**:
  - 許可する口の集合は専用のビット集合の型を作らず `&[EntryKind]` にする（既定は `EntryKind::ALL`）。
  - 「許可された口が見つからない」は新しい変種を足さず `LoadError::NotShiori4` の意味を広げる。
  - v3 の応答が UTF-8 でないときは新しいエラー型を作らず、既存の `SHIORI_E_INVALID_UTF8` を返す。
  - `V3Runtime` は request だけを持つ（v3 の request は NOTIFY の全文も受けるので notify の口は要らない）。
  - 従来の口の種類は専用の enum を作らず `EntryKind`（`Loadu`／`Load`）を使う。
  - C ヘッダと `//!` の一致を自動で比べる仕組みは作らず、レビューで突き合わせる。
  - 発見の口は機械の種類を自動で確かめない（ホストの手順。x86 のホストなど `TargetArch` に無い arch でも発見の口を使えるようにするため）。

### 6.4 設計の決定（要約。詳しくは design.md）

| 決定 | 選んだもの | 退けたもの |
|---|---|---|
| Poster の無効化 | Arc の門（`Instance<S>` に門を持たせ、unload で閉じて実行中の 1 回分を待つ） | 漏らす門・高水準層だけで包む |
| 待ちの道具 | `Mutex`＋`Condvar` | `RwLock`（書き手の飢え）・スピン |
| -7 の検出 | `Bridge` に持ち主の `ThreadId` と `AtomicBool` の印 | スレッドローカルの積み上げ |
| ファイルの分け方 | 混合（規約の守り方は `lib.rs`、新しい口と純粋な関数は別ファイル） | `lib.rs` に全部・全部を分ける |
| 許可する口の型 | `&[EntryKind]` | ビット集合の新しい型 |
| `from_library` の失敗 | `FromLibraryError`（非公開のフィールドで「ライブラリを返すのは読み直せる失敗だけ」を守る） | `(LoadError, Option<Library>)` のタプル・`LoadError` に変種を足す |
| v3 の口 | `V3Runtime::request(&str) -> Result<String, ShioriStatus>` | 新しいエラー型・notify の口 |
| 機械の種類 | std だけの読み取り、ヘッダのとおりに判定 | 依存の追加・ARM64EC の特別扱い |
| テスト用の DLL | ワークスペースの外の cdylib をテストの中で `cargo build` | メンバーの cdylib・`build.rs`・artifact dependencies |
| 統合の順 | 未定（仮定: この spec が先） | — |

### 6.5 危険と対策
- **unload の待ちでの停止**: ホストの post の実装（`Host::accept_post`・`Host::wake`）が unload を呼んでいるスレッドを待つと止まる。規約 4（コピーしてすぐ返る）をホストの義務とし、`Host` の doc に明記する。`GhostRuntime` の Drop は unload の前に `shutdown` を立てるので、新しい post は `accept_post`・`wake` まで進まない。
- **テストの中の `cargo build`**: 外の環境変数を引き継ぐ・CI の時間が延びる。`--target-dir` を明示し、`OnceLock` で 1 回にする。
- **host-protocol との衝突**: 同じ 2 ファイル。`lib.rs` の変更を最小にし、後から統合する側が門とテストを保つ。
- **相対パスの dir を渡していた既存のホスト**: `InvalidPath` で落ちるようになる。破壊的な変更としてリリースノートに載せ、`shiori4_host` の `//!` の例を絶対パスに直す。

## 7. 設計ディスカッションの決定（2026-10-23、全議題を推奨案で確定）
| 議題 | 決定 | 根拠 |
|---|---|---|
| 1 テスト用 DLL | `testdata/testghost/` をテストの中の `cargo build` で作る。子の cargo から `CARGO_BUILD_TARGET` を外す | 安定版で別クレートの cdylib をテストより先に確実に作らせる方法がほかに無い（artifact dependencies は nightly） |
| 2 unload の待ちで止まる危険 | コードは変えず、`Host::log`・`accept_post`・`wake` の doc に「Drop するスレッドを同期で待たない」と書く | 規約 4 がすでに求めている。`Bridge::log` を捨てると Ghost の Drop の中の log を失う |
| 3 `from_library` の `InvalidPath` | ライブラリを返す（要件 12.2 に足した） | 初期化の前の失敗なので、読み直しても二重の初期化にならない |
| 4 発見の口と機械の種類 | 確かめない | ファイルを選ぶのはホスト側の責任。`machine_matches` を先に使う |
| 5 ARM64EC・ARM64X | ヘッダどおり。doc に 2 つの例外を書く | 作者は `x64` を別に置くので実害が小さい。CHPE の解析は割に合わない |
| 6 ディレクトリ付きの `shiori,` | 最後の区切りで分け、名前の部分にだけ規則を当てる | `lib` がディレクトリの前に付く誤りを防ぐ |
| 7 公開の型の形 | 設計どおり（`V3Runtime::request` だけ、`NotShiori4` を広げる、`Legacy` は `EntryKind`） | 型と変種を増やさない。`Legacy` を作るのはこのクレートだけ |
