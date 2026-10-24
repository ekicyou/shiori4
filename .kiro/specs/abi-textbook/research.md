# ギャップ分析: abi-textbook

## Summary
- **Feature**: `abi-textbook`
- **Discovery Scope**: Extension（文書が中心。既存の 3 クレートの doc・README を書き直し、読み物の文書と C の例と試験用のホストを足す。芯の API は変えない）
- **Key Findings**:
  - 正本はそろっている。規約は C ヘッダ冒頭と `shiori4_abi` の `//!` に同文で 10 の節（版〜配布の決まり）、語彙は `doc/host-protocol.md`（第 0〜8 章・付録 A・B）。hearing.md の「規約そのものの論点」a〜e は、上流ですべて決着している（a: v3 は SHIORI 自身のログ／b: 両方を持ってよい・発見の順／c: 配布の決まり／d: カレントディレクトリはホスト次第／e: `GhostRuntime::from_library`）。教科書は新しい決定を要さず、引いて並べて例を足す仕事になる。
  - **番号の穴**: 「規約 N」で引けるのはスレッド規約 1〜7 だけ。ほかの 9 つの節は箇条書きで番号が無い。要件 4.3 の番号付けは、上流の文に触れずに付けると、上流が節に項目を足したときに番号がずれる（Q3）。
  - docs.rs の例は今 7 本（`shiori4` 1・`shiori4_abi` 1・`shiori4_host` 5〔`dist.rs` の 1 本を含む〕）。hearing.md の 2 と 3 の 13 項目のうち、今の doc にあるのは「getproperty に答える Host」「Completer で completed を返す」「from_library で読み直す」「配布の決まり」の一部だけ。残りはすべて足す。
  - **検査の穴**: C の例をビルドする仕組みが CI に無い（C のコンパイラを使う段が無い）。DLL を要する Rust の例は `no_run` でビルドだけ。SHIORI 側の例（`shiori4`）は、`shiori4_host` の `from_static` を依存の向きの都合で使えず、往復まで実行して確かめる手段が無い。
  - 試験用のホストの部品はそろっている。`load_ghost`／`load_ghost_with`（発見の順）・`LoadedGhost`・`V3Runtime`・`LoadError` の Display・テスト用 cdylib `testdata/testghost`（3 OS でビルド済み・v4/v3 のモード切り替え・ダミーの `loadu`/`load`）。足すのは「引数を受けて表示する薄い殻」と CI の段だけで、形（例か CLI か・配り方）が未決（Q9）。

## 1. Current State Investigation

### 関係する資産
| 場所 | 中身 | この spec との関係 |
|------|------|--------------------|
| `README.md`（ルート、22 行） | 3 クレートの表・`cargo test`・「SHIORI を作る」「ホストを作る」の 2 段落（文だけ、コードの例は無い） | 全体図・最小の例・道案内に書き直す（要件 6.1） |
| `shiori4_abi/README.md`・`shiori4/README.md`・`shiori4_host/README.md`（各 30 行ほど） | 役割・利用者・入口・関係・docs.rs への道案内・ライセンス（crate-publish-ready の形）。リンクは GitHub の絶対 URL | 形は要件 6.2 に既に合う。教科書の各文書への道案内を足す。`readme` は既定（各クレートの `README.md`）で、crates.io の頁になる |
| `shiori4_abi/include/shiori4_abi.h` 冒頭（約 105 行の注釈） | 規約の 10 節。番号はスレッド規約 1〜7 だけ。型の注釈に「スレッド規約 2」「load の規約」などの参照 | 規約の一覧の出典（要件 4.1）。文言は変えない（要件 1.2・Out of scope） |
| `shiori4_abi/src/lib.rs` の `//!`（99 行） | C ヘッダと同文の規約。doc の例 2 本 | 同上。docs.rs の `shiori4_abi` の頁が規約の本文の表示先 |
| `shiori4/src/lib.rs` の `//!` | 芯と便利関数の説明・`Ghost` の例 1 本（`no_run`・`Properties` と `HeaderWriter`） | SHIORI 側の例を足す所（要件 7） |
| `shiori4/src/protocol.rs` | `Headers`・`HeaderWriter`・定数・`getproperty_request` ほか・`Properties`・`PendingTags`（`register`・`take`・`take_from` など）。モジュールの `//!` に例は無い | 要件 7.2 の `PendingTags` の例、要件 5 の語の表の定数名の出典 |
| `shiori4_host/src/lib.rs` の `//!` | スレッドモデル・例（`Host` の getproperty・`on_post` → `host_complete` → `complete_with`）・「規約との対応」（規約 1〜5・load の規約・Charset）・エラー定数の例・配布と発見の例・`retry_v3` | ホスト側の例を足す所（要件 8）。「規約との対応」は全体図（要件 2）の下書きにもなる |
| `shiori4_host` の公開 API | `Host`（`name`・`request`・`accept_post`・`on_post`・`log`・`wake`）・`GhostRuntime`（`load`・`from_library`・`from_static`・`request`・`notify`・`deliver`・`pump`・`pump_limit`・`event_sender`・`is_poisoned`）・`Post`・`Completer::complete_with`・`Event`・`EventSender`・`Delivery`・`Outcome`・`Delivered`・`LoadError`（`Library`・`NotShiori4`・`Unsupported`・`InvalidPath`・`LoadFailed`）・`FromLibraryError`・`load_ghost`／`load_ghost_with`・`EntryKind`・`LoadedGhost`（`V4`・`V3`・`Legacy`）・`V3Runtime`・`shiori_file_candidates`・`machine_matches`・`TargetOs`・`TargetArch` | 例と試験用のホストの部品。新しい API は要らない見込み |
| `shiori4_host/src/lib.rs` の `mod tests`（約 35 本） | `property_post_and_completion`・`completer_delivers_headers_verbatim`・`rejected_post_is_400`・`request_from_other_thread_is_wrong_thread`・`two_runtimes_on_two_threads`・`poster_log_from_other_thread_reaches_host`・`from_library_*`・`load_failure_reason_reaches_host_log` など | 例の材料（テストの `Ghost`・`Host` の形をほぼそのまま例にできる） |
| `shiori4_host/src/lib.rs` のテスト `core_refers_to_protocol_only_by_its_reexport` | 芯のコード行（`//` で始まらない行）に `protocol` の語が 1 行だけあることを確かめる | **Constraint**: 例を `//!` に書く限りは影響しない。例を `#[doc = include_str!(..)]` や `#[cfg(doctest)]` の項目で足すとき、その行に `protocol` の語を書くと落ちる |
| `testdata/testghost/`（自前の `[workspace]`・`publish = false`） | テスト用 cdylib。`shiori_get_api` はモードで v4/v3 を返し分け、ダミーの `loadu`・`load` を持つ。`shiori4_host/src/test_dll.rs` の `dll_path()` が `cargo build` で作り、`lock()` で守る | 試験用のホストの CI の相手（要件 13.8）。C のホストの例の実行の相手にもなる（Q10） |
| `doc/host-protocol.md`（357 行） | 語彙の正本。第 8 章が語と定数名の表、付録 A が 0.0.1 からの移行 | 語の表・確定の範囲の出典（要件 5）。読み物を `doc/` に置く前例（Q1） |
| `.github/workflows/build.yml` | test（3 OS: `cargo test --all`・clippy）・deny・package（`cargo package --workspace` と中身の検査）・release-scripts | 例の検査と試験用のホストの段を足す所（要件 13.8・14）。**Constraint**: CI を変える PR は CI の完了を見届けてから統合する（MEMORY・workflow.md） |
| `.kiro/specs/shiori3-legacy-host/brief.md` | 「abi-textbook の試験用のホストが、従来の DLL も読めるようにするためのつなぎ」を In に持つ | 試験用のホストは従来の口を足せる形にしておく（要件 13.6）。移行の手引きのホスト側の記述が後で変わる（Q7） |

