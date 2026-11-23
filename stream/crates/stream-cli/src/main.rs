//! `stream`: the command-line interface to a STREAM log.

use std::io::{self, BufRead, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use stream_books::{attribution, balances, close_entry, spans, verify_closes};
use stream_kernel::json::Object;
use stream_kernel::{kernel_hash, preamble, Address, Time};
use stream_log::{Genesis, Log, Receipt};

const USAGE: &str = "\
stream: one append-only log for work, resources and software

  stream kernel                                   print the kernel hash and preamble
  stream init <dir> --log <id> --root <actor>[,<actor>…] [--clock ntp] [--tz UTC]
  stream append <dir> --as <recorder> [<line>]    submit a line, or lines from stdin
  stream adopt <dir> --as <recorder> <pack-file>  adopt a pack with one BIND entry
  stream seal <dir>                               seal the open segment
  stream close <dir> --as <recorder> --book <book> [--upto <seg:seq>]
  stream verify <dir>                             check seals, the chain and every close
  stream view <dir> entries|register|gaps|balances|spans|attribution [--upto <seg:seq>]
  stream encode [<file>]                          canonical lines -> run form (stdin if no file)
  stream decode [<file>]                          run form -> canonical lines

  --time <unix-seconds>  overrides the clock for entries the log itself writes
";

fn now() -> Time {
    Time::secs(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

struct Args {
    pos: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    fn parse(v: Vec<String>) -> Args {
        let (mut pos, mut flags) = (vec![], vec![]);
        let mut it = v.into_iter();
        while let Some(a) = it.next() {
            match a.strip_prefix("--") {
                Some(k) => flags.push((k.to_string(), it.next().unwrap_or_default())),
                None => pos.push(a),
            }
        }
        Args { pos, flags }
    }
    fn flag(&self, k: &str) -> Option<&str> {
        self.flags.iter().find(|(f, _)| f == k).map(|(_, v)| v.as_str())
    }
    fn need(&self, k: &str) -> Result<&str, String> {
        self.flag(k).ok_or_else(|| format!("missing --{k}"))
    }
    fn dir(&self) -> Result<PathBuf, String> {
        self.pos.get(1).map(PathBuf::from).ok_or_else(|| "missing <dir>".to_string())
    }
    fn time(&self) -> Result<Time, String> {
        match self.flag("time") {
            Some(t) => Time::parse(t).map_err(|e| e.to_string()),
            None => Ok(now()),
        }
    }
    fn upto(&self) -> Result<Option<Address>, String> {
        self.flag("upto").map(|u| Address::parse(u).ok_or_else(|| format!("bad address {u}"))).transpose()
    }
}

fn read_input(path: Option<&String>) -> Result<String, String> {
    let mut s = String::new();
    match path {
        Some(p) if p != "-" => s = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?,
        _ => {
            io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
        }
    }
    Ok(s)
}

fn show(r: &Receipt) -> String {
    match r {
        Receipt::Accepted(a) => format!("accepted {a}"),
        Receipt::Duplicate(a) => format!("duplicate {a}"),
        Receipt::Rejected { at: Some(a), reason } => format!("rejected {a} {reason}"),
        Receipt::Rejected { at: None, reason } => format!("rejected - {reason}"),
    }
}

fn run(args: Args) -> Result<ExitCode, String> {
    let err = |e: stream_log::LogError| e.to_string();
    match args.pos.first().map(String::as_str) {
        Some("kernel") => {
            println!("{}", kernel_hash());
            println!("{}", preamble());
        }
        Some("init") => {
            let roots: Vec<&str> = args.need("root")?.split(',').collect();
            let log = Log::init(&args.dir()?, Genesis {
                log: args.need("log")?,
                roots: &roots,
                clock: args.flag("clock").unwrap_or("ntp"),
                tz: args.flag("tz").unwrap_or("UTC"),
                time: args.time()?,
            })
            .map_err(err)?;
            println!("created log {} at {}", log.id(), log.location());
        }
        Some("append") => {
            let mut log = Log::open(&args.dir()?).map_err(err)?;
            let who = args.need("as")?;
            let lines: Vec<String> = match args.pos.get(2) {
                Some(l) => vec![l.clone()],
                None => io::stdin().lock().lines().collect::<Result<_, _>>().map_err(|e| e.to_string())?,
            };
            let mut ok = true;
            for l in lines.iter().filter(|l| !l.is_empty()) {
                let r = log.submit_line(who, l, args.time()?).map_err(err)?;
                ok &= !matches!(r, Receipt::Rejected { .. });
                println!("{}", show(&r));
            }
            return Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) });
        }
        Some("adopt") => {
            let mut log = Log::open(&args.dir()?).map_err(err)?;
            let path = args.pos.get(2).ok_or("missing <pack-file>")?;
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let r = log.adopt(args.need("as")?, &bytes, Object::new(), args.time()?).map_err(err)?;
            println!("{}", show(&r));
        }
        Some("seal") => {
            let mut log = Log::open(&args.dir()?).map_err(err)?;
            println!("sealed at {}", log.seal(args.time()?).map_err(err)?);
        }
        Some("close") => {
            let mut log = Log::open(&args.dir()?).map_err(err)?;
            let upto = args.upto()?.unwrap_or_else(|| log.last_address());
            let e = close_entry(&log, args.need("book")?, upto, args.time()?);
            let r = log.submit(args.need("as")?, e, args.time()?).map_err(err)?;
            println!("{}", show(&r));
        }
        Some("verify") => {
            let log = Log::open(&args.dir()?).map_err(err)?;
            let mut problems = log.verify();
            problems.extend(verify_closes(&log).into_iter().map(|a| format!("close at {a} no longer matches the balances")));
            for p in &problems {
                println!("problem: {p}");
            }
            if problems.is_empty() {
                println!("ok: {} entries, seals and closes verify", log.entries().len());
                return Ok(ExitCode::SUCCESS);
            }
            return Ok(ExitCode::from(1));
        }
        Some("view") => {
            let log = Log::open(&args.dir()?).map_err(err)?;
            let upto = args.upto()?;
            match args.pos.get(2).map(String::as_str) {
                Some("entries") => {
                    for (a, e) in log.entries().iter().filter(|(a, _)| upto.is_none_or(|u| *a <= u)) {
                        println!("{a}\t{}", e.to_line());
                    }
                }
                Some("register") => {
                    for d in log.register().all() {
                        println!("{}\t{}\t{}\t{}", d.kind, d.id, d.addr, d.pack.as_deref().unwrap_or("-"));
                    }
                }
                Some("gaps") => {
                    for (rec, missing) in log.gaps() {
                        println!("{rec}\t{missing:?}");
                    }
                }
                Some("balances") => {
                    for ((book, acct, unit), b) in balances(&log, upto) {
                        println!("{book}\t{acct}\t{unit}\t{}{}", if b.estimated { "~" } else { "" }, b.q);
                    }
                }
                Some("spans") => {
                    let (sp, anomalies) = spans(&log);
                    for s in sp {
                        let wall = s.wall().map(|w| format!("{w}s")).unwrap_or_else(|| "open".into());
                        println!("{}\t{}\t{}\t{}\t{wall}", s.run, s.verb.name(), s.object, s.actor);
                    }
                    for a in anomalies {
                        println!("anomaly\t{a:?}");
                    }
                }
                Some("attribution") => {
                    let conserved = ["xs", "J", "B", "wB", "ti", "to", "tr", "tw", "#", "kg", "USD", "h"];
                    for (run, a) in attribution(&log, &conserved) {
                        let rem: Vec<String> = a.remainder.iter().map(|(k, (q, e))| format!("{}{q}{k}", if *e { "~" } else { "" })).collect();
                        println!("{run}\twall={}s\tremainder={}\tutil={:?}\tpar={:?}", a.wall, rem.join(","), a.utilisation, a.parallelism);
                    }
                }
                _ => return Err("view what? entries|register|gaps|balances|spans|attribution".into()),
            }
        }
        Some("encode") => {
            let text = read_input(args.pos.get(1))?;
            let lines: Vec<String> = text.lines().filter(|l| !l.is_empty()).map(String::from).collect();
            print!("{}", stream_codec::encode(&lines).map_err(|e| e.0)?);
        }
        Some("decode") => {
            let text = read_input(args.pos.get(1))?;
            for l in stream_codec::decode(&text).map_err(|e| e.0)? {
                println!("{l}");
            }
        }
        _ => {
            print!("{USAGE}");
            return Ok(ExitCode::from(64));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run(Args::parse(std::env::args().skip(1).collect())) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("stream: {e}");
            ExitCode::from(64)
        }
    }
}
