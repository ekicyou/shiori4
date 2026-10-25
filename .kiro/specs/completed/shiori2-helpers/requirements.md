# Requirements Document

## Project Description (Input)
従来の SHIORI の DLL には、SHIORI/3.0 ではなく SHIORI/2.x で話すものがまだ残っている。`load`・`unload`・`request` の HGLOBAL の口は SHIORI/3.0 と同じで、違うのは `request` に流れる文字列の形だけである。

ベースウェアの作者が SHIORI/2.x の DLL と話したくても、今の shiori4 には SHIORI/2.x の要求の組み立ても、応答の読み分けも無い。版の見分け（SHIORI/3.0 の要求を出し、応答が SHIORI/2.x だったら SHIORI/2.x の要求に切り替える）も、ホストごとに自分で書いている。`shiori4::protocol`（`shiori4_host::protocol` として再エクスポート）は SHIORI/3.0 の形だけを扱い、shiori3-legacy-host の芯（`LegacyRuntime`）は中身を解釈せずにバイト列を運ぶので、SHIORI/2.x の要求もそのまま往復できる。足りないのは文字列の形の側だけである。

この spec では、ホストの作者が SHIORI/2.x の要求を組み立て、応答を読み分け、版を見分けて切り替える便利関数を、`shiori4_host` の新しいモジュール `shiori2` から使えるようにする（brief の案 A）。芯の口（v4・v3 互換モード・従来の DLL の口）は SHIORI/2.x を知らないままにし、SHIORI の側（版 2 の「SHIORI/2.x の要求には 400 を返す」）も変えない。

## Introduction
この spec は、ベースウェアの作者が SHIORI/2.x の DLL と話すための文字列の便利関数を `shiori4_host::shiori2` に足す。持つのは、要求の組み立て（汎用の形と命令ごと）、応答の読み分け（開始行・ヘッダ・命令ごとの決まったヘッダ）、版の見分けと切り替えの手順の 3 つで、どれも文字列（版の見分けだけはバイト列も）を受けて返すだけの関数である。DLL の読み込み・HGLOBAL の受け渡し・文字コードの変換・SHIORI/3.0 の出来事から SHIORI/2.x の命令への移し方は持たない。

要件の根拠は brief.md の「決まっていること」1〜6 と Constraints で、各要件の目的の末尾に「（決定 N）」で示す。brief が一次資料と突き合わせるよう求めていた所（命令の一覧・決まったヘッダ・版の番号・`GET Version`・版の見分けの手がかり）は、要件の段の調べもの（research.md の「SHIORI/2.x の一次資料の調べ」）に基づく。brief で決め切れなかった所は、要件ディスカッション（2026-10-25）で決めた（末尾の「要件ディスカッションで決めたこと」）。

### 調べものから分かったこと（要件の前提）
- 要求の開始行は `<メソッド> <命令> SHIORI/<版>`（例: `GET Sentence SHIORI/2.2`）。`TEACH SHIORI/2.4` のように命令の語が無いものもある。改行は CRLF、空行で終わる（伺かの SHIORI の仕様書。以下「一次資料」）。
- 応答の開始行は `SHIORI/<版> <ステータス> <理由句>`（例: `SHIORI/2.6 200 OK`）。ステータスは 200・204・311 Not Enough・312 Advice・400・500（一次資料）。
- 命令ごとに、それを足した版の番号を開始行に書く（`GET Word`・`GET Status` は 2.0、出来事の `GET Sentence` は 2.2、`NOTIFY OwnerGhostName`・`NOTIFY OtherGhostName` は 2.3、`TEACH` は 2.4、`GET String` は 2.5、`TRANSLATE Sentence` は 2.6）。2.6 の一次資料は無く、`TRANSLATE` は SSP の記録と華和梨の文書だけで確かめた。SHIORI の側は自分の最も高い版で答える（華和梨 7 はどの要求にも `SHIORI/2.6` で答える）。
- SSP は読み込みの直後に `GET Version SHIORI/2.6` を 1 回送り、応答の**開始行の版**で SHIORI/3.0 と SHIORI/2.x を見分ける（実測）。brief の決定 3 の元の手順（SHIORI/3.0 の要求を先に出す）とは順が違うので、SSP の順を既定に改めた（議題 1）。
- SHIORI/2.x の DLL が SHIORI/3.0 の要求にどう答えるかの一次資料は無い。華和梨は SHIORI/2.x の要求でも、辞書に無い出来事には 400 を返すので、「400 なら SHIORI/2.x」とは読めない（areka の実測）。