### 守るべき規約・パターン
- 規約と方針はクレート冒頭の `//!` に書き、README は使い方の入口だけ（structure.md）。規約を変えたら C ヘッダと `//!` の両方を直す（tech.md）。この spec は規約を変えないので、C ヘッダと `//!` の規約の節には触れない前提。
- コメントとドキュメントは日本語（tech.md）。
- 外部依存を増やさない（`shiori4_abi`・`shiori4` は依存ゼロ、`shiori4_host` は `libloading` だけ）。Win32 の API の crate や C のビルド用の crate を足すと、この方針と `deny.toml` に触れる。
- 依存は下向きだけ（`shiori4_host` → `shiori4` → `shiori4_abi`）。`shiori4` の doc の例から `shiori4_host` を使えない（dev-dependency の循環は cargo が許すが、`cargo package` と版の要求の扱いが難しくなる）。
- テストは同じファイルの `mod tests`。DLL を使うテストは `test_dll::lock()` を取る。
- 版はルート `Cargo.toml` の 3 か所。公開するクレートは 3 つ（roadmap・release.yml・`/release`）。

## 2. Requirements Feasibility Analysis

### 要件と資産の対応
| 要件 | 必要なもの | 今あるもの | ギャップ |
|------|-----------|-----------|---------|
| 1 構成と正本 | 読む順の道案内・正本の引き方・版の表示 | 正本は C ヘッダ・`//!`・`doc/host-protocol.md`。道案内は README の数行 | **Missing**: 教科書の目次と道案内。**Unknown**: 読み物の置き場所（Q1）。**Constraint**: 正本の文言を変えない |
| 2 全体図 | 8 関数 × 呼ぶ側・呼ばれる側・スレッド・期間・違反の結果の表 | 規約 1〜7 の文、`shiori4_host` の「規約との対応」 | **Missing**: 表そのもの。材料は全部ある。`release`（規約 6）・`log`（規約 4）・unload の後（規約 5）の行の根拠もヘッダにある |
| 3 最小の例 | Rust と C の SHIORI・ホストの 4 本 | Rust の SHIORI の例（`shiori4` の doc、約 30 行）・Rust のホストの例（`shiori4_host` の doc。ただし `on_post` を含み最小ではない） | **Missing**: C の SHIORI とホストの例、最小に絞った Rust のホストの例。**Unknown**: 最小の SHIORI を v4 にするか v3 にするか（Q2） |
| 4 規約の一覧と理由 | 全規約の番号と 1〜2 文の理由 | 規約の文（番号はスレッド規約だけ）。理由は spec の記録（`.kiro/specs/completed/abi-conventions/`・`host-protocol/` の requirements・design・research）と hearing.md の 6 に散在 | **Missing**: 番号付けと理由の執筆。**Unknown**: 番号の付け方と、ヘッダに書き入れるか（Q3）、理由の置き場所（Q4）。**Constraint**: 理由の根拠は完了済みの spec の記録から引く（新しい理由を作らない） |
| 5 語の表と確定の範囲 | 語 → 定数 → 正本の節の表、版ごとの確定の一覧 | 第 8 章の表（語と定数名）、付録 A（0.0.1 → 0.0.2） | **Missing**: 正本の節への対応と、ステータス（200・204・400・500・311・312）の行（第 8 章の表には ID・ヘッダ・値の語だけで、ステータスの行が無い）。**Unknown**: 「確定」の判定の基準（Q5） |
| 6 README | ルート: 全体図・最小の例・道案内。各クレート: 道案内の追加 | 形は crate-publish-ready の要件に合う | **Missing**: ルートの書き直し。**Constraint**: crates.io の頁では相対リンクが切れる（今は GitHub の絶対 URL で回避している）。README の例を検査するには、README を doc として取り込む（`include_str!`）か、同じ例を doc に重ねて置く必要がある（設計） |
| 7 SHIORI 側の例 | 5 本（アクター中継・PendingTags・join しない unload・インスタンスごとの状態・v3/v4 両対応） | `Ghost`・`ShioriHostHandle`（`request_str`・`log`・`poster`）・`ShioriHostPoster`（`post`・`post_str`・`log`・`.clone()`）・`PendingTags`・`Properties`（v3 では受動キャッシュ） | **Missing**: 例 5 本。**Constraint**: `shiori4` の doc から `shiori4_host` を使えないので、例は `no_run`（ビルドだけ）になるか、`shiori4_abi` の型で偽のホストのテーブルを組む必要がある。アクター中継の例は規約 2（期間の中・同じスレッド）を守る形を示すので、ビルドだけでは中継の正しさを確かめられない |
| 8 ホスト側の例 | 9 項目 | getproperty の Host・`host_complete` → `complete_with`（completed）・`from_library` と `retry_v3`・配布の決まり・`from_static`（テストにある） | **Missing**: LoadError の値ごとの扱いの一覧・channel と Win32 の pump のつなぎ・interrupted と NOTIFY の完了・nobreak の 204・setproperty と forbidden・unload の順序の例・`from_static` のテストの例。**Constraint**: Win32 のメッセージの輪は依存なしにビルドできない（Q6）。`GhostRuntime::load` の NotShiori4 の挙動（ライブラリを手放す）は今も同じで、doc に明記が無い（`from_library` の doc にだけ逆の道がある） |
| 9 C の説明と例 | 所有権の表と図・release のスレッド・負の戻り値の解放・静的 CRT・版の判定・最小の C のホスト・`SHIORI_HAS_FIELD`・スレッド規約の言い直し・言語ごとの例外 | C ヘッダの型の注釈と `SHIORI_HAS_FIELD`・`SHIORI_API_IS_VALID` のマクロ（例は無い） | **Missing**: すべて。**Constraint**: C の例を CI でビルドする段が無い（要件 14.2）。Windows の runner で MSVC を使うには開発者用の環境の設定が要り、Linux・macOS は `cc` がある（Research Needed） |
| 10 別のプロセス | 3 つの注意 | 「ホストのプロセス」の節（2 文） | **Missing**: 執筆だけ（短い） |
| 11 移行の手引き | SHIORI 側 3 段・ホスト側の差分の表 | 版の節（v3 は全文・Charset は UTF-8）・load の規約（dir の形）・発見の順・付録 A | **Missing**: 差分の表の執筆。**Unknown**: 従来の DLL を shiori4_host で読む道（shiori3-legacy-host）をどう書くか（Q7） |
| 12 配布の決まり | 手順・Rust の例・SHIORI の作者向けの名前 | 配布の決まりの節・`shiori_file_candidates`・`machine_matches` と doc の例 2 本 | **Missing**: 章としてまとめること、SHIORI の作者向けの「どの名前で置くか」（Rust の cdylib は Linux で `lib<名前>.so`、macOS で `lib<名前>.dylib`、Windows で `<名前>.dll` を作る。`.x64` などの接尾辞は自分で付け替える）。hearing c の論点は決着済み |
| 13 試験用のホスト | 引数で DLL とディレクトリと要求を受け、発見 → 要求 → 表示 → unload、失敗の種類と終了の状態 | `load_ghost_with`・`LoadedGhost`・`V3Runtime`・`LoadError` の Display・テスト用 cdylib | **Missing**: 殻（引数・表示・終了の状態）と CI の段。**Unknown**: 形と配り方（Q9）、post の扱い（Q8）。**Constraint**: 公開するクレートを増やすと release.yml・`/release`・版の 3 か所・`cargo package` の検査・README の表に波及する |
| 14 例の検査 | Rust の例を 3 OS でビルド・実行、C の例のビルドと実行、検査しない例の明示 | `cargo test --all` が 3 OS で doctest を回す（`no_run` はビルドだけ） | **Missing**: C の例の段、README の例の取り込み、試験用のホストの段。**Constraint**: CI を変える PR になる |

