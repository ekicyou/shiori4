# Brief: host-protocol

## Problem
v4 で SHIORI とホストが交わすメッセージの語彙は、`shiori4::host_protocol` に「暫定。ホスト側プロトコル仕様で確定させる」として置かれているだけである。今あるのはプロパティの読み・post の `Command` と `Tag`・完了通知 `OnHostComplete` の名前だけで、足りないものが多い。

- プロパティの書き込みが無い。
- post のコマンドが 1 つも決まっていない。
- 拒否の返し方が決まっていない。
- ホストが必ず付けるヘッダが決まっていない。
- SHIORI の名前と版をホストが知る手段が無い。

SHIORI 実装者（pasta）にとって v4 の最大の利点は「SHIORI から今すぐ台本を出せる」ことだが、それを表す語が無い。ベースウェア実装者（areka）は、自分の spec `shiori4-api` の上流として参照できる正本を必要としている。

## Current State
- `shiori4/src/lib.rs` の `host_protocol` モジュール（`ID_PROPERTY`・`HEADER_VALUE`・`HEADER_COMMAND`・`HEADER_TAG`・`ID_HOST_COMPLETE`・`HEADER_STATUS`）。
- `shiori4_host` の `Host` trait と `Bridge` の組み込みの処理:
  - `property` に 200 ＋ `Value` か 204 で答える（値の改行は取り除く）
  - `accept_post` は `Command` の有無だけを見る
  - 完了通知は request か notify のどちらかでホストが届ける

2026-10-23 に、pasta と areka の意見に shiori4 側の推奨を返して合意した（下の「合意した論点」）。

## Desired Outcome
- ホスト側プロトコル仕様（文書）が shiori4 リポジトリにあり、pasta・areka・shiori4 の 3 者の正本になっている。
- `host_protocol` の定数と `shiori4_host` の組み込みの処理が、その仕様どおりに動く。
- SHIORI 側の高水準層（`shiori4`）から、プロパティの読み書きと SEND / NOTIFY の post を型のついた呼び方で使える。

## 合意した論点（pasta = P、areka = A）
1. **プロパティの読み書き**（A1）:
   - 書き込み: `ID: setproperty` / `Reference0: 名前` / `Reference1: 値` → 204 成功・400 不正。
   - 読み: ukadoc の語に寄せて `property` を `getproperty` に改名し、get / set の対にする。この改名は 0.0.1 の動きを壊す唯一の変更だが、利用者はまだいないので 0.0.x のうちに入れる。
2. **post の語彙**（A2・P4）: SSTP の語に寄せる。post は「同じプロセスからの SSTP」として扱う。pasta の `Command: script` 案は取り下げられた。
   - `Command: SEND` ＋ `Script: <さくらスクリプト>`: 今すぐ再生する。
   - `Command: NOTIFY` ＋ `Event` / `Reference*`: ホストがそのイベントを SHIORI に送る。
3. **post の宛先**（P 追加 a）: 宛先は「post したインスタンス」に固定する。SSTP の `IfGhost` などの宛先指定は読まない。
4. **プロパティ名の決まり**（A4）:
   - 名前は ukadoc のプロパティシステムの名前をそのまま使う。
   - ベースウェア固有の名前は接頭辞（例 `areka.`）で分ける。
   - 大文字小文字は区別しない。
   - 値の改行は取り除く（エスケープして残すことはしない）。改行を含む値が要るプロパティが出たら、別の名前で足す。
5. **v4 の要求ヘッダの語彙**（A7・P7）:
   - v4 のヘッダの名前と意味は SHIORI/3.0（ukadoc）と同じ（ID・Reference*・Sender・SecurityLevel・SecurityOrigin・BaseID・Status 等）。
   - ホストが必ず付けるヘッダを決める。
6. **SHIORI の名前と版**（P7）: v3 の GET Version に当たる手段を決める（候補 `ID: version`）。
7. **OnHostComplete の届け方**（A8）: 既定は request で届ける（結果を台本で返す必要がありうるため）。SHIORI は request と notify の両方を受けられなければならない。Reference の並びを確定させる。

