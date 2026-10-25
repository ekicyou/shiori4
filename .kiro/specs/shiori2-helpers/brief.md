# Brief: shiori2-helpers

## Problem
従来の SHIORI の DLL には、SHIORI/3.0 ではなく SHIORI/2.x で話すものがまだ残っている。`load`・`unload`・`request` の HGLOBAL の口は SHIORI/3.0 と同じで、違うのは `request` に流れる文字列の形だけである。

ベースウェアの作者が SHIORI/2.x の DLL と話したくても、今の shiori4 には SHIORI/2.x の要求の組み立ても、応答の読み分けも無い。版の見分け（SHIORI/3.0 の要求を出し、応答が SHIORI/2.x だったら SHIORI/2.x の要求に切り替える）も、ホストごとに自分で書いている。

## Current State
- `shiori4::protocol` は SHIORI/3.0 の形の便利関数（`Headers`・`HeaderWriter`・語の定数・組み立てと読み分け）を持つ。`shiori4_host::protocol` として再エクスポートされる。SHIORI/2.x の形は扱わない。
- shiori3-legacy-host（完了・2026-10-25）は、x86 の Windows で従来の DLL の HGLOBAL の口を運ぶ芯を足した（`LegacyRuntime`）。芯は中身を解釈せずバイト列で運ぶので、SHIORI/2.x の要求もそのまま往復できる。足りないのは文字列の形の側だけ。
- SHIORI の側の v4 は、SHIORI/2.x の要求に 400 を返す（教科書の版 2）。これは変えない。
- SHIORI/2.x の見分けは、areka の spec `shiori2-protocol` が持つと決めていた（shiori3-legacy-host の裁定 c）。それを 2026-10-25 に改め、便利関数としてこの spec が持つ。

## Desired Outcome
- ホストの作者が、SHIORI/2.x の要求を組み立て、応答を読み分ける関数を `shiori4_host` から使える。
- 版の見分けと切り替えの手順を、便利関数として使える。
- これらの関数がテストで確かめられている。

## Approach
`shiori4_host` に新しいモジュール `shiori2`（`shiori4_host/src/shiori2.rs`）を足す（開発者が案 A を選んだ。2026-10-25）。
- 使うのはホストの側だけなので、ホストのクレートに置く。SHIORI の側のクレート（`shiori4`）には足さない。
- ヘッダの読み分けは `shiori4::protocol::Headers` を使い回す。
- 芯ではなく便利関数である（方針「中身の通信には関与しない」）。芯の口（v4・v3 互換モード・shiori3-legacy-host の口）は SHIORI/2.x を知らないままにする。

却下した案:
- `shiori4::protocol` の下に置く: SHIORI の側では使わない（SHIORI/2.x の要求には 400 を返す）のに、SHIORI の側のクレートに載るため。
- 新しいクレート: 版と公開の手間が 4 クレートに増え、「3 クレートを同じ版で」を崩すため。

## 決まっていること（2026-10-25 のヒアリングから）
1. **持つのはホストの側の便利関数だけ**。SHIORI の側（`Ghost` が SHIORI/2.x に答えること）は要らない。
2. **組み立てとパースの深さ**（推奨の案 2）:
   - 汎用の形: 「メソッド＋命令＋版＋ヘッダ」の要求の組み立てと、「`SHIORI/2.x <ステータス>`＋ヘッダ」の応答の読み分け。
   - 命令ごとの便利関数: `GET Sentence`・`GET Word`・`GET Status`・`GET String`・`NOTIFY OwnerGhostName`・`TEACH`・`TRANSLATE Sentence`・`GET Version` などの組み立てと、応答の決まったヘッダ（`Sentence`・`Word`・`String` など）の取り出し。
   - SHIORI/3.0 の出来事を SHIORI/2.x へ移す変換（例: `ID: OnBoot` → `GET Sentence` ＋ `Event: OnBoot`）は持たない。どの出来事をどの命令に移すかはベースウェアごとの方針で、ホストが持つ。
