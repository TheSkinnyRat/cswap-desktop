//! Release tool.
//!
//!   cswap-release keygen                         print a new key pair (for the release owner)
//!   cswap-release sign --version V FILE...       write FILE.sig with $CSWAP_UPDATE_SIGNING_KEY
//!   cswap-release verify --version V FILE...     check FILE.sig against the key compiled
//!                                                into this build (or --key B64)
//!   cswap-release legacy-feed --version V --out latest.yml FILE
//!                                                electron-updater feed for 0.1.x copies
//!
//! The signature covers the file's NAME, so sign the file under the name it is uploaded as.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cswap_update::{legacy, sign, time};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cswap-release: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    version: Option<String>,
    key: Option<String>,
    out: Option<PathBuf>,
    files: Vec<PathBuf>,
}

fn opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { version: None, key: None, out: None, files: vec![] };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--version" => o.version = Some(value()?.trim_start_matches('v').to_string()),
            "--key" => o.key = Some(value()?),
            "--out" => o.out = Some(PathBuf::from(value()?)),
            f if f.starts_with("--") => return Err(format!("unknown option {f}")),
            f => o.files.push(PathBuf::from(f)),
        }
    }
    if let Some(v) = &o.version {
        semver::Version::parse(v).map_err(|e| format!("--version {v}: {e}"))?;
    }
    Ok(o)
}

fn name_of(f: &Path) -> Result<String, String> {
    f.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("{} has no file name", f.display()))
}

fn digest(f: &Path) -> Result<[u8; 64], String> {
    sign::sha512_file(f).map_err(|e| format!("{}: {e}", f.display()))
}

fn run(args: &[String]) -> Result<(), String> {
    let (cmd, rest) = args.split_first().ok_or("usage: cswap-release keygen|sign|verify|legacy-feed ...")?;
    let o = opts(rest)?;
    let version = || o.version.clone().ok_or("--version is required");
    match cmd.as_str() {
        "keygen" => {
            let (private, public) = sign::keygen()?;
            println!("private (secret {}): {private}", sign::SIGNING_KEY_ENV);
            println!("public  (RELEASE_PUBLIC_KEY):        {public}");
            Ok(())
        }
        "sign" => {
            let v = version()?;
            let raw =
                std::env::var(sign::SIGNING_KEY_ENV).map_err(|_| format!("${} is not set", sign::SIGNING_KEY_ENV))?;
            let key = sign::signing_key(&raw)?;
            if o.files.is_empty() {
                return Err("no files to sign".into());
            }
            for f in &o.files {
                let sig = sign::sign(&key, &name_of(f)?, &v, &digest(f)?);
                let out = PathBuf::from(format!("{}.sig", f.display()));
                std::fs::write(&out, sig).map_err(|e| format!("{}: {e}", out.display()))?;
                println!("signed {} ({v}) with key {}", f.display(), sign::public_of(&key));
            }
            Ok(())
        }
        "verify" => {
            let v = version()?;
            let key = match &o.key {
                Some(k) => sign::public_key(k)?,
                None => sign::release_key()?,
            };
            if o.files.is_empty() {
                return Err("no files to verify".into());
            }
            for f in &o.files {
                let sig_path = PathBuf::from(format!("{}.sig", f.display()));
                let sig = std::fs::read_to_string(&sig_path).map_err(|e| format!("{}: {e}", sig_path.display()))?;
                sign::verify(&key, &name_of(f)?, &v, &digest(f)?, &sig)?;
                println!("ok {}", f.display());
            }
            Ok(())
        }
        "legacy-feed" => {
            let v = version()?;
            let out = o.out.clone().ok_or("--out is required")?;
            let [f] = o.files.as_slice() else { return Err("legacy-feed takes exactly one file".into()) };
            let size = std::fs::metadata(f).map_err(|e| format!("{}: {e}", f.display()))?.len();
            let yml = legacy::feed(&v, &name_of(f)?, &digest(f)?, size, &time::iso_now());
            std::fs::write(&out, yml).map_err(|e| format!("{}: {e}", out.display()))?;
            println!("wrote {}", out.display());
            Ok(())
        }
        c => Err(format!("unknown command {c}")),
    }
}
