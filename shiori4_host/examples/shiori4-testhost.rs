//! 試験用のホスト `shiori4-testhost`: ベースウェア無しで SHIORI のライブラリを読み、要求を 1 回投げて、結果を表示して unload する
//!
//! ```text
//! shiori4-testhost [--entry v3] [-H "名前: 値"]... <SHIORI のファイル> <ゴーストのディレクトリ> [ID]
//! ```
//!
//! - 口は発見の順（v4 → v3 → `loadu` → `load`）で探す。`--entry v3` は v3 互換モードだけを試す
//! - v4 は 5 つの必須ヘッダ（`doc/host-protocol.md` 第 2.2 節）に `-H` のヘッダを続けて要求する。v3 は SHIORI/3.0 の全文で要求する
//! - post は表示するだけで実行せず、完了通知も返さない
//! - 信頼できる SHIORI だけを渡す（ライブラリを開くと、その初期化のコードが動く）
//!
//! 終了の状態: 0 = 2xx、1 = 2xx 以外、2 = ABI エラー、3 = 読み込みの失敗、4 = 従来の口だけ、64 = 引数の誤り。
//! 名前・引数・終了の状態は他のリポジトリの CI の契約なので、変えるときは `doc/textbook/testhost.md` も直す。
//! 依存は `shiori4_host` と std だけ（`cargo install --example` は dev-dependencies を使わない）

use std::{ffi::OsString, process::ExitCode};

use shiori4_host::{
    EntryKind, GhostRuntime, Host, LoadError, LoadedGhost, Post, SHIORI_E_INVALID_ARG,
    SHIORI_E_INVALID_UTF8, SHIORI_E_PANIC, SHIORI_E_REENTRANT, SHIORI_E_SHUTDOWN,
    SHIORI_E_UNSUPPORTED, SHIORI_E_WRONG_THREAD, ShioriStatus, V3Runtime, load_ghost,
    load_ghost_with, protocol::HeaderWriter, shiori_is_success,
};

const NAME: &str = "shiori4-testhost";
const USAGE: &str = "usage: shiori4-testhost [--entry v3] [-H \"名前: 値\"]... <SHIORI のファイル> <ゴーストのディレクトリ> [ID]\n信頼できる SHIORI だけを渡す（ライブラリを開くと、その初期化のコードが動く）";

// 終了の状態
const EXIT_OK: u8 = 0;
const EXIT_NOT_2XX: u8 = 1;
const EXIT_ABI_ERROR: u8 = 2;
const EXIT_LOAD_ERROR: u8 = 3;
const EXIT_LEGACY: u8 = 4;
const EXIT_USAGE: u8 = 64;

/// 解析した引数
#[derive(Debug, PartialEq)]
struct Args {
    /// `--entry v3`: v3 互換モードだけを試す
    v3_only: bool,
    /// `-H` で与えたヘッダ（与えた順）
    headers: Vec<(String, String)>,
    lib: OsString,
    dir: OsString,
    id: String,
}

/// 引数（プログラム名を除く）を解析する。Err は使い方の前に出す理由
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
    let mut args = args.into_iter();
    let mut v3_only = false;
    let mut headers = Vec::new();
    let mut positional = Vec::new();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--entry") => match args.next().as_ref().and_then(|v| v.to_str()) {
                Some("v3") => v3_only = true,
                _ => return Err("--entry に与えられるのは v3 だけ".into()),
            },
            Some("-H") => {
                let v = args.next().ok_or("-H の後にヘッダが無い")?;
                let v = v.to_str().ok_or("-H のヘッダが UTF-8 でない")?;
                let (name, value) = v
                    .split_once(':')
                    .ok_or_else(|| format!("-H のヘッダに `:` が無い: {v}"))?;
                let name = name.trim();
                if name.is_empty() || name.contains(['\r', '\n']) {
                    return Err(format!("-H のヘッダの名前が正しくない: {v}"));
                }
                headers.push((name.to_owned(), value.trim().to_owned()));
            }
            Some(s) if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("知らないオプション: {s}"));
            }
            _ => positional.push(arg),
        }
    }
    let mut positional = positional.into_iter();
    let (Some(lib), Some(dir)) = (positional.next(), positional.next()) else {
        return Err("SHIORI のファイルとゴーストのディレクトリが要る".into());
    };
    let id = match positional.next() {
        None => "OnBoot".to_owned(),
        Some(id) => id.into_string().map_err(|_| "ID が UTF-8 でない")?,
    };
    if positional.next().is_some() {
        return Err("引数が多すぎる".into());
    }
    Ok(Args {
        v3_only,
        headers,
        lib,
        dir,
        id,
    })
}

