# ギャップ分析: shiori2-helpers

## Summary
- **Feature**: `shiori2-helpers`
- **Discovery Scope**: Extension（`shiori4_host` に、芯から独立した文字列の便利関数のモジュール `shiori2` を 1 つ足す。依存・ABI・芯の口は変えない）
- **Key Findings**:
  - 新しく書くのはほぼ全部で、使い回せるのはヘッダの読み分け（`shiori4::protocol::Headers`）と組み立て（`HeaderWriter`）だけ。開始行（`SHIORI/<版> <ステータス>`）を読む公開の関数は今どこにも無い。似たものは `shiori4::v3::parse_request`（SHIORI/3.0 の要求の開始行だけを受け、SHIORI/2.x は 400）と、試験用のホストの非公開の `v3_status`（`SHIORI/3.0` だけを受ける）で、どちらも使えない。
  - `Headers::parse` は `:` を含まない行を飛ばすので、開始行ごと渡してもヘッダだけを読める。ただし**終端の空行で止まらない**（空行より後ろに `:` を含む行があれば読む）。要件 4.2 を守るには、渡す前に空行までを切り出す必要がある（`shiori4::v3::parse_request` に同じ切り出しがあるが非公開の作り）。
  - 一番大きな未決は版の見分けの手順。brief の決定 3 は「SHIORI/3.0 の要求を先に出す」が、SSP の実測は `GET Version SHIORI/2.6` を先に出して開始行の版で決める。SHIORI/2.x の DLL が SHIORI/3.0 の要求にどう答えるかは一次資料が無く、華和梨は辞書に無い出来事にも 400 を返すので、ステータスでは見分けられない（要件 6.5・7.1・Q1）。
  - 置き場所の継ぎ目はそろっている。`shiori4_host/src/lib.rs` の `mod` の並び（`v3`・`discover`・`dist`・`legacy`）に `pub mod shiori2;` を 1 行足し、`shiori4_host/src/shiori2.rs` に本体とテストを置けばよい。芯が便利関数を参照しないことの検査（`core_refers_to_protocol_only_by_its_reexport`）は `lib.rs` の `protocol` の語だけを数えるので、`shiori2.rs` の中で `shiori4::protocol::Headers` を使っても落ちない。
  - 文書の触る所は、crate doc の「DLL の選び方と読み方」の節（`LegacyRuntime` の項の近く）、`shiori4_host/README.md` の 17・21 行の近く、`doc/textbook/migration.md` の「v3 互換モードと旧来の DLL」の箇条。教科書の章は doctest に載る（`lib.rs` の `#[cfg(doctest)]`）ので、Rust の例を足すならビルドが通る形にする。

## 1. Current State Investigation

