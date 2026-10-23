# ヒアリング: ABI の教科書（README・docs.rs・C ヘッダの説明）

2026-10-23 に、SHIORI の作者（pasta、`SHIORI4:pasta`）とベースウェアの作者（areka、`SHIORI4:areka`）に、欲しい文書を聞いた。

- shiori4 の開発者の方針: 最後には shiori4 リポジトリが「ABI の教科書」を持ち、書くのも shiori4 の仕事である。
- この記録は spec ではない。spec にする時期と形は、開発者が決める。
- 下の「P」は pasta、「A」は areka の要望を表す。

## 1. 読む順（両者でほぼ一致）
1. **1 枚の全体図**（P・A）: 誰が誰を、どのスレッドから、いつ呼べるか。対象は load・request・notify・host.request・post・log・release・unload の 8 つで、表 1 つにまとめる。今回の会談でいちばん時間を使った所（規約 2・5 の解釈）である。
2. **最小の例**:
   - P: Rust と C の両方。v3 だけで 30 行ほど。
   - A: 最小のホスト。読む → OnBoot の request → unload。
3. **規約の一覧**（P・A）: 番号を付ける。実装側から「規約 N」で引用できる形を保つ。
4. **host-protocol の語の表**（A）: ID・ヘッダ・ステータス・Reason。
5. 細部。

README は 1 と 2 と、各文書への道案内だけで足りる（A）。

## 2. docs.rs: SHIORI 側（`shiori4`）（P）
- 処理を別のスレッドで動かす SHIORI の例。アクターからの host.request を、応答を待っている SHIORI のスレッドへ中継する形を示す。
- post → Tag → OnHostComplete で、待っている処理を再開する例。PendingTags の使い方と、登録してから post する理由も書く。
- join しない unload の例。poster が unload の開始で無効になることと、固まったスレッドがあっても規約を守れることを示す。
- 状態をインスタンスごとに持つ例（グローバルな状態を置かない書き方）。
- v3 と v4 の両方で動く Ghost の例（host が None のときの代わりの動き）。

## 3. docs.rs: ホスト側（`shiori4_host`）（A）
- 読み込みの失敗の分かれ方。LoadError の値ごとに、ホストがすべきことを書く。
  - NotShiori4: 旧来の SHIORI/3.0 として読む。
  - LoadFailed: 読めなかったゴーストとして扱う。
- 今の `GhostRuntime::load` は、NotShiori4 のときに読んだライブラリを手放す。SHIORI/3.0 として読み直すと、DLL が 2 回読まれることを明記する（下の論点 e）。
- post のキュー。wake から pump へのつなぎ方を 2 通り示す（Win32 のメッセージの輪と、std の channel のスレッド）。
- Completer で OnHostComplete を返す例。
  - SEND（completed と interrupted）と NOTIFY の両方。
  - nobreak で待たせた SEND を、待つ間の終了で捨てる（204）例。
- getproperty と setproperty に答える Host の例。拒否を 400 ＋ `Reason: forbidden` で返す例。
- unload の順序（shutdown → 未配送を捨てる → unload → ライブラリを放す）と、Drop の中で起きること。
- テストの書き方。`GhostRuntime::from_static` で、DLL なしに通す決定論の試験。

## 4. ホストを別のプロセスで動かすときの注意（A。短くてよい）
- host.request の中継は同期の往復になる。親は、SHIORI の応答を待つ間にも子からの問い合わせに答えられる待ち方にする。そうしないと互いに待って止まる。
- post は子のプロセスで受けてから親へ送るので、受付（204）と完了は別の時点になる。
- SHIORI_E_PANIC や子のプロセスの異常終了は、インスタンスの終わりとして扱う。

## 5. C ヘッダの説明（Rust 以外の作者向け）
- **所有権の表と図**（P・A）:
  - out の ShioriBytes を誰が確保し、誰がいつ release するか。
  - release はどのスレッドからも呼ばれうる。
  - 戻り値が負のときも out を解放する。これは例を付けて示す。
