# ギャップ分析: host-protocol

## Summary
- **Feature**: `host-protocol`
- **Discovery Scope**: Extension（既存の 2 クレートの API を動かす。新しい文書と独立したモジュールを足す）
- **Key Findings**:
  - 語彙の文書（要件 1〜7）は新規に書くだけで、コードの土台は要らない。今の語の記述は `shiori4/src/lib.rs` の `host_protocol` の doc（数行）と、C ヘッダ・`shiori4_abi` の doc の「post の完了通知」の節だけ。
  - 芯が中身を解釈している所は、ホスト側で 4 か所（`Bridge::request` の振り分け・`Host::accept_post` の既定・`Post::new` の `Tag`／`Command` の読み取り・`Completer::complete`／`complete_with`）、SHIORI 側で 1 か所（`Adapter` が v4 の要求を `Headers::parse` して `Ghost` に渡す）ある。
  - いちばん大きい波及は `Headers` の移動。`Ghost::request`／`notify`・`Host::request`／`accept_post` のシグネチャが `Headers` を受けているので、「芯は便利関数のモジュールに依存しない」を文字どおり守ると、両 trait のシグネチャが原文（`&str`）に変わる（議題 4・3）。
  - abi-conventions の規約 2 の門（`Bridge::request` の先頭）はそのまま残せる。門の後ろの振り分けを `Host` への素通しに替えるだけ。
  - 影響するテストは `shiori4_host/src/lib.rs` に 7 本・`shiori4/src/lib.rs` に 2 種のテスト用 Ghost・`shiori4_host/src/v3.rs` と `testdata/testghost` の Ghost 実装。README にコード例は無い。

## 1. Current State Investigation

### 関係する資産
| 場所 | 中身 | この spec との関係 |
|------|------|--------------------|
| `shiori4/src/lib.rs` `Headers`・`HeaderWriter` | ヘッダ部分の解析（大文字小文字を区別しない `get`・`id`・`reference`・`iter`）と組み立て（`push` は CR/LF を除く・`push_raw`・`into_response`） | 便利関数へ移すもの（要件 10.2）。`Ghost`・`Adapter`・`v3`・`Properties`・`PendingTags`・`shiori4_host` 全体が使っている |
| `shiori4/src/lib.rs` `Ghost`・`Adapter` | `Ghost::request(&mut self, &Headers)`／`notify(&mut self, &Headers)`。`Adapter` は v4 では `Headers::parse(req)` して渡し、v3 では `v3::parse_request` で開始行と Charset を処理する | v4 の解析を外す（要件 9.1）。v3 の変換は残す（要件 9.2） |
| `shiori4/src/lib.rs` `mod v3` | `parse_request`（開始行・Charset 検査、`Headers` を返す）・`response`（全文の組み立て、Charset を補う） | インターフェースの変換なので芯に残る。ただし `Headers` を返すので、`Headers` を移すと依存の向きが問題になる |
| `shiori4/src/lib.rs` `host_protocol` | `ID_PROPERTY = "property"`・`HEADER_VALUE`・`HEADER_COMMAND`・`HEADER_TAG`・`ID_HOST_COMPLETE`・`HEADER_STATUS` と「暫定」の doc | 改名・追加して便利関数へ（要件 10.3） |
| `shiori4/src/lib.rs` `Properties`・`Lookup` | v4 は `ID: property` を `host.request_str` で問い合わせ、200/204/その他でキャッシュを更新。v3 は受動キャッシュ。`set` はキャッシュへの取り込み | `getproperty` へ。置き場所は議題 5 |
| `shiori4/src/lib.rs` `PendingTags` | `Tag` を発行・登録してから `poster.post_str("Tag: n\r\n{headers}")`、`take_from(&Headers)` | そのまま便利関数へ移せる（要件 10.7） |
| `shiori4_host/src/lib.rs` `Host` | `property(&self,&str)->Option<String>`・`request(&self,&Headers)->Option<ShioriResponse>`・`accept_post(&self,&Headers)->bool`（既定は `Command` の有無）・`on_post`・`log`・`wake` | `property` を撤去、`request` を素通しに、`accept_post` の既定を true に（要件 8.1〜8.3・8.6） |
| `shiori4_host/src/lib.rs` `Bridge` | 先頭に規約 2 の門 → UTF-8 検査 → `Host::request` → 組み込みの `property` → それ以外 400。post は shutdown → UTF-8 → `accept_post` → キュー → `wake` | 門と UTF-8 検査を残し、振り分けを外す（要件 8.1・8.4・8.5） |
| `shiori4_host/src/lib.rs` `Post`・`Completer` | `Post::new` が `Tag` と `Command` を解析して保持。`command()`・`tag()`・`wants_result()`・`headers()`。`Completer::complete` が `OnHostComplete` を組み立て、`complete_with` が `Tag` を先頭に足す。どちらも `Tag` が無ければ送らない | 解析と組み立てを外す（要件 8.7・8.8・議題 3） |
| `shiori4_host/src/lib.rs` `Event` | `parsed()`・`id()` が `Headers::parse` を呼ぶ | 解析つきメソッド（議題 3） |
| `shiori4_host/src/lib.rs` の再エクスポート | `pub use shiori4::{host_protocol, HeaderWriter, Headers}` | 便利関数のモジュールの再エクスポートに替える |
| C ヘッダ・`shiori4_abi/src/lib.rs` の「post の完了通知」 | `Tag` ヘッダの発行・登録、post の戻り値、完了は通常のイベント、`Tag` 無しは結果不要 | ABI の文は変えない（要件 11.2）。語彙の正本を指す 1 行（議題 6） |
| `doc/` | `release-notes/` だけ | 仕様の文書の置き場の候補（議題 8） |