## Boundary Context
- **In scope**:
  - `shiori4_host::shiori2`: SHIORI/2.x の要求の組み立て（汎用の形と命令ごと）（決定 2）。
  - 応答の読み分け: 開始行（版・ステータス）・ヘッダ・命令ごとの決まったヘッダの取り出し（決定 2）。
  - 版の見分けと切り替えの手順（文字列とバイト列の両方から版を読む）（決定 3）。
  - それぞれのテスト（同じファイルの `mod tests`）（決定 4）。
  - 文書（crate doc の道案内・関数の doc・必要なら教科書の移行の手引きへの一言）。
- **Out of scope**:
  - SHIORI の側での SHIORI/2.x への対応。版 2 の「SHIORI/2.x の要求には 400 を返す」は変えない（決定 1）。
  - SHIORI/3.0 の出来事から SHIORI/2.x の命令への移し方（例: `ID: OnBoot` → `GET Sentence` ＋ `Event: OnBoot`）。どの出来事をどの命令に移すかはホストが持つ（決定 2）。
  - 文字コードの変換（Shift_JIS ほか）と、Charset の方針（どの文字コードで送るか・応答の Charset に合わせて切り替えるか）。ホストが持つ（決定 6）。
  - 試験用のホスト（`shiori4-testhost`）の変更（決定 4）。
  - HGLOBAL の受け渡しと DLL の読み込み（shiori3-legacy-host が持つ）。
  - SAORI・MAKOTO など、SHIORI/2.x 以外の形。
  - 廃止された SHIORI/2.1 の `COMMUNICATE` と 310 Communicate（一次資料が廃止と書いているので持たない）。
  - `TEACH` の 311・312 を受けて問い直す繰り返し（ユーザーへの問い合わせを含む）。便利関数は 1 回分の要求の組み立てと、応答のステータスの読み分けだけを持つ（議題 5）。
  - ユーザーの言葉の `GET Sentence SHIORI/2.0` と、ゴーストどうしの会話の `GET Sentence SHIORI/2.3` の命令ごとの便利関数。汎用の形（要件 2）で作る（議題 2）。
  - 版を上げること（`/release` が上げる）。
- **Adjacent expectations**:
  - host-protocol（`shiori4::protocol`）: ヘッダの読み分けは `shiori4::protocol::Headers` と同じ規則（名前の大文字小文字を区別しない・最初の `:` で分ける・同名なら最初のもの・CRLF と LF のどちらも読む）に従い、組み立ては `HeaderWriter` と同じ規則（値の CR・LF を取り除く）に従う。`shiori4::protocol` の既存の型と関数は変えない。
  - shiori3-legacy-host: 従来の DLL の口（`LegacyRuntime`）はバイト列を中身を解釈せずに運ぶ。この spec はそれに依存せず、ホストが組み合わせて使う（便利関数が作った要求を、ホストが文字コードを変えて `LegacyRuntime::request` に渡す）。`LegacyRuntime` は変えない。
  - 下流: areka（SHIORI/2.x の DLL と話す所をこの便利関数に乗り換えられる。areka の spec `shiori2-protocol` の範囲と突き合わせる）、ほかのベースウェアの作者。

## Requirements

### Requirement 1: 置き場所・使える環境・既存の API
**Objective:** As a ベースウェア実装者, I want SHIORI/2.x の便利関数を `shiori4_host` だけを依存に入れて、どの環境でも使えること, so that x64 の本体が IPC の向こうの 32bit の helper に送る文字列を組み立てるような使い方もでき、既存の利用者は何も変わらない（決定 1・5・6・Approach・Constraints）