### 関係する資産
| 場所 | 中身 | この spec との関係 |
|------|------|--------------------|
| `shiori4/src/protocol.rs` の `Headers` | `parse(&str)`（CRLF/LF・`:` の無い行と空行を飛ばす・名前の前後と値の先頭の空白を除く）・`get`（大文字小文字を区別しない・最初のもの）・`id`・`reference(n)`・`iter` | 要件 4.1 のヘッダの読み分けにそのまま使う。終端の空行で止まらない（Constraint） |
| `shiori4/src/protocol.rs` の `HeaderWriter` | `push`（値の CR/LF を除く・`Name: Value\r\n`）・`push_raw`・`finish` | 要件 2.1・2.4 のヘッダの行の組み立てに使う。開始行と終端の空行は持たない（足すのは `shiori2`） |
| `shiori4/src/protocol.rs` の語の定数 | `HEADER_ID`・`HEADER_SENDER`・`HEADER_SECURITY_LEVEL`・`HEADER_VALUE`・`HEADER_EVENT`・`HEADER_STATUS` ほか | 名前が同じヘッダは使い回せる。`Sentence`・`Word`・`String`・`Type`・`Ghost`・`GhostEx`・`Charset`・`Craftman`・`Version` の定数は無い（Missing。`shiori2` の中に置くか、置かないかは設計） |
| `shiori4/src/lib.rs` の `pub mod v3` | `parse_request`（`GET SHIORI/3.0`／`NOTIFY SHIORI/3.0` だけ。SHIORI/2.x は 400・空行までの切り出し・Charset が UTF-8 以外は 400）・`response` | SHIORI の側の芯。変えない（要件 1.7）。空行までの切り出しの書き方は手本になる |
| `shiori4_host/src/lib.rs` | `pub use shiori4::protocol;`・`mod` の並び・crate doc の「DLL の選び方と読み方」 | `pub mod shiori2;` を足す所（要件 1.1）。crate doc の道案内（要件 8.4） |
| `shiori4_host/src/lib.rs` のテスト `core_refers_to_protocol_only_by_its_reexport` | `lib.rs` の `mod tests {` より前の行のうち `protocol` を語として含む行が `pub use shiori4::protocol;` だけであることを確かめる | `lib.rs` に `protocol` の語を書かなければ落ちない。`shiori2` を芯でなく便利関数として置くことと合う |
| `shiori4_host/src/legacy.rs` の `LegacyRuntime` | `request(&[u8]) -> Result<Vec<u8>, _>`・`request_str(&str) -> Result<String, _>`（x86 の Windows だけ） | 組み合わせる相手。変えない。バイト列の応答から版を読む要件 6.1 は、`request` の戻り値をそのまま渡せる形にすると使いやすい |
| `shiori4_host/examples/shiori4-testhost.rs` の `v3_status` | `SHIORI/3.0` の開始行からステータスを取る（非公開） | 試験用のホストは変えない（要件 8.3）。使い回さない |
| `doc/textbook/migration.md` | 23 行の表（版 2 の「SHIORI/2.x の要求には 400 を返す」）・「v3 互換モードと旧来の DLL」の箇条（`LegacyRuntime` の案内） | 要件 8.5 の一言を足す候補は箇条の方。表の版 2 の文は正本の写しで、`shiori4_abi/tests/textbook.rs` が写しの一致を検査する範囲に入りうるので変えない |
| `shiori4_host/README.md` | 17 行（`LegacyRuntime` の案内）・21 行（`protocol` の案内） | crates.io の顔。`shiori2` の一言を足すかは設計（要件 8.4 は crate doc だけを求める） |
| `.kiro/steering/tech.md`・`structure.md` | `shiori4_host` の中身の一覧（`shiori2` は無い） | 実装の後に steering を同期する（kiro-complete の段） |

### 慣習
- 便利関数は芯から独立し、芯は使わない（tech.md「芯は中身を解釈しない」）。`shiori2` もこれに従う。
- ファイル内は `// ====` の区切りで節に分け、テストは同じファイルの末尾の `#[cfg(test)] mod tests`。
- 関数の doc と注釈は日本語。組み立ては `String` を返し、読み分けは借用（`Headers<'a>`）で返すのが `protocol` の形。
- 依存は増やさない（`shiori4_host` は `libloading` だけ）。`shiori2` は std と `shiori4` だけで書ける。
- 公開の enum は `#[non_exhaustive]` を付けていない（`LoadError` など）。`shiori2` に版や失敗の enum を足すなら、後で値を足すときに網羅の match が壊れることを設計で考える。

## 2. Requirements Feasibility Analysis