### 守るべき規約・パターン
- 依存は下向きだけ（`shiori4_host` → `shiori4` → `shiori4_abi`）。層をまたぐ語彙は `shiori4` に置き、ホスト側は再エクスポートする（structure.md）。
- 外部依存を増やさない（tech.md）。便利関数のモジュールも std だけで書ける。
- テストは同じファイルの `mod tests`。ホスト側は `GhostRuntime::from_static` でテスト用 `Ghost` を直結する。
- 規約・方針はクレート冒頭の `//!` に書き、README は入口だけ。
- 0.0.x の版要求は完全一致。版はルート `Cargo.toml` の 3 か所だけ（この spec では上げない）。

## 2. Requirements Feasibility Analysis

### 要件と資産の対応
| 要件 | 必要なもの | 今あるもの | ギャップ |
|------|-----------|-----------|---------|
| 1 文書と正本 | 1 つの文書・各 doc/README からの導線 | `host_protocol` の doc（暫定） | **Missing**: 文書そのもの。**Constraint**: 置き場所は設計（議題 8） |
| 2〜7 語彙 | 文書の本文 | brief.md の論点 1〜12 | **Missing**: 本文の執筆のみ。**Unknown**: `Status` の意味の重なり（議題 1） |
| 8.1〜8.3 `Bridge` の素通し | `Host` が原文を受ける口と既定の応答 | `Host::request(&Headers)->Option` と組み込みの `property` | **Missing**: 原文を受ける口。**Unknown**: 既定の応答（議題 2） |
| 8.4 規約 2 の門 | 門を先頭に残す | 既にある（`Bridge::request` の先頭） | なし（書き替えで消さないこと） |
| 8.5 UTF-8 | 非 UTF-8 は 400 | 既にある | なし |
| 8.6 `accept_post` の既定 | 既定 true | 既定は `Command` の有無 | **Missing**: 既定の変更。引数を `&Headers` から原文に変えるか（議題 3） |
| 8.7 完了通知の全文 | ヘッダの全文を届ける口 | `complete_with` は `Tag` を自動で足し、`Tag` が無いと送らない | **Missing**: 何も足さない口。**Constraint**: `Tag` を誰が足すか（議題 3） |
| 8.8 芯の型 | 原文だけを渡す `Post`・`Event`・`Completer` | 解析つきのメソッドが多数 | **Missing**: 撤去と便利関数への移し替え（議題 3） |
| 8.9 キュー・pump・unload | 振る舞いを変えない | 既にある | なし |
| 9.1 SHIORI 側の芯 | `Ghost` に原文を渡す | `Adapter` が `Headers::parse` して渡す | **Missing**: シグネチャの変更（議題 4） |
| 9.2 v3 の変換 | 開始行・Charset の変換を残す | `mod v3` | **Constraint**: `v3::parse_request` が `Headers` を返すので、`Headers` を移すと芯→便利関数の依存になる。芯の中に私的な最小の解析を持つか、v3 の出力も原文にするかを設計で決める |
| 10.2 解析と組み立て | `Headers`・`HeaderWriter` | 既にある | 移動だけ |
| 10.3 定数 | 新しい語の定数 | 6 個 | **Missing**: `getproperty`・`setproperty`・`SEND`・`NOTIFY`・`Script`・`Event`・`Option`・`Reason`・`forbidden`・`completed`・`interrupted`・必須ヘッダ名 |
| 10.4 SHIORI 側の組み立て | get/set 要求・SEND/NOTIFY の post | 無い（`Properties` が要求を手で組み立てる） | **Missing** |
| 10.5 ホスト側の応答 | 200＋Value・204・400・400＋Reason | `Bridge` の中に 200＋Value・204・400 がある | **Missing**: 関数として外に出すこと・`Reason` |
| 10.6 OnHostComplete の組み立て | post の原文から `Tag` とコマンド名を引き継ぐ | `Completer::complete` | 移し替え |
| 10.7 `PendingTags` | 今の使い方 | 既にある | 移動だけ（`take_from` が `Headers` を受ける） |
| 10.8 `Properties` | `getproperty` を使う取得 | 既にある（`property`） | 改名と置き場所（議題 5） |
| 10.10・11.4 Charset | 芯と便利関数が Charset を付けない | `builtin_responses_have_no_charset`・`completion_headers_have_no_charset` | **Constraint**: 組み込みの応答が無くなるので前者を作り直す |
| 11.6 連絡 | pasta・areka への連絡 | なし | **Constraint**: エージェントは送らない。開発者（またはコントローラ）が行う作業（議題 9） |
| 11.7 steering | `tech.md`・`structure.md` の記述 | 「`ID: property`」「`host_protocol` は `shiori4` に置く」 | **Missing**: 完了時の同期（kiro-complete の段でもよい） |