3. **版の見分けと切り替え**: 最初の要求を出し、応答の開始行の版が SHIORI/2.x だったら SHIORI/2.x の要求に切り替える手順を、便利関数として持つ。最初の要求は SSP と同じ `GET Version SHIORI/2.6` にする（2026-10-25 の要件ディスカッションで改めた。元は「SHIORI/3.0 の要求を先に出す」だったが、SHIORI/2.x の DLL が SHIORI/3.0 の要求にどう答えるかの一次資料が無く、実測で確かめられているのは SSP の順だけのため）。見分けは開始行の版だけで決めるので、SHIORI/3.0 の要求を先に出すホストも同じ関数を使える。応答の開始行は ASCII なので、文字コードを変える前のバイト列からも版を読めるようにする。
4. **試験用のホストは変えない**。便利関数のテストがあればよい。
5. **全環境で使える**。文字列の関数だけなので `cfg` で x86 の Windows に限らない（例: x64 の本体が、IPC の向こうの 32bit の helper に送る文字列を組み立てる）。
6. **文字コードには触れない**。SHIORI/2.x の本文はほとんど Shift_JIS だが、変換はホストが持つ。組み立てと読み分けは `&str` で行う（版の見分けだけはバイト列でも読める。決定 3）。ANSI のエンコーダーは抱え込まない。

## Scope
- **In**:
  - `shiori4_host::shiori2`: SHIORI/2.x の要求の組み立て（汎用の形と命令ごと）、応答の読み分け（ステータス・ヘッダ・決まったヘッダの取り出し）、版の見分けと切り替えの手順。
  - それぞれのテスト（同じファイルの `mod tests`）。
  - 文書（crate doc の道案内・関数の doc・必要なら教科書の移行の手引きへの一言）。
- **Out**:
  - SHIORI の側での SHIORI/2.x への対応（版 2 の「400 を返す」は変えない）。
  - SHIORI/3.0 の出来事から SHIORI/2.x の命令への変換の決まり。
  - 文字コードの変換（Shift_JIS ほか）。
  - 試験用のホスト（`shiori4-testhost`）の変更。
  - HGLOBAL の受け渡しと DLL の読み込み（shiori3-legacy-host が持つ）。
  - SAORI・MAKOTO など、SHIORI/2.x 以外の形。

## Boundary Candidates
- 要求の組み立て（汎用の形と命令ごと）
- 応答の読み分け（開始行とヘッダ）
- 版の見分けと切り替えの手順

## Out of Boundary
- 芯の口（v4・v3 互換モード・従来の DLL の口）。SHIORI/2.x を知らないままにする
- 出来事の 3 → 2 の変換の方針（ホストが持つ）
- 文字コードの方針（ホストが持つ）

## Upstream / Downstream
- **Upstream**: `shiori4::protocol` の `Headers`（使い回す）。shiori3-legacy-host には依存しない（文字列の関数だけで、組み合わせて使う）
- **Downstream**: areka（SHIORI/2.x の DLL と話す所を、この便利関数に乗り換えられる。areka の spec `shiori2-protocol` の範囲と突き合わせる）、ほかのベースウェアの作者

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**:
  - shiori3-legacy-host: SHIORI/2.x の要求もバイト列のまま運ぶ芯を持つ。その要件の Out of scope と brief の裁定 c を、この spec を指すように改めた（2026-10-25）。
  - host-protocol: `shiori4::protocol` の便利関数の形（`Headers`・`HeaderWriter`）に揃える。

## Constraints
- 依存を足さない（std と既存の依存だけ）。
- 版を上げない（`/release` が上げる）。
- 公開の API を足すだけで、既存の型・関数は変えない。
- **調べもの**: SHIORI/2.x の一次資料が、伺かの文書の索引（ukadoc ほか）に無い。命令の一覧・各命令の決まったヘッダ・版の番号（2.0〜2.6）の違い・`GET Version` の応答の形・SHIORI/2.x の DLL が SHIORI/3.0 の要求にどう応答するか（版の見分けの手がかり）を、要件の段で一次資料（当時の SHIORI/2.x の仕様書・SSP の振る舞い）と突き合わせる。brief の命令の一覧は覚えからのもので、確定ではない。