### 複雑さの性質
- 大半は執筆（全体図・規約の一覧と理由・語の表・移行・配布・C の説明・別のプロセスの注意）。新しいアルゴリズムは無い。
- 例は約 20 本（最小の例 4・SHIORI 側 5・ホスト側 9 前後・C の説明の例数本）。正しさの急所は規約 2（期間とスレッド）と規約 5（unload の後）を守る例の形。
- 試験用のホストは既存の部品の薄い殻。
- CI の段の追加（C のビルドと実行・試験用のホストの往復）が唯一の外部の環境への依存。

## 3. Implementation Approach Options

### Option A: 既存の doc と README に足す（読み物も crate doc に置く）
- 読み物の本文を各クレートの `//!`（または `#![doc = include_str!("../doc/*.md")]` でクレートに取り込む Markdown）に置き、docs.rs を教科書の表示先にする。規約の一覧と理由は `shiori4_abi` の doc、移行と配布はそれぞれの側のクレートの doc。試験用のホストは `shiori4_host/examples/` の 1 本。
- **Trade-offs**:
  - ✅ docs.rs だけで読める。doctest で例が検査される。
  - ✅ 試験用のホストを `examples/` にすると、`cargo test --all` が 3 OS でビルドする。公開するクレートが増えない。
  - ❌ `shiori4_abi` の doc に理由を足すと、C ヘッダと `//!` の同文の約束（tech.md）が崩れる。取り込む Markdown をパッケージに入れる設定（`include`）が要る。
  - ❌ `examples/` の試験用のホストは、pasta の CI から使うには shiori4 のソースを取ってくる必要がある。