### 影響するテストとコード
- `shiori4_host/src/lib.rs` のテスト:
  - `property_post_and_completion`: `Host::property`・`post.command()`・`wants_result()`・`complete` を使う。
  - `unknown_host_request_is_400`: 「Command の無い post は 400」を確かめている。要件 8.6 で逆になる。
  - `builtin_responses_have_no_charset`: 組み込みの応答が前提。作り直す。
  - `completion_headers_have_no_charset`: `complete`・`complete_with` が前提。
  - `request_from_other_thread_is_wrong_thread`: `calls == 2`（`Host::request` と `property` の 2 回）を数えている。1 回になる。
  - `property_inside_load_request_notify`・`two_runtimes_on_two_threads`: `Properties::get` → `property` の往復が前提。`CountHost::property` を `Host::request` で答える形に替える。
  - crate doc の例（`fn property`・`done.complete(..)`）も直す（doctest で検査される）。
- `shiori4/src/lib.rs` のテスト用 `Echo`・`Counter`（`Ghost` を `&Headers` で実装、`Counter` は `post_str("Command: test")`）と crate doc の例（`props.get`・`HeaderWriter`）。
- `shiori4_host/src/v3.rs` のテスト用 Ghost、`testdata/testghost/src/lib.rs`（`Headers`・`HeaderWriter` を使う）。
- 申し送りの要件: 規約 2 の門は `Bridge::request` の先頭に残す。`Host::log`・`accept_post`・`wake` は Drop するスレッドを同期で待たない（doc を残す）。

### 複雑さの性質
- 語彙の文書: 執筆だけ。
- 芯の変更: 振る舞いはむしろ減る（素通し）。難しいのは型の置き場所と依存の向きで、アルゴリズムは無い。
- 便利関数: 小さな純関数の集まり（文字列の組み立てと解析）。

## 3. Implementation Approach Options

### Option A: `shiori4` の中に便利関数のモジュールを作り、移して足す（既存の拡張）
- `shiori4` に `pub mod`（例: `shiori4::proto`／`shiori4::sstp_like` など。名前は設計）を作り、`Headers`・`HeaderWriter`・定数・`PendingTags`・`Properties`・組み立て関数を移す。`shiori4_host` はそれを再エクスポートする（今の再エクスポートの形を保てる）。
- 「芯はこのモジュールに依存しない」は、`Ghost`・`Adapter`・`Host`・`Bridge`・`Post`・`Event`・`Completer` がモジュールの型をシグネチャに出さないことで守る。v3 の変換は芯の中の私的な最小の解析で行う。
- **Trade-offs**:
  - ✅ クレートを増やさない。structure.md の「層をまたぐ語彙は `shiori4` に置く」に合う。公開の段取り（3 クレート・依存順）が変わらない。
  - ✅ 利用者は今と同じく 1 クレートを use すれば足りる。
  - ❌ 「独立」は同じクレートの中のモジュールの境界だけで、コンパイラは依存を止めない（テストか lint で守るしかない）。
  - ❌ v3 の変換に私的な解析が残り、`Headers` と似たコードが 2 つになる。

