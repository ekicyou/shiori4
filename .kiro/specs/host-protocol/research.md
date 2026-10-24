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
  7. C ヘッダ・`shiori4_abi` の doc の「post の完了通知」の節から正本を指す 1 行の書き方（ABI の文は変えない。要件 1.5・11.2）。
  8. SEND が読む `Option` の語（`nobreak`・`notranslate`・`strict`）を便利関数の定数に入れるか（要件 10.3 は `Option` の名前だけを挙げる）。
  9. pasta・areka の要望（2026-10-24 の照合）:
     - 便利関数は `ShioriHostHandle` を持たず、文字列を受けて文字列を返す形にする（例 `getproperty_request(name)`・`send_post(script, options, tag: Option<&str>)`）。`Properties`・`PendingTags` はその上に載せる。pasta は要求をアクタースレッドで組み立てて中継するため。
     - 400 の読み分けと `OnHostComplete` の解析を型 1 つずつで持つ（例 `Rejection { Invalid | Forbidden | Unknown }`・`HostComplete { tag, status, command, outcome, rejection }`）。
     - 便利関数を `Ghost`／`Adapter` なしで使えること。依存はできれば `shiori4_abi` だけ（pasta）。v3 の変換（`parse_request`／`response`）か、その逆向きの変換を公開するか。
     - `OnHostComplete` の組み立て関数が、必須のヘッダ（`Sender`・`SenderType`・`SecurityLevel`・`SecurityOrigin`）を `ID` より前に置けること（ukadoc の「`SecurityLevel` は少なくとも ID より前」。areka）。
     - 0.0.1 → 0.0.2 の移行の一覧をリリースノートに置くこと、crates.io に出た版を両者に知らせること。
- **守ること**: `Bridge::request` の先頭の規約 2 の門、UTF-8 の検査、`Host::log`・`accept_post`・`wake` の「Drop するスレッドを同期で待たない」の doc、Charset を付けないことのテスト。

## 7. 設計フェーズ（2026-10-24）

### Summary
- **Discovery Scope**: Extension（light discovery）。既存の 2 クレートの公開 API の組み替えと、新しい文書・独立したモジュールの追加。新しい外部依存は無いので、外部の調査は ukadoc の参照先の確認だけにした。
- **Key Findings**:
  - ホスト側の便利関数（応答・`OnHostComplete` の組み立て）も `shiori4_host` の型を使わない純粋な文字列の関数にできる。Option C の「`shiori4_host` にも別の `pub mod` を置く」は要らず、`shiori4::host_protocol` の 1 つにまとめられる。
  - 組み込みの `property` が無くなると、`Host::request` の `Option`（None で組み込みへ回す）は意味を失う。戻り値を `ShioriResponse` にし、既定の 400 を trait の既定の実装で持てば足りる。
  - 芯から解析が抜けると `Completer` は `EventSender` の 1 回きりの包みになる。brief の語（「完了通知の芯は complete_with」）に合わせ、`complete_with` だけを残す。
  - v3 の私的な解析は Charset の 1 ヘッダを読むだけで足りる。`v3::parse_request` は原文のヘッダ部分（部分文字列）を返せる。

### Research Log

#### ukadoc の参照先
- **Context**: 要件 1.6（ukadoc を参照先として示す）と、areka の要望（必須ヘッダを ID より前に置けること）の根拠の確認。
- **Sources Consulted**: <https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html>、<https://ssp.shillest.net/ukadoc/manual/spec_sstp.html>
- **Findings**:
  - SHIORI/3.0 の頁は `SecurityLevel` を「少なくとも ID ヘッダより前に現れる」とする。`SenderType` の語は `internal`・`external`・`sakuraapi`・`embed`・`raise`・`property`・`plugin`・`sstp`・`communicate`（カンマ区切り）。`SecurityLevel` は `local`・`external`。`SecurityOrigin` は `null` か `<scheme>://<host>[:<port>]`。節のアンカーは `#request`・`#response`。
  - SSTP の頁は SEND・NOTIFY を `#method_send`・`#method_notify` に置き、Option の `nobreak`（今のスクリプトを中断せず終わるまで待つ）・`notranslate`・`nodescript`・`strict` を定める。
- **Implications**: 文書の付録 B にこの URL を置く。`host_complete` は必須ヘッダを `lead` として `ID` より前に置く形にする。