### Option B: 読み物は `doc/` の新しい文書、例は doc に、試験用のホストは別のクレートに
- `doc/`（例: `doc/textbook/` か `doc/abi-*.md`）に読み物を置き、README と各クレートの doc から GitHub の URL で導く（`doc/host-protocol.md` と同じ扱い）。C の例は `doc/` の隣（または `testdata/` の下）の `.c` のファイルとして置き、CI でビルドする。試験用のホストはワークスペースの新しいクレート（小さな CLI）。
- **Trade-offs**:
  - ✅ C ヘッダと `//!` に触れずに、理由と番号を教科書の側だけに置ける。
  - ✅ CLI は pasta などの CI から `cargo install --git` で取り込める。
  - ❌ 読み物の文書は doctest の外なので、文書の中の Rust の例は doc の例と重ねて置くか、検査しない例として明示するしかない。
  - ❌ CLI をワークスペースの公開しないメンバーにすると `cargo package --workspace` の扱いの確認が要る。公開するなら 4 つ目のクレートとしてリリースの仕組みに波及する。

### Option C: ハイブリッド（推奨の起点）
- **読み物**: `doc/` の文書に置く（Option B）。規約の一覧と理由・全体図・語の表と確定の範囲・別のプロセス・移行・配布・C の説明。C ヘッダと `//!` には手を入れない（入れるなら導線の 1 行だけ）。
- **Rust の例**: 各クレートの `//!`（docs.rs）に置き、doctest で検査する。README の最小の例は README を doc として取り込んで doctest に乗せるか、doc の例と同じものであることをテストで確かめる。
- **C の例**: `.c` のファイルとして置き、テスト用 cdylib と同じく「テストから C のコンパイラを呼んでビルドし、C のホストでテスト用の SHIORI を読む」か、CI の段で直接コンパイルする（設計で選ぶ）。
- **試験用のホスト**: 部品は `shiori4_host` に既にあるので、まず `shiori4_host/examples/` の 1 本で作り、CI でテスト用 cdylib を相手に 3 OS で動かす。pasta への配り方（例のまま `cargo run --example` を使ってもらうか、別のクレートに切り出すか）は Q9 の裁定に従う。
- **Trade-offs**:
  - ✅ 上流の文言の同文の約束を崩さない。公開するクレートを増やさずに始められる。
  - ✅ Rust の例は既存の doctest の仕組みで腐らない。
  - ❌ 読み物の文書の中の例と doc の例の重複を、テストか手順で守る必要がある。
  - ❌ C の検査の段の作りは新しい（Research Needed）。

## 4. Research Needed（設計へ持ち越す）
- **C の例のビルド**: 3 OS の GitHub の runner での C のコンパイラ（Linux の `cc`・macOS の `clang`・Windows の MSVC か MinGW）の呼び方。外部の crate（`cc` など）を足さずに、テストからコンパイラを呼ぶか、CI の段で呼ぶか。Windows で MSVC を使うときの開発者用の環境の設定。
- **README の例の検査**: README を `#![doc = include_str!("../README.md")]` か `#[cfg(doctest)]` の項目で取り込むときの、crates.io の頁と docs.rs の頁での見え方と、`core_refers_to_protocol_only_by_its_reexport` のテストとの兼ね合い。
- **SHIORI 側の例の実行**: `shiori4` の doc で往復まで実行するために、偽の `ShioriHostApi`（`shiori4_abi` の型）を例の中で組むか、`no_run` にしてホスト側のテストで同じ形を確かめるか。
- **試験用のホストの置き場所**: `examples/` がクレートのパッケージに入るか（`cargo package` の対象と `include`）、ワークスペースの公開しないメンバーを `cargo package --workspace` がどう扱うか、`testdata/testghost` と同じ「自前の `[workspace]`」の形にするか。
- **規約の理由の根拠**: 完了済みの spec（abi-conventions・host-protocol）の requirements・design・research から、各規約の理由として引ける文の洗い出し。理由が記録に無い規約の扱い（上流へ申し送るか、開発者に聞くか）。
- **語の表のステータスの行**: 第 8 章の表にステータス（200・204・400・500 と `SHIORI_E_*`）の行が無い。教科書で足す行が「語を変えない」に当たるか（ステータスの定数は `shiori4_abi` にある）。