/// v4 の要求のヘッダ部分: 第 2.2 節の 5 つ（`SecurityLevel` は `ID` より前）に `-H` のヘッダを続ける
fn request_headers(id: &str, extra: &[(String, String)]) -> String {
    extra
        .iter()
        .fold(
            HeaderWriter::new()
                .push("SecurityLevel", "local")
                .push("ID", id)
                .push("Sender", NAME)
                .push("SenderType", "internal")
                .push("SecurityOrigin", "null"),
            |w, (n, v)| w.push(n, v),
        )
        .finish()
}

/// v3 の要求の全文
fn v3_request(headers: &str) -> String {
    format!("GET SHIORI/3.0\r\nCharset: UTF-8\r\n{headers}\r\n")
}

/// SHIORI/3.0 の応答の開始行からステータスコードを取る
fn v3_status(response: &str) -> Option<ShioriStatus> {
    let line = response.lines().next()?;
    let mut words = line.split(' ');
    (words.next()? == "SHIORI/3.0").then_some(())?;
    words.next()?.parse().ok()
}

fn abi_error_name(s: ShioriStatus) -> &'static str {
    match s {
        SHIORI_E_INVALID_ARG => "SHIORI_E_INVALID_ARG",
        SHIORI_E_UNSUPPORTED => "SHIORI_E_UNSUPPORTED",
        SHIORI_E_PANIC => "SHIORI_E_PANIC",
        SHIORI_E_REENTRANT => "SHIORI_E_REENTRANT",
        SHIORI_E_SHUTDOWN => "SHIORI_E_SHUTDOWN",
        SHIORI_E_INVALID_UTF8 => "SHIORI_E_INVALID_UTF8",
        SHIORI_E_WRONG_THREAD => "SHIORI_E_WRONG_THREAD",
        _ => "(unknown)",
    }
}

fn load_error_name(e: &LoadError) -> &'static str {
    match e {
        LoadError::Library(_) => "Library",
        LoadError::NotShiori4 => "NotShiori4",
        LoadError::Unsupported(_) => "Unsupported",
        LoadError::InvalidPath => "InvalidPath",
        LoadError::LoadFailed => "LoadFailed",
    }
}

/// 往復の結果を表示して終了の状態にする
fn finish(result: Result<(ShioriStatus, &str), ShioriStatus>) -> u8 {
    match result {
        Ok((status, response)) => {
            println!("status: {status}");
            println!("response:\n{response}");
            if shiori_is_success(status) {
                EXIT_OK
            } else {
                EXIT_NOT_2XX
            }
        }
        Err(e) => {
            println!("abi error: {e} {}", abi_error_name(e));
            EXIT_ABI_ERROR
        }
    }
}

/// 最小の Host: post は表示だけして実行せず、Completer を使わずに捨てる（完了通知を返さない）
struct TestHostImpl;

impl Host for TestHostImpl {
    fn name(&self) -> &str {
        NAME
    }

    fn on_post(&self, post: Post) {
        println!("post: {}", post.text());
    }

    fn log(&self, level: i32, msg: &str) {
        println!("log[{level}]: {msg}");
    }
}

fn run_v4(mut rt: GhostRuntime<TestHostImpl>, args: &Args) -> u8 {
    let req = request_headers(&args.id, &args.headers);
    println!("request:\n{req}");
    let code = match rt.request(&req) {
        Ok(r) => finish(Ok((r.status, &r.body.to_str_lossy()))),
        Err(e) => finish(Err(e)),
    };
    // 要求で積まれた post を 1 回だけ届ける。後に届いたものは Drop が捨てる
    rt.pump();
    drop(rt);
    println!("unloaded");
    code
}