### 要件から資産への対応
| 要件 | 要るもの | 今あるもの | ギャップ |
|------|----------|------------|----------|
| 1.1〜1.2 置き場所・全環境 | `pub mod shiori2;`（`cfg` なし） | `mod` の並び | Missing（1 行） |
| 1.3〜1.7 既存を変えない | 足すだけ | — | なし（Constraint を守るだけ） |
| 2.1〜2.6 汎用の組み立て | 開始行＋`HeaderWriter`＋終端の空行・開始行の語の検査 | `HeaderWriter` | Missing（開始行・空行・語の検査）。失敗の型（要件 2.5）も Missing |
| 3.1〜3.6 命令ごとの組み立て | 9 つの命令の関数・共通のヘッダの渡し方・`Reference` の番号・`GhostEx` の `\x01` | `notify_post` の `Reference{i}` の振り方が手本 | Missing。共通のヘッダの渡し方（引数か、並びか、小さな型か）は設計（Q4） |
| 4.1〜4.7 応答の読み分け | 開始行の解析（`SHIORI/<大>.<小> <数> [理由句]`）・空行までの切り出し・`Headers` | `Headers` | Missing（開始行の解析・切り出し・失敗の型） |
| 5.1〜5.5 決まったヘッダ | 命令ごとの取り出し・`Value` への代替 | `Headers::get` | Missing（薄い関数。Q5） |
| 6.1〜6.5 版の見分け | `&[u8]` の開始行を ASCII として読む・`&str` と同じ結果 | なし | Missing |
| 7.1〜7.7 切り替えの手順 | 最初の要求（SHIORI/3.0 か `GET Version SHIORI/2.6`）・応答から「3.0／2.x（小さい方の番号つき）／不明」 | なし | Missing。手順の既定（Q1）と形（Q7）が Unknown |
| 8.1〜8.7 テストと文書 | `mod tests`・crate doc・関数の doc・移行の手引きの一言 | 文書の置き場所 | Missing（書くだけ） |

### Research Needed（設計に持ち越す）
- **SHIORI/2.x の DLL が SHIORI/3.0 の要求に何を返すか**（Q1）。一次資料は無い。手元の華和梨 7（areka の検証で 400 を返したとある）と、ほかの SHIORI/2.x の DLL（偽林檎・文・美坂など）で、開始行の版が `SHIORI/2.x` か、応答が空か、落ちるかを確かめたい。決定 3 の手順を既定にするなら必須の調べ。SSP の手順（`GET Version SHIORI/2.6` を先に）を既定にするなら、SSP の実測で足りる。
- **SHIORI/3.0 の SHIORI が `GET Version SHIORI/2.6` に何を返すか**。里々は `SHIORI/3.0 200 OK` で答える（実測）。shiori4 の SHIORI の側（v3 互換モードと、v3 の変換の芯）は SHIORI/2.x の要求に 400 を返す（版 2）が、開始行は `SHIORI/3.0 400 Bad Request` になる（`shiori4::v3::response` は常に `SHIORI/3.0` を書く）ので、開始行の版で見分ける手順なら SHIORI/3.0 と読める。設計で確かめる。
- **SSP が `Value` を先に読むこと**（Q5）。根拠は ssp.exe の逆アセンブルの報告だけで、実機で確かめられていない。
- **areka の spec `shiori2-protocol` の範囲**。areka の側で何を持っていて、何をこの便利関数に乗り換えるか（下流の突き合わせ）。

### 複雑さの見立て
文字列の組み立てと解析だけ（algorithmic / 純粋な関数）。DLL・スレッド・FFI・外部の連携は無い。

## 3. Implementation Approach Options

### Option A: `shiori4_host/src/shiori2.rs` に新しいモジュールを 1 つ置く（brief の案 A）
- 足す所: `shiori4_host/src/lib.rs` に `pub mod shiori2;` と crate doc の数行、`shiori4_host/src/shiori2.rs` に本体とテスト。
- `shiori4::protocol::{Headers, HeaderWriter}` を `use` して使い回す。開始行・空行の切り出し・版の見分けは `shiori2.rs` の中に書く。
- ✅ 開発者が選んだ案。芯にも `shiori4` にも触れない。全環境で使える。
- ✅ テストを同じファイルに閉じられる。
- ❌ 空行までの切り出しが `shiori4::v3::parse_request` と二重になる（数行）。