#### Acceptance Criteria
1. The shiori4_host shall SHIORI/2.x の便利関数を、`shiori4_host` の公開のモジュール `shiori2` として公開する。
2. The shiori4_host shall `shiori2` の便利関数を、ビルドの対象の OS・arch で限らずに公開する（x86 の Windows に限らない）。
3. The shiori4_host shall `shiori2` を足すときに、既存の公開の型・関数・定数（`shiori4::protocol` と `shiori4_host` のもの）の形と振る舞いを変えない。
4. The shiori4_host shall `shiori2` のために外部の依存を足さない（std と既存の依存だけを使う）。
5. The shiori4_host shall `shiori2` の便利関数で文字コードを変換しない。組み立てと読み分けは UTF-8 の文字列で行い、ANSI のエンコーダーを持たない（版の見分けだけはバイト列も受ける。要件 6）。
6. The shiori4_host shall 芯の口（v4 の `GhostRuntime`・v3 互換モードの `V3Runtime`・従来の DLL の `LegacyRuntime`）を `shiori2` に依存させず、SHIORI/2.x を知らないままにする。
7. The shiori4 shall SHIORI の側の振る舞い（SHIORI/2.x の要求に 400 を返す。版 2）を変えない。

### Requirement 2: 要求の組み立て（汎用の形）
**Objective:** As a ベースウェア実装者, I want メソッド・命令・版・ヘッダを渡すと、SHIORI/2.x の要求の全文が返ること, so that 命令ごとの便利関数に無い命令や版でも、決まりどおりの要求を作れる（決定 2）

#### Acceptance Criteria
1. When ホストがメソッド・命令・版・ヘッダの並びを渡す, the shiori2 shall `<メソッド> <命令> SHIORI/<版>` の開始行、渡した順のヘッダの行、終端の空行からなる要求の全文を返す。各行は CRLF で終える。
2. When ホストが命令を渡さない（`TEACH SHIORI/2.4` の形）, the shiori2 shall 命令の語を入れずに `<メソッド> SHIORI/<版>` の開始行を作る。
3. The shiori2 shall 版を、ホストが渡した SHIORI/2.x の版（2.0〜2.6 など）のとおりに開始行に書く。
4. The shiori2 shall ヘッダの値に含まれる CR・LF を取り除いてから書く（`HeaderWriter` と同じ。ヘッダの改行注入を防ぐ）。
5. If メソッド・命令・版の語に空白・CR・LF が含まれる, the shiori2 shall 要求を作らずに、ホストがそれと分かる失敗を返す（開始行の語はホストの定数で、壊れていれば作りの誤りなので、取り除かずに知らせる。議題 6）。
6. The shiori2 shall 組み立てた要求に、ホストが渡さないヘッダ（`Charset` を含む）を勝手に足さない。

### Requirement 3: 要求の組み立て（命令ごと）
**Objective:** As a ベースウェア実装者, I want よく使う命令の要求を、命令の語・版・決まったヘッダを覚えずに組み立てられること, so that SSP が送るのと同じ形の要求を、間違えずに作れる（決定 2）

#### Acceptance Criteria
1. The shiori2 shall 次の命令の要求を組み立てる便利関数を持つ。開始行の版は、SSP が実際に送る版にする（議題 2。ほかの版で送りたいホストは要件 2 の汎用の形を使う）。
   | 命令 | 開始行 | 命令に固有のヘッダ |
   |------|--------|--------------------|
   | 版の問い合わせ | `GET Version SHIORI/2.6` | なし |
   | 出来事 | `GET Sentence SHIORI/2.2` | `Event`、`Reference0`〜（渡した数だけ） |
   | 語の問い合わせ | `GET Word SHIORI/2.0` | `Type` |
   | 状態の問い合わせ | `GET Status SHIORI/2.0` | なし |
   | 文字列の資源 | `GET String SHIORI/2.5` | `ID` |
   | 主のゴーストの名前 | `NOTIFY OwnerGhostName SHIORI/2.3` | `Ghost` |
   | ほかのゴーストの名前 | `NOTIFY OtherGhostName SHIORI/2.3` | `GhostEx`（ゴーストごとに 1 行。brief の一覧に無いが SSP が送るので入れる。議題 2） |
   | 教える | `TEACH SHIORI/2.4` | `Word`、`Reference0`〜（渡した数だけ） |
   | スクリプトの変換 | `TRANSLATE Sentence SHIORI/2.6` | `Sentence` |