fn run_v3(mut rt: V3Runtime, args: &Args) -> u8 {
    let req = v3_request(&request_headers(&args.id, &args.headers));
    println!("request:\n{req}");
    let code = match rt.request(&req) {
        // 開始行が読めない応答は 0 として 2xx 以外に数える
        Ok(text) => finish(Ok((v3_status(&text).unwrap_or(0), &text))),
        Err(e) => finish(Err(e)),
    };
    drop(rt);
    println!("unloaded");
    code
}

/// 従来の口: shiori3-legacy-host がこの枝を差し替えて従来の DLL を読む
fn run_legacy(library: shiori4_host::libloading::Library) -> u8 {
    println!("従来の口は読まない（shiori3-legacy-host が足す）");
    drop(library);
    EXIT_LEGACY
}

fn run(args: Args) -> u8 {
    let loaded = if args.v3_only {
        load_ghost_with(&args.lib, &args.dir, TestHostImpl, &[EntryKind::V3])
    } else {
        load_ghost(&args.lib, &args.dir, TestHostImpl)
    };
    match loaded {
        Ok(LoadedGhost::V4(rt)) => {
            println!("entry: v4");
            run_v4(rt, &args)
        }
        Ok(LoadedGhost::V3(rt)) => {
            println!("entry: v3");
            run_v3(rt, &args)
        }
        Ok(LoadedGhost::Legacy { entry, library }) => {
            let name = if entry == EntryKind::Loadu {
                "loadu"
            } else {
                "load"
            };
            println!("entry: {name}");
            run_legacy(library)
        }
        Err(e) => {
            eprintln!("load error: {}: {e}", load_error_name(&e));
            EXIT_LOAD_ERROR
        }
    }
}

fn main() -> ExitCode {
    let code = match parse_args(std::env::args_os().skip(1)) {
        Ok(args) => run(args),
        Err(why) => {
            eprintln!("{why}\n{USAGE}");
            EXIT_USAGE
        }
    };
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn defaults_to_on_boot_and_all_entries() {
        let a = parse(&["a.dll", "/g"]).unwrap();
        assert_eq!(
            a,
            Args {
                v3_only: false,
                headers: vec![],
                lib: "a.dll".into(),
                dir: "/g".into(),
                id: "OnBoot".into(),
            }
        );
    }

    #[test]
    fn options_anywhere_and_headers_in_order() {
        let a = parse(&[
            "-H",
            "Reference0: x",
            "a.dll",
            "--entry",
            "v3",
            "/g",
            "OnClose",
            "-H",
            "Status:  y ",
        ])
        .unwrap();
        assert!(a.v3_only);
        assert_eq!(a.id, "OnClose");
        assert_eq!(
            a.headers,
            vec![
                ("Reference0".into(), "x".into()),
                ("Status".into(), "y".into())
            ]
        );
    }

    #[test]
    fn usage_errors() {
        for bad in [
            &[][..],
            &["a.dll"],
            &["a.dll", "/g", "ID", "extra"],
            &["--entry", "v4", "a.dll", "/g"],
            &["--entry"],
            &["-H", "no-colon", "a.dll", "/g"],
            &["-H", ": empty name", "a.dll", "/g"],
            &["a.dll", "/g", "-H"],
            &["--bogus", "a.dll", "/g"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn v4_headers_put_security_level_before_id() {
        let h = request_headers("OnBoot", &[("Reference0".into(), "x".into())]);
        assert_eq!(
            h,
            "SecurityLevel: local\r\nID: OnBoot\r\nSender: shiori4-testhost\r\n\
             SenderType: internal\r\nSecurityOrigin: null\r\nReference0: x\r\n"
        );
        assert_eq!(
            v3_request("ID: OnBoot\r\n"),
            "GET SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnBoot\r\n\r\n"
        );
    }

    #[test]
    fn v3_status_reads_start_line() {
        assert_eq!(v3_status("SHIORI/3.0 204 No Content\r\n\r\n"), Some(204));
        assert_eq!(v3_status("HTTP/1.1 200 OK\r\n"), None);
        assert_eq!(v3_status(""), None);
    }
}