## 5. Implementation Complexity & Risk
- **Effort: L（1〜2 週間）**: 執筆の量が多い（読み物 8 種・例約 20 本）。C の例と CI の段は新しい作り。試験用のホストは小さい。
- **Risk: Medium**: 技術は既知だが、(1) 規約の番号は一度出すと固定になる（Q3 を誤ると下流の引用が壊れる）、(2) C の検査の段は CI を変える（3 OS の runner の差）、(3) 試験用のホストの配り方次第でリリースの仕組みに波及する（Q9）。正本の文言を写し間違えると「語を変えない」に反する。

## 6. Recommendations for Design Phase
- **推奨の起点**: Option C。読み物は `doc/`、Rust の例は `//!` と doctest、C の例はファイルと CI の検査、試験用のホストはまず `shiori4_host/examples/`。
- **要件ディスカッションで決まったこと**（2026-10-24。詳細は requirements.md の「決定事項」）: 読み物は `doc/`（Q1）。規約の番号は C ヘッダと `//!` にも書き入れ、写しの一致を CI で確かめる（Q3・Q4）。理由は教科書だけ（Q4）。Win32 の例は Windows でビルドまで検査（Q6）。試験用のホストは `shiori4_host` の example 1 本で、pump 1 回・完了通知なし（Q8・Q9）。C の例はビルドと実行、言語を入れ替えた組で往復（Q10）。
- **設計で決めること**:
  1. 読み物の文書の分け方と名前（1 本か、章ごとか）と、README・各クレートの doc・C ヘッダからの導線。
  2. 「節の名前 ＋ 番号」を C ヘッダと `//!` に書き入れる書式と、規約の写しの一致を確かめる検査の作り（Rust のテストか CI の段か）。
  3. Win32 の関数の宣言を例の中で隠す書き方と、Windows 以外での扱い（`cfg`）。
  4. 試験用のホストの名前・引数の形（要求の与え方）・終了の状態の値、`cargo install --git … --example` で取り込めるかの確認、shiori3-legacy-host が従来の口を足す継ぎ目。
  5. C の例のビルドと実行の段の作り（3 OS のコンパイラの呼び方・Windows の MSVC の環境）。
  6. README の例と doc の例の重複の守り方。
  7. SHIORI 側の例をどこまで実行するか（偽のホストのテーブルを組むか、`no_run` か）。
- **守ること**: C ヘッダと `shiori4_abi` の `//!` の同文の約束、正本の綴り、外部依存を増やさないこと、`core_refers_to_protocol_only_by_its_reexport` のテスト、`cargo package --workspace` の検査、CI を変える PR は CI を見届けてから統合すること。

---

# 設計フェーズの調査（2026-10-24、`/kiro-spec-design abi-textbook -y`）

## Summary（設計）
- **Discovery Scope**: Extension（軽い discovery: `rules/design-discovery-light.md`）。新しい外部依存は無く、確かめたのは cargo・rustdoc・GitHub の runner の振る舞いだけ。
- **使ったスキルと指針**: `kiro-spec-design`（`design-principles.md`・`design-discovery-light.md`・`design-synthesis.md`・`design-review-gate.md`、テンプレート `design.md`・`research.md`）。足す物を最小にし既存の部品を使う方針を、build-vs-adopt と simplification の判断に使った。
- **Key Findings**:
  - `cargo install --git <url> shiori4_host --example shiori4-testhost` は、通常の依存だけを使う example を取り込める（手元の git の URL で確かめた）。`examples/` は `cargo package` のパッケージに入り、`cargo package --workspace` は通る。
  - `#[cfg(doctest)] #[doc = include_str!("../../README.md")]` で、パッケージの外の文書の Rust の例を doctest に載せられる。存在しないファイルを指しても通常のビルドは通る（cfg が先に項目を外す）。行に `protocol` の語が無ければ `core_refers_to_protocol_only_by_its_reexport` に触れない。
  - C ヘッダは MSVC で `/std:c11` が無いと `_Static_assert` で落ちる。`/std:c11 /utf-8 /W4 /WX` なら通り、`/MT /LD` の C の SHIORI と C のホストが testghost・試験用のホストと往復できた。
  - 前のギャップ分析の「規約の 10 の節」は数え違いで、節は 9 つ（版／ABI 規約／戻り値／スレッド規約／load の規約／ホストのプロセス／DLL の口と発見の順／post の完了通知／配布の決まり）。スレッド規約のほかの節は 8 つ。最上位の項目は計 40。
  - 正本に、`load`・`unload` を呼べるスレッドと、`release` を呼べる期間の定めが無い（要件 1.3 の「未決」。下の「正本の未決」）。

## Research Log（設計）

### `cargo install --git … --example` で試験用のホストを取り込めるか
- **Context**: 決定事項 Q9（引数の形と `--example` の取り込みが動くかを設計で確かめる）。
- **Sources Consulted**: 手元の実験（作業ツリーを scratchpad に clone し、`shiori4_host/examples/shiori4-testhost.rs` を足してコミット）。cargo 1.99.1。
- **Findings**:
  - `cargo install --git file:///<clone> shiori4_host --example shiori4-testhost --root <dir>` が成功し、`<dir>/bin/shiori4-testhost.exe` ができた。GitHub の URL も同じ git の取り込みの仕組みを使う。
  - 入れた実行ファイルで testghost（v4 で 200）、C の SHIORI（200）、`kernel32.dll`（`NotShiori4` で終了の状態 3）を確かめた。
  - `cargo install` は dev-dependencies を使わないので、example は通常の依存（`shiori4_host` 自身と std）だけで書く。
  - `Cargo.lock` はコミットしていないので、取り込みは毎回最新の依存で解決される（tech.md の方針どおり）。
