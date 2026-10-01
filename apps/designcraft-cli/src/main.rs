//! Headless DesignCraft.
//!
//! ```text
//! designcraft-cli run [--in FILE.designcraft|FILE.idml | --sample] [--cmd ID[=JSON]]... [--page N] [--scale S] [--export OUT.png|.jpg|.pdf|.designcraft|.idml|.epub] [--pdf-options JSON] [--all-pages DIR]
//! designcraft-cli commands            # list every command (JSON)
//! designcraft-cli mcp [--connect PORT] [--sample]  # MCP server over stdio (docs/mcp.md)
//! designcraft-cli perf [--pages N] [--frames N] [--chars N] [--images N] [--runs N] [--strict]  # budgets on a synthetic stress document
//! designcraft-cli bench FILE [--runs N]  # the same measurements on one document
//! designcraft-cli links                   # Discord, website, app page and GitHub links
//! ```
use std::process::ExitCode;

mod perf;

use designcraft_engine::Session;
use serde_json::{Value, json};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => report(run(&args[1..])),
        Some("commands") => {
            let s = Session::new();
            println!("{}", serde_json::to_string_pretty(&s.commands()).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Some("mcp") => report(mcp(&args[1..])),
        Some("perf") => report(perf::perf(&args[1..])),
        Some("bench") => report(perf::bench(&args[1..])),
        Some("links") => {
            use designcraft_engine::links::*;
            println!("Discord   {DISCORD}\nWebsite   {WEBSITE}\nApp page  {APP_PAGE}\nGitHub    {GITHUB}\nIssues    {ISSUES}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage: designcraft-cli run [--in FILE | --sample] [--cmd ID[=JSON]]... [--page N] [--scale S] [--pdf-options JSON] [--export OUT] [--all-pages DIR]\n       designcraft-cli commands\n       designcraft-cli mcp [--connect PORT] [--sample]\n       designcraft-cli perf [--pages N] [--runs N] [--strict]\n       designcraft-cli bench FILE [--runs N]\n       designcraft-cli links"
            );
            eprintln!(
                "\nCommunity: {}  ·  {}  ·  {}",
                designcraft_engine::links::DISCORD,
                designcraft_engine::links::APP_PAGE,
                designcraft_engine::links::GITHUB
            );
            ExitCode::FAILURE
        }
    }
}

fn report(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("designcraft-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `mcp` (headless, in-process engine) or `mcp --connect PORT|HOST:PORT` (drive a running app
/// started with `designcraft --control PORT`). JSON-RPC on stdin/stdout; logs on stderr.
fn mcp(args: &[String]) -> Result<(), String> {
    use designcraft_mcp::{Backend, Headless, Remote, Server, control_addr};
    let mut connect: Option<String> = None;
    let mut sample = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--connect" => connect = Some(it.next().cloned().ok_or("--connect needs a port or host:port")?),
            "--sample" => sample = true,
            other => return Err(format!("unknown mcp option `{other}` (usage: designcraft-cli mcp [--connect PORT] [--sample])")),
        }
    }
    let backend: Box<dyn Backend> = match connect {
        Some(c) => {
            let addr = control_addr(&c);
            Box::new(
                Remote::connect(&addr)
                    .map_err(|e| format!("cannot connect to the DesignCraft app at {addr}: {e} (start it with `designcraft --control PORT`)"))?,
            )
        }
        None => {
            let mut h = Headless::with_document();
            if sample {
                h.session.execute("file.newSample", &json!({})).map_err(|e| e.to_string())?;
            }
            Box::new(h)
        }
    };
    eprintln!("designcraft-cli: MCP server on stdio ({})", backend.describe());
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    Server::new(backend).serve(stdin.lock(), stdout.lock()).map_err(|e| e.to_string())
}

fn run(args: &[String]) -> Result<(), String> {
    let mut s = Session::new();
    let mut page = 0usize;
    let mut scale = 1.0f64;
    let mut it = args.iter();
    let mut opened = false;
    let mut pdf_opts = json!({});
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--in" => {
                let p = val()?;
                s.execute("file.open", &json!({"path": p})).map_err(|e| e.to_string())?;
                opened = true;
            }
            "--sample" => {
                s.execute("file.newSample", &json!({})).map_err(|e| e.to_string())?;
                opened = true;
            }
            "--cmd" => {
                if !opened {
                    s.execute("file.new", &json!({})).map_err(|e| e.to_string())?;
                    opened = true;
                }
                let c = val()?;
                let (id, p) = c.split_once('=').unwrap_or((&c, "{}"));
                let p: Value = serde_json::from_str(p).map_err(|e| format!("--cmd {id}: {e}"))?;
                let r = s.execute(id, &p).map_err(|e| e.to_string())?;
                if !r.is_null() {
                    println!("{}", serde_json::to_string(&r).unwrap_or_default());
                }
            }
            "--page" => page = val()?.parse().map_err(|_| "bad --page")?,
            "--scale" => scale = val()?.parse().map_err(|_| "bad --scale")?,
            "--pdf-options" => {
                pdf_opts = serde_json::from_str(&val()?).map_err(|e| format!("--pdf-options: {e}"))?;
            }
            "--export" => {
                let out = val()?;
                if out.ends_with(".epub") {
                    let r = s.execute("file.exportEpub", &json!({"path": out})).map_err(|e| e.to_string())?;
                    eprintln!("wrote {out} ({} bytes)", r["bytes"]);
                } else if out.ends_with(".pdf") {
                    let mut p = pdf_opts.clone();
                    p["path"] = json!(out);
                    let r = s.execute("file.exportPdf", &p).map_err(|e| e.to_string())?;
                    eprintln!("wrote {out} ({} pages, {} bytes)", r["pages"], r["bytes"]);
                    for w in r["warnings"].as_array().into_iter().flatten() {
                        eprintln!("warning: {}", w.as_str().unwrap_or_default());
                    }
                } else {
                    export(&mut s, &out, page, scale)?;
                }
            }
            "--all-pages" => {
                let dir = val()?;
                let n = s.doc().map_err(|e| e.to_string())?.doc.page_count();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                for p in 0..n {
                    export(&mut s, &format!("{dir}/page-{:03}.png", p + 1), p, scale)?;
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(())
}

fn export(s: &mut Session, out: &str, page: usize, scale: f64) -> Result<(), String> {
    if out.to_ascii_lowercase().ends_with(".idml") {
        let r = s.execute("file.exportIdml", &json!({"path": out})).map_err(|e| e.to_string())?;
        eprintln!("wrote {out} ({} bytes)", r["bytes"]);
        return Ok(());
    }
    if out.ends_with(".designcraft") {
        s.execute("file.saveAs", &json!({"path": out})).map_err(|e| e.to_string())?;
        eprintln!("saved {out}");
        return Ok(());
    }
    let st = s.doc().map_err(|e| e.to_string())?;
    let mut r = designcraft_render::Renderer::new();
    let t = std::time::Instant::now();
    let img = r
        .render_page(&st.doc, &s.cache, page, scale, true, &designcraft_render::RenderOptions { printing_only: true, ..Default::default() })
        .ok_or("no such page")?;
    let bytes = if out.ends_with(".jpg") || out.ends_with(".jpeg") { img.to_jpeg(90) } else { img.to_png() };
    std::fs::write(out, bytes).map_err(|e| format!("{out}: {e}"))?;
    eprintln!("wrote {out} ({}×{}, {:.1} ms, {} glyphs)", img.width, img.height, t.elapsed().as_secs_f64() * 1000.0, r.stats.glyphs);
    Ok(())
}