2. When ホストが命令ごとの便利関数に共通のヘッダ（`Charset`・`Sender`・`SecurityLevel` など）の値を渡す, the shiori2 shall それらを命令に固有のヘッダより前に、渡した値のとおりに書く。既定の値は持たず、ホストが渡さない共通のヘッダは書かない（議題 3。決定 6）。
3. When ホストが `Reference` の並びを渡す, the shiori2 shall `Reference0` から順に番号を振って、渡した順に書く。
4. When ホストが `NOTIFY OtherGhostName` にゴーストの並びを渡す, the shiori2 shall ゴーストごとに 1 行の `GhostEx` を、渡した順に書く。1 行の値は、ゴーストの名前・本体の側のサーフィス・相方の側のサーフィスを `\x01` でつないだものにする（一次資料の 2.3）。
5. The shiori2 shall 命令ごとの便利関数の結果を、要件 2 の汎用の形で同じ引数から作った要求と同じ全文にする（同じ決まりで作る）。
6. The shiori2 shall SHIORI/3.0 の出来事を SHIORI/2.x の命令に移す変換を持たない。出来事の名前と `Reference` は、ホストが渡したものをそのまま書く。

### Requirement 4: 応答の読み分け（開始行とヘッダ）
**Objective:** As a ベースウェア実装者, I want SHIORI/2.x の応答の全文から、版・ステータス・ヘッダを読み分けられること, so that 応答の形を自分で解かずに、結果を扱える（決定 2）

#### Acceptance Criteria
1. When ホストが `SHIORI/<版> <ステータス> <理由句>` の開始行で始まる応答の全文を渡す, the shiori2 shall 開始行の版・ステータスの数・ヘッダの並びを返す。ヘッダの並びは `shiori4::protocol::Headers` と同じ規則で読む。
2. The shiori2 shall 改行が CRLF でも LF でも同じに読み、終端の空行より後ろを読まない。
3. The shiori2 shall 理由句を読み分けに使わない（理由句が無いか知らない語でも、ステータスの数だけで読む）。
4. The shiori2 shall 一次資料に無いステータスの数（例: 一次資料に無い 3 桁の数）も、数のまま返す。
5. If 応答の開始行の版が分からない（要件 6.4）か、ステータスの数が読めない, the shiori2 shall 読み分けずに、ホストがそれと分かる失敗を返す。
6. When 応答の開始行が `SHIORI/3` で始まる, the shiori2 shall その版を SHIORI/3.x として返し、SHIORI/2.x の応答と取り違えない（SHIORI/2.x の要求に SHIORI/3.0 の開始行で答える SHIORI がある。華和梨 8）。
7. When ステータスが 311（Not Enough）か 312（Advice）である, the shiori2 shall それを `TEACH` の続きを求める応答としてホストが見分けられる形で返す（問い直しの繰り返しは持たない。議題 5）。

### Requirement 5: 命令ごとの決まったヘッダの取り出し
**Objective:** As a ベースウェア実装者, I want 応答から、命令ごとの決まったヘッダ（スクリプト・語・文字列・状態・版の情報）を取り出せること, so that 命令ごとにどのヘッダを読むかを覚えなくて済む（決定 2）

#### Acceptance Criteria
1. The shiori2 shall 次の決まったヘッダを応答から取り出す便利関数を持つ。
   | 命令 | 取り出すヘッダ |
   |------|----------------|
   | 出来事・スクリプトの変換（`GET Sentence`・`TRANSLATE Sentence`） | `Sentence`（スクリプト） |
   | 語の問い合わせ（`GET Word`） | `Word` |
   | 文字列の資源（`GET String`） | `String` |
   | 状態の問い合わせ（`GET Status`） | `Status`（値は分けずに原文のまま） |
   | 版の問い合わせ（`GET Version`） | `ID`・`Craftman`・`Version`（どれも無くてよい） |