### Option B: `shiori4::protocol` を広げて、開始行の読み分けを共通にする
- `shiori4::protocol` に開始行の汎用の解析（`SHIORI/<版> <数>`）と空行までの切り出しを足し、`shiori2` はその上に命令ごとの関数だけを置く。
- ✅ 開始行の解析を 1 か所にできる（SHIORI/3.0 の応答を読むホストも使える）。
- ❌ brief が却下した方向（SHIORI の側のクレートに、SHIORI の側で使わないものが載る）に近づく。`shiori4` の公開の API が増える。
- ❌ brief の Scope（In は `shiori4_host::shiori2` だけ）を広げる。

### Option C: Option A を基本に、版の見分けだけを独立した小さな関数にする（混成）
- `shiori2.rs` の中を節で 3 つ（組み立て・読み分け・版の見分けと切り替え）に分け、版の見分け（`&[u8]` と `&str`）は組み立てと読み分けに依存しない関数にする。読み分けの関数は、版の見分けの関数で開始行の版を読む。
- ✅ brief の Boundary Candidates（組み立て・読み分け・版の見分けと切り替え）と節がそのまま対応する。
- ✅ 版の見分けを `LegacyRuntime::request` のバイト列の戻り値にそのまま当てられる。
- ❌ Option A との差は節の分け方だけで、実質は同じ。

## 4. Implementation Complexity & Risk
- **Effort**: S（1〜3 日）。純粋な文字列の関数とテストだけで、既存の型（`Headers`・`HeaderWriter`）と節の分け方・テストの置き方の手本がそろっている。
- **Risk**: Low〜Medium。作りは Low（足すだけ・依存なし・芯に触れない）。Medium にしているのは手順の根拠で、SHIORI/2.x の DLL が SHIORI/3.0 の要求にどう答えるかが一次資料で決まらず、Q1 の決め方によっては実機での確かめが要る。

## 5. Recommendations for Design Phase
- **推す案**: Option C（= brief の案 A を、Boundary Candidates の 3 つの節に分けて書く）。
- **設計で決めること**:
  - 版の見分けの最初の要求の既定（Q1）。SSP と同じ `GET Version SHIORI/2.6` を既定にすると、根拠が実測でそろい、shiori4 の SHIORI の側（`SHIORI/3.0 400`）も開始行で SHIORI/3.0 と読める。
  - 版の表し方（SHIORI/3.0・SHIORI/2.x の小さい方の番号・不明）と、読み分けの失敗の型。`#[non_exhaustive]` にするか。
  - 命令ごとの関数に共通のヘッダ（`Charset`・`Sender`・`SecurityLevel`）をどう渡すか（Q4）。
  - 応答の読み分けの戻り値の形（借用の `Headers<'a>` を持つ型にするか）。空行までの切り出しを `Headers::parse` の前に置く。
  - ヘッダの名前の定数（`Sentence`・`Word`・`String`・`Type`・`Ghost`・`GhostEx`・`Craftman`・`Version`）を `shiori2` に置くか。置くなら structure.md の命名（`HEADER_*`）に合わせる。
- **持ち越す調べ**: 上の「Research Needed」の 4 つ。

---

## SHIORI/2.x の一次資料の調べ（要件の段）
brief の Constraints（「SHIORI/2.x の一次資料が伺かの文書の索引に無い。要件の段で一次資料と突き合わせる」）に答えるために調べたもの。要件の前提（requirements.md の「調べものから分かったこと」）の根拠。