## 要件・設計の段で裁定する論点
- **拒否の返し方**（A3）: ベースウェアが安全上の理由で、プロパティの読み書きや post を断るときの返し方。「存在しない（204）」と区別する。2 案のどちらにするかを開発者が裁定する。
  - 案 1（shiori4 の推奨）: 400 ＋理由のヘッダ（`Reason: forbidden`。`X-` は付けない）。SHIORI/3.0 の状態コードの範囲を崩さない。
  - 案 2: 403 を ABI の定数に足す。v4 だけの話なので技術的には可能。この案に決まったら、定数は abi-conventions の `shiori4_abi` に入れる。
- **SEND の割り込み**（P 追加 b）: 台本の再生中に SEND が来たときの扱い（割り込む / 待つ / 捨てる）。pasta には連続トークの途中で割り込まれる場面（chain-talk-interrupt）がある。
  - shiori4 の推奨: 既定は「待つ（キューに積む）」。割り込みは post 側が明示したときだけ（例 `Interrupt: true`。名前は設計で決める）。
  - 結果は、Tag があれば OnHostComplete の Status で返す（再生した / 捨てた の値の割り当ても決める）。
  - 割り込まれた側の連続トークへの知らせ方も決める。
  - areka の再生キューと、SSTP SEND の割り込みの今の扱いを 2026-10-23 に問い合わせ中。返事を推奨に反映する。

## Approach
仕様の本文は、新しい文書 `doc/host-protocol.md` に置く。置き場所は設計で確定させる。`shiori4` の crate doc と `host_protocol` モジュールの doc はその文書を指す。

- 定数は `shiori4::host_protocol` に足す。
- `shiori4_host` の `Host` trait と `Bridge` の組み込みの処理（getproperty・setproperty・accept_post の既定・完了通知）を、仕様どおりにする。
- SHIORI 側の高水準層には、get / set のプロパティと、SEND / NOTIFY の post の呼び方を足す。

## Scope
- **In**:
  - ホスト側プロトコル仕様の文書。
  - `host_protocol` の定数（改名と追加）。
  - `shiori4` のプロパティと post の呼び方。
  - `shiori4_host` の `Host` trait の追加メソッド（setproperty など）と組み込みの処理。
  - 上のテスト。
  - pasta・areka への正本の場所の連絡。
- **Out**:
  - ABI の規約（スレッド・寿命・log・load の dir・Charset）。これは abi-conventions。
  - SSTP そのもの（ネットワーク越しの SSTP の受け口）。
  - ukadoc のプロパティの一覧を shiori4 が持つこと（名前の決まりだけを持つ）。
  - ベースウェアの再生キューの実装。

## Boundary Candidates
- 仕様の文書（語彙の正本）
- SHIORI 側の型のついた呼び方（`shiori4`）
- ホスト側の組み込みの処理と `Host` trait の拡張（`shiori4_host`）

## Out of Boundary
- ABI の関数テーブルとスレッド規約（abi-conventions）
- areka の SSTP の受け口・安全度の扱い（areka が持つ）
- pasta のシーン再生の仕組み（pasta が持つ）

## Upstream / Downstream
- **Upstream**: v0.0.1 の ABI と `host_protocol`。abi-conventions とは独立だが、同じファイルに触れる
- **Downstream**:
  - areka の spec `shiori4-api`（areka リポジトリ）が、この仕様を上流として参照する。
  - pasta の v4 対応。
  - 0.0.2 のリリース。

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: abi-conventions（403 の定数を足す場合の置き場所、`shiori4/src/lib.rs`・`shiori4_host/src/lib.rs` の同時編集）

## Constraints
- 確定した語は shiori4 を正本とする。areka・pasta の文書は shiori4 の仕様を参照し、語を写して変えない。
- property の改名以外は、0.0.1 の動きを変えない（足すだけ）。
- ヘッダの書式は SHIORI/3.0 と同じ（`名前: 値`、CRLF、ヘッダ部分だけ）。
- 要件の段に入ったら、決まった語を pasta・areka の両セッションへ知らせる。