- **Implications**: 試験用のホストは `shiori4_host` の example 1 本で足り、公開するクレートを増やさない。取り込み方は `--tag vX.Y.Z` で版を固定する形を教科書に書く。crates.io からの `cargo install shiori4_host --example …` も同じ仕組みだが、公開前のため未確認。

### `examples/` は `cargo package` に入るか
- **Findings**: `cargo package --workspace --list` に `examples/shiori4-testhost.rs` が出る。`cargo package --workspace`（検証のビルドを含む）は通る。
- **Implications**: 公開した `shiori4_host` にも試験用のホストが入る（害は無い）。package の job の中身の検査を変える必要は無い。

### パッケージの外の文書の検査（README・教科書の章・規約の写し）
- **Context**: 設計で決めること 2・6、`core_refers_to_protocol_only_by_its_reexport` との兼ね合い。
- **Findings**:
  - `#[cfg(doctest)] #[doc = include_str!("../../README.md")] struct RootReadmeDoctests;` を `shiori4_host/src/lib.rs` に足すと、ルートの README の ```` ```rust ```` の塊が `cargo test --doc -p shiori4_host` に載る（`--list` で確かめた）。`sh` などの言語の付いたフェンスは載らない。
  - 同じ形で存在しないファイルを指しても `cargo build` は通る（`#[cfg(doctest)]` が `include_str!` の展開より先に項目を外す）。`cargo package --workspace` も通る。crates.io から取ったパッケージで `cargo test --doc` を走らせたときだけ落ちる。
  - `shiori4_host` の doctest から `use shiori4::…` で `shiori4` を名指しできる（通常の依存も `--extern` で渡る）。
  - `export_ghost!` を使う doctest を 2 つ並べても、`fn main` を書いた例は merged doctest でも衝突しない（`#[no_mangle]` の `shiori_get_api` が 2 つあっても通った）。
  - 外部の crate を使わない Win32 の例（`#[link(name = "user32")] unsafe extern "system" { PostThreadMessageW, GetMessageW }`・`GetCurrentThreadId`）を隠し行の `#[cfg(windows)]` で包んだ doctest が Windows でビルドと実行まで通った。
  - `shiori4_abi/tests/textbook.rs` を `exclude = ["tests/textbook.rs"]` でパッケージから外すと、リポジトリの中では `cargo test` で走り、`cargo package` は「含まれないテスト `textbook` を無視する」の警告を出すだけで通る。
  - `core_refers_to_protocol_only_by_*` は `mod tests {` より前の、`//` で始まらない行の `protocol` の語を数える。`#[doc = include_str!("../../doc/host-protocol.md")]` は `host-protocol` が語 `protocol` に分かれるので落ちる。教科書の章の名前に `protocol` を含めない。
  - リポジトリに `.gitattributes` が無く、手元の git は LF を CRLF に替えると警告した。Windows の runner の checkout も CRLF になりうるので、比べる前に CRLF を LF にする。
- **Implications**: 正本の一致の検査は integration test（パッケージの外）、Rust の例は `#[cfg(doctest)]` の取り込み、という 2 つの置き場所に分けた（design.md の TextbookConsistencyTest・DoctestHooks）。

### SHIORI 側の例（`shiori4`）をどこまで実行するか
- **Context**: 設計で決めること 7。`shiori4` の doc は `shiori4_host` を使えない。
- **Findings**: `shiori4_abi` に `ShioriHost` trait・`ShioriHostBox::new`・`ShioriApi::new::<S, V>()`・`ShioriApi::load(&host_box, dir)`・`load_v3` があり、`shiori4` が `pub use shiori4_abi::*` で再エクスポートしている。`shiori4_host` の `Bridge` と同じ形の偽のホストを例の隠し行で組める。
- **Implications**: `shiori4` の例 5 本は、偽のホストで往復まで実行して `assert!` で確かめる（`no_run` にしない）。

### C のコンパイラ（3 OS の runner）
- **Context**: 設計で決めること 5、決定事項 Q10。
- **Sources Consulted**: 手元の Windows 11（Visual Studio 2022 Professional、`%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe`）での実験。GitHub の runner のイメージは、windows に Visual Studio 2022（MSVC と `vswhere`）、ubuntu に gcc（`cc`）、macOS に Xcode の Apple clang（`cc`）を持つ（runner-images の既定の構成。CI で初めて確かめる）。
- **Findings**:
  - `vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath` → `<VS>\VC\Auxiliary\Build\vcvars64.bat` を同じ cmd の中で `call` すると `cl` が使える。サードパーティの Action（`ilammy/msvc-dev-cmd` など）は要らない。
  - `/std:c11` が無いと、C ヘッダの `SHIORI_STATIC_ASSERT`（`_Static_assert`）の行で構文エラーになる。日本語の注釈があるので `/utf-8` を付ける（付けないと既定のコードページで C4819 の警告になりうる）。
  - `cl /nologo /W4 /WX /utf-8 /std:c11 /MT /LD shiori.c`（静的 CRT の DLL）と `cl … host.c` がどちらも警告なしで通り、C のホスト × testghost で `200` と `Value: testghost`、C のホスト × C の SHIORI で `200` を得た。
  - `cl` の環境は段をまたいで残らないので、ビルドは 1 つの `shell: cmd` の段で行う。
  - Linux・macOS では `cc -std=c11` のとき `dlopen` の宣言のために `_POSIX_C_SOURCE` を定義する（glibc の厳密なモード）。