### 資料
- **[P1] 一次資料**: 伺かの SHIORI の仕様書 <http://usada.sakura.vg/contents/shiori.html>（Shift_JIS。平文の HTTP でしか読めない）。節は 2.0・2.1（廃止。HTML の注釈の中）・2.2・2.3b・2.4・2.5・セキュリティ・ステータスコード。**2.6 の節は無い**。ukadoc の SHIORI/3.0 の頁（<https://ukatech.shillest.net/ukadoc-spec/spec/shiori3/>）が SHIORI/2.x の参照としてこの頁を指す。
- **[P2]** 同じ作者の覚え書き <http://usada.sakura.vg/contents/specification2.html> の「SHIORI プロトコル」（一般の形）。
- **[I1]** 華和梨 8 のソース（`kawari_shiori.cpp`）と文書（`users.html`）。
- **[I2]** 里々の文書 <https://ukatech.github.io/satori-docs/shiori/protocol/>。
- **[I3]** ukadoc（311・312 を「SHIORI/2系のTEACHメソッド」と書く。OnTranslate）。
- **[O1]** SSP と里々の実際の記録（`ekicyou/pasta` の `crates/pasta_shiori/doc/shiori-sample.log`）。
- **[O2]** areka の調べ（`ekicyou/areka` の `.kiro/specs/completed/shiori2-protocol/research.md`）: §10 は SSP 2.9.23 と華和梨 7.4 の実測（2026-10-23）、§4.2 は SSP 2.6.07 と華和梨 7.3.1（SSP の BTS #503 から）。
- **[D1]** ssp.exe 2.9.10 の `SPShiori` の逆アセンブル（`Heyuri/Sumika` の `docs/research/kawari7-wasm.md` §6）。作者は実機で確かめていないと書く。

### 要求の開始行と命令（[P1]）
| 版 | 開始行 | 命令に固有のヘッダ |
|----|--------|--------------------|
| 2.0 | `GET Version SHIORI/2.0` | なし |
| 2.0 | `NOTIFY OwnerGhostName SHIORI/2.0` | `Ghost`（2.0 の NOTIFY はこれだけ） |
| 2.0 | `GET Sentence SHIORI/2.0` | `Sentence`（ユーザーの言葉） |
| 2.0 | `GET Word SHIORI/2.0` | `Type`（`\ms` `\mz` `\ml` `\mc` `\mh` `\mt` `\me` `\mp` `\m?` `\dms`） |
| 2.0 | `GET Status SHIORI/2.0` | なし |
| 2.1（廃止） | `COMMUNICATE Sentence SHIORI/2.1` | 310 Communicate・`TargetHWnd`・`Age` |
| 2.2 | `GET Sentence SHIORI/2.2`（例の綴りは `Get`） | `Event`・`Reference0`〜`Reference7` |
| 2.3b | `NOTIFY OwnerGhostName SHIORI/2.3`・`NOTIFY OtherGhostName SHIORI/2.3` | `Ghost`・`GhostEx`（ほかのゴーストごとに 1 行。`名前\x01本体のサーフィス\x01相方のサーフィス`） |
| 2.3b | `GET Sentence SHIORI/2.3`（ゴーストどうしの会話） | `Sender`・`Sentence`・`Age`・`Surface`・`Reference0`〜 |
| 2.4 | `TEACH SHIORI/2.4`（命令の語が無い） | `Word`・`Reference0`〜（増えていく） |
| 2.5 | `GET String SHIORI/2.5` | `ID`（`homeurl`・`sakura.recommendsites` など） |
| 2.6（一次資料なし） | `TRANSLATE Sentence SHIORI/2.6` | `Sentence`（[O2]・[I1]・[D1] で確かめた） |

- 共通の要求のヘッダ: `Sender`・`Charset`・`SecurityLevel`（`local`／`external`。2.2 から必ず付ける）。
- 応答のヘッダ: `Sender`・`BalloonOffset`・`Sentence`・`Word`・`Status`（`neuron,neuronm,neuronk,neurond,neurone,synapse`）・`String`、2.3 から `To`・`Age`・`Reference0`〜・`Surface`。
- 開始行: `SHIORI/2.x <数> <理由句>`。ステータス: 200・204・310（廃止）・311 Not Enough・312 Advice・400・500。
- 改行は CRLF、2 つの CRLF で終わる。ヘッダの数と順は決まりが無い（[P2]）。
- `Charset` の既定は一次資料に書かれていない。例はどれも `Shift_JIS`。