#### 既存コードの影響範囲（gap 分析の確認）
- **Context**: File Structure Plan を確定させるため。
- **Findings**: gap 分析の一覧どおり。追加で、`shiori4_host/src/v3.rs` のテスト用 Ghost 3 つと `testdata/testghost/src/lib.rs` が `Ghost` のシグネチャに依存する。`discover.rs`・`dist.rs`・`test_dll.rs` は影響しない。`shiori4_abi` は doc の 1 行だけ。
- **Implications**: 変更するファイルは `shiori4/src/lib.rs`・`shiori4/src/host_protocol.rs`（新規）・`shiori4_host/src/lib.rs`・`shiori4_host/src/v3.rs`（テストだけ）・`testdata/testghost/src/lib.rs`・C ヘッダと `shiori4_abi/src/lib.rs`（1 行）・README 3 つ・steering 2 つ・`doc/host-protocol.md`（新規）。

### Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A | `shiori4` の中に `pub mod` | クレートを増やさない | 独立はモジュールの境界だけ | 検査のテストで補う |
| B | 新しいクレート | 依存で独立を強制できる | 公開の仕組み（4 クレート目・初回の手動公開・版の置き場所）に波及 | pasta の「依存は shiori4_abi だけ」に最も近い |
| C | A ＋ `shiori4_host` にも `pub mod` | 利用者ごとに置ける | ホスト側の関数も `shiori4_host` の型を使わないので、分ける理由が無い | gap 分析の推奨 |
| **採用: A（C の簡約）** | `shiori4/src/host_protocol.rs` の 1 つ。`shiori4_host` は再エクスポート | 最小。境界はファイルで見え、テストで検査できる | pasta は `shiori4` ごと依存する（`shiori4` 自体は `shiori4_abi` と std だけ） | design.md の OPEN QUESTION 1 |

### Design Decisions

#### Decision: 便利関数は `shiori4::host_protocol` の 1 つのモジュール
- **Context**: 要件 10.1・9.3、本書 6 の 1・9（pasta: `Ghost`／`Adapter` なしで使いたい、依存はできれば `shiori4_abi` だけ）。
- **Alternatives Considered**: 新しいクレート（B）、両クレートに分ける（C）。
- **Selected Approach**: 名前は今の `host_protocol` を保ち、`shiori4/src/host_protocol.rs` に置く。`Headers`・`HeaderWriter`・`Properties`・`Lookup`・`PendingTags` もここへ移し、クレートの直下には再エクスポートしない。
- **Rationale**: 文書の名前（ホスト側プロトコル仕様）と対応し、steering の「層をまたぐ語彙は `shiori4`」に合う。`shiori4` は std と `shiori4_abi` だけに依存するので、pasta の依存の重さは新しいクレートとほぼ同じ。
- **Trade-offs**: 独立はコンパイラでは強制されない → `include_str!` の検査のテストで守る。
- **Follow-up**: 将来の別の語は別の名前のモジュールとして並べる。

#### Decision: 芯の trait は原文を受け、既定は 400 と全部受付
- **Context**: 要件 8.1〜8.3・8.6・8.8・9.1、要件ディスカッションの結果 2・3。
- **Selected Approach**: `Ghost::request(&mut self, &str)`、`Host::request(&self, &str) -> ShioriResponse`（既定 400）、`Host::accept_post(&self, &str) -> bool`（既定 true）。`Host::property` は削る。
- **Rationale**: 組み込みの応答が無くなるので `Option` は不要（簡約）。

#### Decision: 完了通知の芯は `Completer::complete_with` だけ
- **Context**: 要件 8.7・8.8・10.6、本書 6 の 3。
- **Alternatives Considered**: `Completer` を消して `Post` に `EventSender` を持たせる／`complete` に改名する。
- **Selected Approach**: `Completer` を残し、`complete_with(self, delivery, headers)` を「全文をそのまま届ける」にする。`complete`・`tag`・`is_wanted` を削る。`Tag` を引き継ぐのは便利関数 `host_complete`。
- **Rationale**: brief の語と一致し、利用者の移行が `into_completer()` を保ったまま済む。1 回きりの口であることが型で表れる。
- **Follow-up**: design.md の OPEN QUESTION 2。

#### Decision: 組み立ては文字列の関数、状態はその上に載せる
- **Context**: 本書 6 の 9（pasta のアクタースレッドでの組み立て）。
- **Selected Approach**: `getproperty_request`・`setproperty_request`・`send_post`・`notify_post`・`getproperty_response`・`rejection_response`・`host_complete` は `ShioriHostHandle` を持たない。`PendingTags::register` を足し、組み立てと post を別のスレッドで行えるようにする。`Properties::get` は `getproperty_request` を使う。
- **Rationale**: 1 つの組み立てを `Properties`・`PendingTags`・利用者の手組みで共有できる（一般化）。

#### Decision: 400 の読み分けと完了通知の解析は型 1 つずつ
- **Context**: 要件 7.3・6.2〜6.5、pasta の要望。
- **Selected Approach**: `Rejection { Invalid, Forbidden, Unknown }`（値を持たない Copy の enum）と `HostComplete { tag, status, command, outcome, rejection }`（借用）。`outcome` は `&str` のまま（知らない語も渡す。要件 10.9）。
- **Trade-offs**: `outcome` を enum にしないので、`completed`／`interrupted` の比べは利用者が定数で行う。