- **Implications**: CI の job `textbook` は、Windows のビルドの段だけ `shell: cmd`、ほかのビルドと往復は `shell: bash`。C の例の検査は CI だけで行い、手元の DoD（`cargo test --all`・clippy）には入れない。

### 規約の理由の出典
- **Findings**: 完了済みの spec に、要件 4.6 の理由の出典がある。`completed/abi-conventions/`（スレッド規約 1〜7 の論点・`SHIORI_E_WRONG_THREAD`・unload と Poster）、`completed/host-protocol/brief.md` の論点 9（SEND の既定の割り込みは SSTP に揃えた。post は同じプロセスからの SSTP なので、語・既定・ukadoc の参照・areka の既存の裁定を 1 つにできる、が理由）。`hearing.md` の 6 も要望の出どころになる。
- **Implications**: 理由は各規約の後に出典付きで書き、出典の無いものは草案の印を付ける（要件 4.8）。SEND の割り込みは C ヘッダの規約ではないので、規約の節の後の「ホスト側プロトコルの決まりの理由」に置く（design.md の Open Question 5）。

## 正本の未決（要件 1.3。開発者の裁定へ回す）
1. **`load`・`unload` を呼べるスレッド**: スレッド規約 1 は `request`・`notify` の単一のスレッドだけを定め、`load`・`unload` のスレッドを定めない（スレッド規約 2 は load の中から `host.request` を呼べることだけを言う）。`shiori4_host` は作ったスレッドで load し、同じスレッドで Drop する（`GhostRuntime` は `!Send`）。
2. **`release` を呼べる期間**: スレッド規約 6 は「どのスレッドから呼んでもよい」とだけ言い、unload の後・ライブラリを放した後に呼んでよいかを定めない。`shiori4_host` の `GhostRuntime::request` が返す `ShioriOwnedResponse` はライブラリの寿命に縛られず、ランタイムを Drop した後に捨てると、外れたライブラリの `release` を呼びうる（潜在の不具合。API の変更が要るのでこの spec の外）。

## Architecture Pattern Evaluation（設計）

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 正本を引く文書 ＋ 写しごとの機械の検査（採用） | 読み物は `doc/textbook/`、例は実際にビルドされる場所、写しはテストで正本と比べる | 「語を変えない」を機械で守れる。依存を増やさない | 検査の置き場所が 3 つ（integration test・doctest・CI の job）に分かれる | ギャップ分析の Option C の具体化 |
| 読み物も crate doc に置く | `#![doc = include_str!]` で docs.rs を教科書にする | docs.rs だけで読める | C ヘッダと `//!` の同文が崩れる。Q1 で不採用 | — |
| 試験用のホストを別のクレートにする | ワークスペースの 4 つ目のクレート | `cargo install` の名前が素直 | リリースの仕組みに波及。Q9 で不採用 | — |
| C の例を Rust のテストからコンパイルする | テストが `cl`・`cc` を探して呼ぶ | 手元の `cargo test` で走る | コンパイラの探し方（`vcvars`）を Rust で書くことになる。`cc` の crate は足せない | CI の段にした |

## Design Decisions（設計）

### Decision: 教科書の章の分け方と置き場所
- **Context**: 設計で決めること 1。
- **Alternatives Considered**: 1 本の長い文書／章ごとのファイル／ルートの README を全体図と最小の例の置き場所にする。
- **Selected Approach**: `doc/textbook/` に目次（`README.md`）と 7 つの章。全体図と最小の例はルートの README に置き（要件 6.1 がそこに置くことを求める）、目次からそこへ導く。重複して置かない。
- **Rationale**: 章ごとに下流がリンクでき、全体図を 2 か所に置かずに済む。
- **Trade-offs**: 目次から README へ一度戻る読み方になる。

### Decision: 規約の番号の書式と書き入れ方
- **Context**: 設計で決めること 2、決定事項 Q3。
- **Selected Approach**: スレッド規約以外の 8 つの節の最上位の項目の `- ` を `N. ` に替え、字下げを 3 つの空白に揃える。引用は「<節の名前> N」。下位の項目は親の番号で引く。
- **Rationale**: スレッド規約の既存の書式（`1.`）と揃い、`//!` の rustdoc の描画も保てる。文字は替えない。
- **Trade-offs**: 下位の項目を単独で引けない（design.md の Open Question 4）。

### Decision: 写しの一致の検査は integration test にし、パッケージから外す
- **Context**: 要件 14.6、設計で決めること 2。
- **Alternatives Considered**: `shiori4_abi` の `mod tests`（パッケージの外のファイルを `include_str!` するので、取ったパッケージのテストが落ちる）／CI の段の script（手元の DoD で走らない）。
- **Selected Approach**: `shiori4_abi/tests/textbook.rs` を `exclude` でパッケージから外す。std だけ。
- **Rationale**: 手元の `cargo test --all` で走り、パッケージも壊さない。
- **Trade-offs**: `cargo package` の警告が 1 つ出る（design.md の Open Question 11）。