### SSP が SHIORI/2.x の SHIORI に送るもの（[O2 §10.2]・[D1] と一致）
| 要求 | 開始行 | ヘッダの順 |
|------|--------|-----------|
| 出来事（SHIORI/3.0 の NOTIFY の出来事も） | `GET Sentence SHIORI/2.2` | `Charset`・`SecurityLevel`・`Sender`・`Event`・[`BaseEvent`]・[`Status`]・`Reference*` |
| 主のゴーストの名前 | `NOTIFY OwnerGhostName SHIORI/2.3` | `Charset`・`Sender`・`Ghost` |
| ほかのゴーストの名前 | `NOTIFY OtherGhostName SHIORI/2.3` | `GhostEx` の行 |
| 資源 | `GET String SHIORI/2.5` | `Charset`・`SecurityLevel`・`SecurityOrigin: null`・`Sender`・`ID`・[`Status`] |
| 変換 | `TRANSLATE Sentence SHIORI/2.6` | `Charset`・`SecurityLevel`・`SecurityOrigin: null`・`Sender`・`Status`・`Sentence` |

- SHIORI/2.x には出来事の NOTIFY が無いので、SSP は `GET Sentence` で送って応答を捨てる。`basewareversion`・`capability` は送らない。
- [D1] だけ: `TEACH SHIORI/2.4`・`GET Word SHIORI/2.0`・ユーザーの言葉の `GET Sentence SHIORI/2.0`・ゴーストどうしの会話の `GET Sentence SHIORI/2.3`。応答からは `Value` を先に、無ければ `Sentence`（資源は `Value`／`String`／`Sentence`、語は `Value`／`Word`）を、200・311・312 のときだけ読む。

### 版の見分け（[O1]・[O2]）
- SSP は読み込みの直後に `GET Version SHIORI/2.6`・`Charset: UTF-8`・`Sender: SSP` を 1 回送り、応答の**開始行の版**で決める。
  - `SHIORI/3.0 …`（204 でも）なら SHIORI/3.0。その後 `GET SHIORI/3.0` の `ID: version` を送る。里々は `SHIORI/3.0 200 OK` で答える。
  - `SHIORI/2.6 200 OK` なら SHIORI/2.x に切り替え、SHIORI/3.0 の要求は送らない。
  - 応答の `Charset` を、この後の要求の文字コードにする。
- [D1] だけ: 版は `atof×10+0.5`（26）で数え、4xx か 3.0 以上のときだけ SHIORI/3.0 を試す。
- `GET Version` の応答は一次資料では開始行だけだが、実際は `ID`・`Craftman`・`Version` を付ける（華和梨: `SHIORI/2.6 200 OK`・`Charset: Shift_JIS`・`Craftman: KawariDevelopmentTeam`・`ID: KAWARI.kdt`・`Version: 7.4.0`。里々も同じ 3 つ）。
- SHIORI/2.x の DLL が SHIORI/3.0 の要求に何を返すかの一次資料は無い。華和梨 8 はどの要求にも `SHIORI/3.0` の開始行で答える。華和梨 7 は areka の SHIORI/3.0 の要求に 400 を返した（[O2 §8]。ここでは確かめ直していない）。華和梨は辞書に無い出来事にも 400 を返すので（[O2 §4.2]）、400 は版の手がかりにならない。
- 推論: SHIORI/2.x には SHIORI ごとの 1 つの版が無い。要求はそれぞれ命令を足した版を名乗り、SHIORI は自分の最も高い版で答える（華和梨 7 はどれにも `SHIORI/2.6`）。

### 足りない所
2.6 の一次資料が無い。SHIORI/2.x の DLL が `GET SHIORI/3.0` にどう答えるかの一次資料が無い。SSP が送る `TEACH`・`GET Word`・会話の形は逆アセンブル（[D1]）だけで、記録では確かめていない。SSP の BTS #503 は [O2] を通して引いた。