### Option B: 便利関数のための新しいクレートを作る（新規）
- 例: `shiori4_proto`（名前は設計）を足し、`shiori4`（の型 `ShioriHostPoster` など）に依存させる。`shiori4`・`shiori4_host` の芯はこのクレートに依存しない。
- **Trade-offs**:
  - ✅ 「芯はモジュールに依存しない」をクレートの依存で強制できる。将来の JSON-RPC 用の別クレートを並べやすい（brief の「別のモジュールを並べて足す」）。
  - ❌ 公開するクレートが 4 つになる。リリース CI・`/release`・`deny.toml`・`cargo package` の検査・版の置き場所（ルート `Cargo.toml` の 3 か所 → 4 か所）・README の表に波及する。初回は Trusted Publishing で新規クレートを作れず、手元からの初回公開が要る（roadmap の Constraints）。
  - ❌ `shiori4_host` が便利関数を再エクスポートするなら、結局 `shiori4_host` が依存することになる（芯のコードが使わなければ方針には反しない）。

### Option C: ハイブリッド（推奨の起点）
- 便利関数は Option A のとおり `shiori4` の独立した `pub mod` に置き、`shiori4_host` 側の組み立て（`OnHostComplete`・プロパティの応答）は `shiori4_host` の独立した `pub mod` に置く。どちらも芯の型をシグネチャに出さない。
- 芯の trait は原文（`&str`）を受ける形に変え、解析つきの型は便利関数のモジュールにだけ置く。
- 依存の向きはテスト（芯のファイルが便利関数のモジュールを `use` していないことの確認）か、モジュールを別ファイルにして目で見える形で守る。
- **Trade-offs**:
  - ✅ クレートを増やさずに、SHIORI 側の語とホスト側の語をそれぞれの利用者のクレートに置ける。
  - ✅ 将来の別の語は、同じ形の `pub mod` を並べて足せる。
  - ❌ 語の定数は両側で共有するので、`shiori4` 側に置いて `shiori4_host` 側が使う形になる（今の `host_protocol` と同じ）。

## 4. Research Needed（設計へ持ち越す）
- **`Ghost`／`Host` の trait のシグネチャ**: 原文 `&str` を受ける形に変えたときの、v3 の変換（`v3::parse_request` が `Headers` を返す）との兼ね合い。芯に私的な解析を残すか、v3 の変換も原文を返すか。
- **完了通知の口の形**: `Completer` を残して `complete_with` を「全文をそのまま送る」に変えるか、`Post` が `EventSender` を渡すだけにするか。`Tag` の無い post の扱い（今は送らない）。
- **既定の応答**: `Host::request` の既定（議題 2）と、それが `Properties` の 400 の扱い（キャッシュで代替）にどう見えるか。
- **独立の守り方**: 同じクレート内のモジュールの依存をどう検査するか（テスト・ファイルの分け方・lint）。
- **文書の置き場所と crates.io での見え方**: `doc/host-protocol.md` はパッケージに入らないので、docs.rs からは GitHub の URL で指すことになる。各クレートの `include` の設定と合わせて確かめる。
- **ukadoc の参照**: 文書から指す ukadoc の頁（SHIORI/3.0・SSTP・プロパティシステム・`OnBalloonBreak`）の URL。

## 5. Implementation Complexity & Risk
- **Effort: M（3〜7 日）**: 文書の執筆（論点 1〜12）と、2 クレートの公開 API の組み替え（trait のシグネチャ・型の移動）・約 10 本のテストと doctest の書き直し。新しいアルゴリズムは無い。
- **Risk: Medium**: 技術は既知だが、0.0.1 の公開 API を広く動かす。`Ghost`／`Host` のシグネチャの変更は pasta・areka の着手の前提を変えるので、要件ディスカッションと設計で 3 者の語と合わせる必要がある。abi-conventions の規約 2 の門を消さないことが唯一の回帰の急所。

## 6. Recommendations for Design Phase
- **推奨の起点**: Option C（クレートを増やさず、`shiori4` と `shiori4_host` にそれぞれ独立した `pub mod` を置く）。理由: 方針（芯は中身を解釈しない）を守りつつ、公開の仕組みに波及しない。将来の別の語は `pub mod` を並べて足せる。
- **決めること**:
  1. 便利関数のモジュールの名前と、`shiori4`／`shiori4_host` への割り振り。
  2. `Ghost`・`Host` の trait のシグネチャ（原文を受けるか）と、v3 の変換が使う私的な解析。
  3. 完了通知の芯の口（`Completer` を残すか）と、`Tag` を足す責任。
  4. `Host::request` の既定の応答。
  5. `Properties` の置き場所と、`setproperty` の関数の名前（`Properties::set` との紛れ）。
  6. 文書の置き場所と、crate doc・README・C ヘッダからの導線。
- **守ること**: `Bridge::request` の先頭の規約 2 の門、UTF-8 の検査、`Host::log`・`accept_post`・`wake` の「Drop するスレッドを同期で待たない」の doc、Charset を付けないことのテスト。