2. If 応答に決まったヘッダが無い, the shiori2 shall 値が無いことを返す（失敗にしない）。
3. If 決まったヘッダが無く `Value` ヘッダがある, the shiori2 shall `Value` の値を代わりに返す。決まったヘッダと `Value` の両方があるときは、決まったヘッダを返す（SHIORI/3.0 の書き方で答える SHIORI/2.x の SHIORI を取りこぼさない。議題 4）。`GET Version` の `ID`・`Craftman` には `Value` を当てない（`Value` 1 つを名前・作者・版として取り違えないため。設計議題 2）。
4. The shiori2 shall 決まったヘッダを、ステータスが 200 のときだけでなく、渡された応答のステータスによらず取り出せるようにする（311・312 の応答もスクリプトを持つため。どのステータスで使うかはホストが決める）。
5. The shiori2 shall 決まったヘッダの値を、取り出したまま（文字コードの変換・エスケープの解釈をせずに）返す。

### Requirement 6: 版の見分け（文字列とバイト列）
**Objective:** As a ベースウェア実装者, I want 応答の開始行の版を、文字コードを変える前のバイト列からも読めること, so that 本文が Shift_JIS の応答でも、変換する前に SHIORI/3.0 か SHIORI/2.x かを決められる（決定 3・6）

#### Acceptance Criteria
1. When ホストが応答のバイト列を渡す, the shiori2 shall 文字コードを仮定せずに、開始行の先頭の ASCII の `SHIORI/2` か `SHIORI/3` だけから版（SHIORI/2.x か SHIORI/3.x）を読む。小さい方の番号は読まない（`SHIORI/2` まで読めば決まるため。設計議題 1）。
2. When ホストが応答の文字列を渡す, the shiori2 shall バイト列のときと同じ結果を返す。
3. The shiori2 shall 版の見分けに、開始行より後ろ（ヘッダ・本文）を読まない。
4. If 開始行が `SHIORI/2` でも `SHIORI/3` でも始まらない（空・ASCII でない・ほかの大きい方の番号）, the shiori2 shall 版が分からないことを返す（失敗を版の推測で埋めない）。
5. The shiori2 shall 版の見分けを、ステータスの数で行わない（SHIORI/2.x の SHIORI は辞書に無い出来事にも 400 を返すので、400 は版の手がかりにならない）。

### Requirement 7: 版の切り替えの手順
**Objective:** As a ベースウェア実装者, I want 読み込んだ SHIORI に最初に送る要求と、その応答からこの後に SHIORI/3.0 と SHIORI/2.x のどちらで話すかを決める手順を、便利関数として使えること, so that 版の見分けと切り替えをホストごとに書かずに済む（決定 3）

#### Acceptance Criteria
1. The shiori2 shall 版を見分けるために最初に送る要求として、SSP と同じ `GET Version SHIORI/2.6` を組み立てる便利関数を持つ（SSP の実測で確かめた手順。議題 1）。
2. When ホストが最初の要求への応答（文字列かバイト列）を渡し、開始行が `SHIORI/3` で始まる, the shiori2 shall この後は SHIORI/3.0 の形で話すと返す。
3. When ホストが応答を渡し、開始行が `SHIORI/2` で始まる, the shiori2 shall この後は SHIORI/2.x で話すと返す。
4. If 応答の版が分からない（要件 6.4）か、ホストが応答を得られなかった（ABI のエラー・空の応答）, the shiori2 shall どちらとも決めずに、版が分からないことを返す。そのゴーストをどう扱うかはホストが決める。
5. The shiori2 shall 版の切り替えの手順に、ステータスの数を使わない（要件 6.5）。
6. The shiori2 shall 版の切り替えの手順で、DLL を呼ばず、状態を持たない（要求を送るのも、決まった版を覚えておくのもホスト。便利関数は「何を送るか」と「応答をどう読むか」だけを持つ。関数の詳しい形は設計で決める）。
7. The shiori2 shall 版の切り替えの手順で、応答の `Charset` を読んでこの後の文字コードを決めることをしない。応答の `Charset` は、ホストが要件 4 のヘッダの並びから読める（決定 6）。
8. The shiori2 shall 版の見分けを、どの要求への応答にも同じに当てられるようにする（開始行の版だけで決めるので、`GET Version SHIORI/2.6` の代わりに SHIORI/3.0 の要求を先に送るホストも、同じ関数で見分けられる。議題 1）。