- 静的 CRT で作る DLL の注意（P）: 確保した側が解放するので、CRT が別でも安全である、と一言書く。
- **版の判定の例**（P・A）:
  - ホスト側の手順: shiori_get_api(4) が NULL なら 3 を試す。
  - SHIORI 側: 版ごとに別の静的なテーブルを返す。
- 最小の C のホストの例（A）: LoadLibrary／dlopen → shiori_get_api(4) → 無ければ 3 → load → request → unload。
- `struct_size` で、足されたフィールドの有無を確かめる例（`SHIORI_HAS_FIELD`）（A）。
- 呼ぶスレッドの規約を、C の書き手の言葉で書く（A）。
- パニックや例外を境界の外へ出さない書き方（P）: C++ の例外、Zig の panic など、言語ごとに一言。

## 6. 規約の「なぜ」（P・A）
- 各規約に、1〜2 文の理由を付ける。長い経緯は別のページでよい。
- 理由が無いと実装者が緩めたり、「親切に」破ったりしそうなもの:
  - ABI を緩めない理由（P）: host.request をホストの UI スレッドで待つとデッドロックする、など。
  - 再入の禁止（A）。
  - post の完了を、post の中で届けないこと（A）。
  - unload で未配送を捨てること（A）。
  - SEND が既定で割り込むこと（SSTP に揃えた）（A）。

## 7. 移行の手引き（P・A とも要る。短くてよい）
- **SHIORI 側**（P）: SHIORI/3.0 の DLL → v3 互換 → v4 の 3 段で。差分の表で足りる。
  - Charset は UTF-8 だけ（Shift_JIS の要求は来ない）。
  - HGLOBAL が (ptr, len) と release に変わる。
  - load の dir の形が変わる。
- **ホスト側**（A）:
  - 要求は、開始行と Charset を外すだけ。ヘッダの語は同じ。
  - 応答の状態は戻り値で受ける。
  - 旧来の DLL との見分け方（shiori_get_api の有無）と、両方を持つ DLL の扱い。
  - v3 互換モードは他の OS のホスト向けである。Windows のホストは、旧来の DLL を今までどおり読めばよい。

## 8. 文書と一緒に案内してほしいもの
- **host_protocol の確定の範囲**（A）: どれが確定で、どれが変わりうるか。版ごとの一覧にする。
- **ゴーストの配布の決まり**（A・P）: abi-conventions の論点 12 が入ったら、章を 1 つ設ける。DLL を読まずに選ぶ手順を、ホストの作者向けに書く。
- **試験用のホスト**（P）: ベースウェア無しで SHIORI の DLL を読み、要求を投げて応答を見るもの。shiori4_host の例か、小さな CLI。Linux と macOS の CI で、DLL の境界まで試すために使う。

## 規約そのものの論点になったもの（文書だけでは済まない。開発者の裁定へ）
- **a. v3 の load の失敗理由**（P）: v3 では host が NULL で log を使えない。理由をどこに出すか。
- **b. 両方を持つ DLL**（P・A）: 同じ DLL が SHIORI/3.0 の書き出し（load・request・unload）と shiori_get_api の両方を持ってよいか。ホストはどちらを先に見るか。pasta は両方を出す。
- **c. Windows 以外のファイル名**（P）: descript.txt の `shiori,pasta.dll` に対し、Linux や macOS のホストがどのファイルを探すか（`pasta.so`・`libpasta.so`・`.dylib`）。Rust の cdylib は既定で `libpasta.so` を作る。abi-conventions の論点 12 と一緒に扱う。
- **d. カレントディレクトリ**（A）: ホストがカレントディレクトリをどうするか。areka は helper のカレントディレクトリをゴーストの ghost/master にする。規約で決めないなら「ホスト次第で、SHIORI は頼らない」と一行書く。
- **e. NotShiori4 のときのライブラリ**（A）: `GhostRuntime::load` が NotShiori4 のときにライブラリを手放すこと。手放さずに返す入口が要るかどうかは、areka の開発者が判断中。