### Decision: README と章の Rust の例は `#[cfg(doctest)]` で取り込む
- **Context**: 設計で決めること 6。
- **Selected Approach**: `shiori4_host/src/lib.rs` に文書ごとの `#[cfg(doctest)]` の項目。C の塊は「C のファイルか C ヘッダからのそのままの抜き出し」に限り、integration test で確かめる。
- **Rationale**: 例を 2 か所に書かずに済み、新しい仕組みが要らない。
- **Trade-offs**: 取ったパッケージで `cargo test --doc` を走らせると落ちる（design.md の Open Question 10）。

### Decision: Win32 の例は宣言を見せ、cfg を隠し行に置く
- **Context**: 設計で決めること 3、決定事項 Q6。
- **Selected Approach**: 使う Win32 の関数の `extern "system"` の宣言は読者に見せ、`#[cfg(windows)]` の `mod` と `fn main` は隠し行にする。`no_run`。
- **Rationale**: 写して使える形のまま、Windows だけでビルドを検査できる。ほかの OS では空の例になる。

### Decision: 試験用のホストの形
- **Context**: 設計で決めること 4、決定事項 Q8・Q9。
- **Selected Approach**: `shiori4_host/examples/shiori4-testhost.rs`。`[-H "名前: 値"]... <lib> <dir> [ID]`。終了の状態 0/1/2/3/4/64。`load_ghost` で全部の口を許可し、`LoadedGhost::Legacy` の枝を shiori3-legacy-host の継ぎ目にする。
- **Rationale**: 既存の部品（`load_ghost`・`LoadedGhost`・`V3Runtime`・`LoadError` の `Display`）だけで作れ、引数の crate を足さない。
- **Trade-offs**: 名前と引数と終了の状態が下流の契約になる（design.md の Open Question 7）。

### Synthesis の結果
- **Generalization**: 「正本を写した物は、写しの置き場所ごとに 1 つの機械の検査で正本と結ぶ」を共通の規則にした（規約の写し・配布の決まりの写し・語の表・C の断片がすべて同じ正規化の関数を使う）。C の塊の検査は「どの C の塊も、C のファイルか C ヘッダの抜き出し」という 1 つの規則で README と `c.md` の両方を覆う。
- **Build vs Adopt**: C のビルドは runner に入っている MSVC・gcc・clang を使い、`cc` の crate やサードパーティの Action を足さない。doctest の取り込みは rustdoc の既存の機能（`#[doc = include_str!]`）を使う。CLI の引数の crate は足さず std で解析する。偽のホストは `shiori4_abi` の既存の `ShioriHostBox` を使う。
- **Simplification**: 全体図と最小の例は README の 1 か所だけに置く。版の判定と `SHIORI_HAS_FIELD` の例は別のファイルにせず、最小の C のホストに含める（C のファイルは 3 本）。試験用のホストに `--notify` や版を指定する引数を足さない。C のホストの終了の状態は 0 と 1 だけ。各クレートの README にはコードの例を置かない。

## Risks & Mitigations（設計）
- 規約の番号は公開後に固定になる — 番号の付け方（design.md の Open Questions 3・4）を開発者が確かめてから統合する。番号を使い回さない方針を `rules.md` に書く。
- Linux・macOS の C のビルドは手元で確かめていない — CI を変える PR なので、CI が通るのを見届けてから統合する（Q10・workflow.md）。
- Windows の runner の改行（CRLF）で写しの比較がずれる — 比べる前に CRLF を LF にする。
- 偽のホストを使う `shiori4` の例は、規約の判定（WRONG_THREAD など）の正本ではない — 判定の正本は `shiori4_host` の `Bridge` とそのテストに残し、例では「偽のホストで確かめている」と書く。

## References（設計）
- `shiori4_abi/include/shiori4_abi.h`・`shiori4_abi/src/lib.rs`（正本の規約）
- `doc/host-protocol.md`（語の正本。第 0・2・8 章、付録 A・B）
- `shiori4_host/src/discover.rs`（`load_ghost`・`LoadedGhost`）、`shiori4_host/src/lib.rs`（`Host`・`GhostRuntime`・`LoadError`・`Completer`・`core_refers_to_protocol_only_by_its_reexport`）、`shiori4_host/src/test_dll.rs`（testghost の置き場所）
- `.kiro/specs/completed/abi-conventions/`・`.kiro/specs/completed/host-protocol/brief.md`（規約の理由の出典）
- `.kiro/specs/shiori3-legacy-host/brief.md`（試験用のホストの継ぎ目）

## 設計の議論 #1 の決定（2026-10-24）
- 正本の未決 3 つ（`load`・`unload` のスレッド、`release` の期間）と、`ShioriOwnedResponse` がライブラリより長く生きて外れた DLL の `release` を呼びうる不具合を、この spec の中で直すと開発者が裁定した（先送りは無駄、のため）。
- 検討した案: (1) `ShioriOwnedResponse` に lifetime を付ける、(2) 呼んだ側が書き込み口（`write` の callback）を渡す、(3) **ホストが `ShioriBytes` を作る（`copy_from_slice`）**。開発者の案 (3) を採った。関数の形を変えずにテーブルの末尾に 1 つ足すだけで、`release` が常にホストのコードになり、所有権の決まりが 1 通りになる。
- `Ghost` は `Send` を求めない（`shiori4/src/lib.rs`）ので、`load`・`unload` を別のスレッドで呼ぶと Rust の SHIORI で UB になる。スレッド規約 1 を広げる根拠。
- 写しが 1 回増える。別のプロセスのホスト（areka）向けの簡易版や写しを減らす工夫は保留し、問題になったら別の spec に切り出す。