#### Decision: `OnHostComplete` の必須ヘッダは `lead` で受ける
- **Context**: areka の要望（`SecurityLevel` は ID より前）、要件 5.4。
- **Alternatives Considered**: 必須ヘッダの値を型（構造体）で受ける。
- **Selected Approach**: `host_complete(post, lead, status, refs, reason)` の `lead` は原文で ID より前に置く。`SENDER_TYPE_INTERNAL`・`SECURITY_LEVEL_LOCAL` を定数で出す。
- **Rationale**: `Sender`・`SecurityOrigin` の値はホストごとに決まり、1 度組み立てれば使い回せる。型を足さない（簡約）。design.md の OPEN QUESTION 6。

#### Decision: setproperty の送信は `Properties::set_on_host`
- **Context**: 要件 10.8（キャッシュへの取り込み `set` と紛れない別の関数）、本書 6 の 5。
- **Selected Approach**: `set_on_host(name, value) -> Result<(), SetPropertyError>`。204 でキャッシュも更新。`SetPropertyError { Offline, Rejected(Rejection), Failed(ShioriStatus) }`。
- **Follow-up**: design.md の OPEN QUESTION 3（名前）。

#### Decision: 文書は `doc/host-protocol.md`、移行の一覧はその付録
- **Context**: 本書 6 の 6・7、要件 1.5・11.1。
- **Selected Approach**: リポジトリの `doc/` に置き、クレートの doc・README からは GitHub の main の URL で指す。C ヘッダと `shiori4_abi` の `//!` の「post の完了通知（v4）」の末尾に 1 行。0.0.1 からの移行の表は文書の付録 A に置き、`/release` がリリースノートに写す。
- **Trade-offs**: main の URL は版に固定されない → 文書の第 0 章で当てはまる版を書く（要件 1.4）。design.md の OPEN QUESTION 4・7。

#### Decision: `Option` の語の定数を入れる
- **Context**: 本書 6 の 8。
- **Selected Approach**: `OPTION_NOBREAK`・`OPTION_NOTRANSLATE`・`OPTION_STRICT` を入れる（`nodescript` は読まない語なので入れない）。

### Synthesis
- **Generalization**: 「SHIORI 側の組み立て」「ホスト側の組み立て」は、どちらも `HeaderWriter` の上の数行の関数で、同じモジュールにまとめられる。`PendingTags::post` は `register` ＋ 組み立て ＋ `post_str` の特殊形。
- **Build vs. Adopt**: ヘッダの解析は SHIORI/3.0 の行の書式だけなので std で書く（既存の `Headers` をそのまま使う）。外部のパーサは採らない（tech.md の依存最小）。
- **Simplification**: Option C の 2 つ目のモジュールを落とした。`Host::request` の `Option` を落とした。`Completer` から解析の状態（`tag`・`command`）を落とした。v3 の逆向きの変換は足さない。

### Risks & Mitigations
- 規約 2 の門を書き替えで消す → 既存の門のテスト 2 本を残し、期待値だけを直す。
- 芯が便利関数を参照し直す（後の変更で）→ `include_str!` の検査のテスト。
- v3 の私的な Charset の読み取りが `Headers::get` と食い違う → v3 の Charset のテスト（大文字小文字・空白）を残す。
- 0.0.1 の API を広く動かす → 移行の一覧を文書の付録とリリースノートに置き、2 回目の連絡で知らせる。

### 参照した skill・ガイド
- `.claude/skills/kiro-spec-design/rules/`（design-principles・design-discovery-light・design-synthesis・design-review-gate）、`.kiro/settings/templates/specs/design.md`・`research.md`、steering の product・tech・structure・roadmap。完了済み spec `abi-conventions` の design.md を書式の手本にした。

## 8. 設計ディスカッション（2026-10-24）
- **便利関数の置き場所を Option B（新しいクレート `shiori4_helper`）に変えた**: 開発者の指摘「便利関数は SHIORI 側でもホスト側でも使う」から見直した。`shiori4::host_protocol` でも両側から使えはするが、芯が依存しないことをソースを読むテストで守るしかなく、pasta は `shiori4` ごと入れることになる。別のクレートなら依存の向きをコンパイラが守り、pasta は `shiori4_abi` と `shiori4_helper` だけで足りる。代わりに公開するクレートが 4 つになり、リリースの道具（`build.yml`・`release.yml`・`/release`・`release-ci-setup.md`・steering）の更新と、初回の手元からの公開・Trusted Publisher の登録が要る。0.0.2 を出す前で、この spec が API を一度に動かす場なので、足すなら今がいちばん安い。