### Requirement 8: テストと文書
**Objective:** As a shiori4 の保守者・ベースウェア実装者, I want 便利関数の振る舞いがテストで確かめられ、どこから使うかが文書で分かること, so that 決まりが変わらずに保たれ、作者が迷わずに使える（決定 4・Scope）

#### Acceptance Criteria
1. The shiori4_host shall 要件 2〜7 の振る舞いを確かめるテストを、`shiori2` と同じファイルの `mod tests` に持つ。
2. The shiori4_host shall テストに、一次資料と SSP の記録にある要求と応答の例（例: `SHIORI/2.6 200 OK` と `ID`・`Craftman`・`Version` の版の応答、Shift_JIS の本文を持つ応答のバイト列）を使う。
3. The shiori4_host shall 試験用のホスト（`shiori4-testhost`）を変えない。
4. The shiori4_host shall crate doc（`//!`）の道案内に `shiori2` を足し、便利関数ごとに doc を付ける。doc には、文字コードの変換・出来事の移し方・DLL の読み込みはホストと `LegacyRuntime` が持つことを書く。
5. The shiori4_host shall 教科書の移行の手引き（`doc/textbook/migration.md`）の「v3 互換モードと旧来の DLL」の箇条に、`shiori2` へ導く 1 行を足す。表の版 2 の行（「SHIORI/2.x の要求には 400 を返す」）は正本の写しなので変えない（議題 7）。
6. The shiori4_host shall 文書で、まだ出ていない版を番号で書かない（`doc/release-notes/README.md` の「次の版の印」を使う）。
7. The shiori4_host shall `cargo clippy --all-targets --workspace -- -D warnings` と `cargo test --all` を、既存の CI の job のまま通す。

## 要件ディスカッションで決めたこと（2026-10-25）
全議題を推奨案で確定した。

1. **版の見分けの最初の要求**: SSP と同じ `GET Version SHIORI/2.6` を送る（要件 7.1）。見分けは開始行の版だけで決めるので、SHIORI/3.0 の要求を先に送るホストも同じ関数を使える（要件 7.8）。brief の決定 3 を改めた。
2. **命令ごとの便利関数**: 要件 3.1 の 9 つの命令を、SSP が実際に送る版で持つ（`NOTIFY OtherGhostName` を含む）。ユーザーの言葉の `GET Sentence SHIORI/2.0` とゴーストどうしの会話の `GET Sentence SHIORI/2.3` は汎用の形で作る。
3. **共通のヘッダ**: `Charset`・`Sender`・`SecurityLevel` に既定の値を持たず、ホストが渡したものだけを書く（要件 3.2）。
4. **`Value` への代替**: 決まったヘッダを先に読み、無ければ `Value` を読む（要件 5.3）。
5. **`TEACH` の 311・312**: 問い直しの繰り返しは持たず、ステータスの見分けだけを持つ（要件 4.7）。
6. **開始行の語の検査**: 空白・CR・LF を含む語は失敗にする（要件 2.5）。
7. **教科書への一言**: `migration.md` の「v3 互換モードと旧来の DLL」の箇条に 1 行足す。表の版 2 の行は変えない（要件 8.5）。

切り替えの手順の詳しい形（元の Q7）・版と失敗の enum の形・戻り値の形・ヘッダの名前の定数・README の一言は、設計で決める（research.md の「Recommendations for Design Phase」）。
