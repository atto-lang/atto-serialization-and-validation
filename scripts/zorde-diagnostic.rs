//! Standalone Rust candidate for the combined native visibility evidence gate.
//! This recognizes diagnostic syntax, not compiler identity or guest execution.
use std::io::Read;
use std::path::Path;

const MAX_LOG: usize = 65536;
const MAX_LINE: usize = 4096;
const ENTRY: &str = "ZordePrivateCompiledEntry";
const OWNER: &str = "ZordeScalarRegistry";
const MEMBER: &str = "entries";

fn fixture_valid(f: &str) -> bool {
    matches!(f, "private-entry-import" | "private-storage-access")
}

// Accept only well-formed SGR escapes. Never remove arbitrary CSI sequences
// or an escape embedded inside a token; that could manufacture a subject.
fn normalize(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if out.chars().last().is_some_and(|p| p.is_alphanumeric() || p == '_') {
                return None;
            }
            if chars.next() != Some('[') { return None; }
            let mut n = 0;
            loop {
                match chars.next()? {
                    'm' => break,
                    '0'..='9' | ';' if n < 32 => n += 1,
                    _ => return None,
                }
            }
        } else if c == '\r' {
            if chars.peek() != Some(&'\n') { return None; }
        } else if (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{2028}' | '\u{2029}') {
            return None;
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn unquote(s: &str) -> Option<&str> {
    let first = s.chars().next()?;
    if matches!(first, '\'' | '"' | '\x60') {
        if s.len() < 2 || !s.ends_with(first) { return None; }
        let inner = &s[1..s.len()-1];
        if inner.contains(['\'','"','\x60']) { return None; }
        Some(inner)
    } else if s.contains(['\'','"','\x60']) {
        None
    } else { Some(s) }
}
fn exact(s: &str, expected: &str) -> bool { unquote(s)==Some(expected) }
fn qualified(s: &str) -> bool {
    if exact(s,"ZordeScalarRegistry.entries") || exact(s,"ZordeScalarRegistry::entries") {
        return true;
    }
    for separator in [".","::"] {
        if let Some((a,b))=s.split_once(separator) {
            if exact(a,OWNER) && exact(b,MEMBER) { return true; }
        }
    }
    false
}
fn wording(tokens: &[&str], expected: &[&str]) -> bool {
    tokens.len()==expected.len() && tokens.iter().zip(expected).all(|(a,b)|a.eq_ignore_ascii_case(b))
}
fn suffix(t: &[&str]) -> bool {
    for p in [
        &["is","private"][..], &["has","private","access"],
        &["not","exported"], &["not","public"], &["not","visible"], &["not","accessible"],
        &["is","not","exported"], &["is","not","public"], &["is","not","visible"],
        &["is","not","accessible"], &["is","inaccessible"], &["cannot","be","accessed"],
    ] {
        if wording(t,p) { return true; }
    }
    false
}
fn descriptor(s: &str) -> bool {
    ["member","field","class","type","symbol","declaration","import"].iter().any(|v|s.eq_ignore_ascii_case(v))
}
fn prefix(t: &[&str]) -> bool {
    wording(t,&["cannot","access"]) || wording(t,&["cannot","import"])
        || wording(t,&["access","denied"]) || wording(t,&["access","denied","to"])
        || (t.len()==2 && t[0].eq_ignore_ascii_case("private") && descriptor(t[1]))
}
fn subject(t: &[&str], f: &str) -> bool {
    if f=="private-entry-import" {
        return t.len()==1 && exact(t[0],ENTRY);
    }
    if t.len()==1 { return qualified(t[0]); }
    // Owner/member association belongs to the very same grammatical subject.
    if t.len()==3 {
        return (exact(t[0],OWNER) && matches!(t[1], "."|"::"|"member"|"field") && exact(t[2],MEMBER))
            || (exact(t[0],MEMBER) && matches!(t[1],"of"|"in"|"on") && exact(t[2],OWNER));
    }
    false
}
fn denial(line: &str, f: &str) -> bool {
    let line = line.trim_matches([' ','\t']);
    let body = if line.get(..6).is_some_and(|p|p.eq_ignore_ascii_case("error:")) {
        line[6..].trim_matches([' ','\t'])
    } else { line };
    let body = body.strip_suffix('.').unwrap_or(body);
    let t:Vec<_>=body.split([' ','\t']).filter(|s|!s.is_empty()).collect();
    for i in 1..t.len() {
        if subject(&t[..i],f) && suffix(&t[i..]) { return true; }
        if prefix(&t[..i]) {
            let rest=&t[i..];
            if subject(rest,f) || (rest.len()>1 && descriptor(rest[0]) && subject(&rest[1..],f)) {
                return true;
            }
        }
    }
    false
}
/// Combined gate: strict single-line subject and owner binding. Intentionally
/// refuses ambiguous continuation-only or mixed failures. Real compiler wording
/// must be confirmed before replacing the existing production harness.
pub fn assess(raw: &[u8], fixture: &str) -> bool {
    if !fixture_valid(fixture) || raw.is_empty() || raw.len()>MAX_LOG || raw.contains(&0) {
        return false;
    }
    // Measure physical bytes before removing formatting.
    if raw.split(|b|*b==b'\n').any(|line| line.len()>MAX_LINE+1 || (line.len()==MAX_LINE+1 && !line.ends_with(b"\r"))) {
        return false;
    }
    let Ok(message)=std::str::from_utf8(raw) else { return false; };
    let Some(clean)=normalize(message) else { return false; };
    let lower=clean.to_ascii_lowercase();
    if ["syntax error","parse error","unexpected token","internal compiler error","internal error",
        "no such file","module not found","class not found","exception in thread","segmentation fault",
        "stack trace","panic"].iter().any(|bad|lower.contains(bad)) { return false; }
    let mut accepted=0;
    for line in clean.lines() {
        if denial(line,fixture) {
            accepted+=1;
        } else if line.trim_start_matches([' ','\t']).get(..6).is_some_and(|p|p.eq_ignore_ascii_case("error:")) {
            return false;
        }
    }
    accepted==1
}
fn read_log(path: &Path) -> std::io::Result<Vec<u8>> {
    let metadata=std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len()>MAX_LOG as u64 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,"unsafe or oversized log"));
    }
    let mut bytes=Vec::new();
    std::fs::File::open(path)?.take((MAX_LOG+1) as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn main() {
    let args:Vec<_>=std::env::args_os().collect();
    if args.len()!=3 { std::process::exit(2); }
    let Some(f)=args[2].to_str() else { std::process::exit(2); };
    if !fixture_valid(f) { std::process::exit(2); }
    let accepted=read_log(Path::new(&args[1])).is_ok_and(|raw|assess(&raw,f));
    if !accepted { std::process::exit(1); }
    println!("targeted visibility diagnostic verified: {f}");
}

#[cfg(test)]
mod tests {
    use super::*;
    const E:&str="private-entry-import";
    const S:&str="private-storage-access";
    #[test] fn exact_subject_and_owner() {
        for (line,f) in [
            ("error: ZordePrivateCompiledEntry is private",E),
            ("error: ZordePrivateCompiledEntry not exported",E),
            ("error: private class 'ZordePrivateCompiledEntry'\r\n",E),
            ("error: cannot import ZordePrivateCompiledEntry",E),
            ("error: cannot access member entries in ZordeScalarRegistry",S),
            ("error: ZordeScalarRegistry.\x60entries\x60 has private access",S),
            ("error: ZordeScalarRegistry.entries IS PRIVATE",S),
            ("ZordeScalarRegistry.entries is private",S),
            ("error: access denied to ZordeScalarRegistry::entries",S),
            ("error: entries of ZordeScalarRegistry not visible",S),
            ("\x1b[31merror: ZordePrivateCompiledEntry not public \x1b[0m",E),
        ] { assert!(assess(line.as_bytes(),f),"{line:?}"); }
    }
    #[test] fn malformed_subject_and_owner_refused() {
        for (line,f) in [
            ("error: ZordePrivateCompiledEntry unknown; Other is private",E),
            ("error: Other is private; ZordePrivateCompiledEntry unknown",E),
            ("error: ZordeprivateCompiledEntry is private",E),
            ("error: ZordePrivateCompiledEntry2 is private",E),
            ("error: preZordePrivateCompiledEntry is private",E),
            ("error: ZordePrivate'CompiledEntry is private",E),
            ("error: 'ZordePrivateCompiledEntry is private",E),
            ("error: 'ZordePrivateCompiledEntry\x60 is private",E),
            ("error: ZordePrivateCompiledEntry not found",E),
            ("error: entries is private",S),
            ("error: Other.entries is private",S),
            ("error: ZordeScalarRegistry unrelated; entries is private",S),
            ("error: cannot access member entries in Other",S),
            ("error: ZordeScalarRegistry.entriesHolder is private",S),
            ("error: ZordeScalarRegistry.ENTRIES is private",S),
            ("error: ZordeScalarRegistry.entr\u{0456}es is private",S),
            ("error: ZordeScalarRegistry.entries\u{00a0}is private",S),
            ("error: ZordeScalarRegistry.entries variable is private",S),
        ] { assert!(!assess(line.as_bytes(),f),"{line:?}"); }
    }
    #[test] fn mixed_failures_and_line_splices_refused() {
        for suffix in ["\nparse error","\nno such file","\nerror: Other is private",
                       "\nerror: ZordePrivateCompiledEntry is private"] {
            assert!(!assess(format!("error: ZordePrivateCompiledEntry is private{suffix}").as_bytes(),E));
        }
        for sep in ["\n","\r","\r\n","\u{0085}","\u{2028}","\u{2029}"] {
            assert!(!assess(format!("error: ZordeScalarRegistry.entries{sep}is private").as_bytes(),S));
        }
        assert!(!assess(b"error: private type\nZordePrivateCompiledEntry",E));
        assert!(!assess(b"error: ZordePrivate\x1b[31mCompiledEntry is private",E));
        assert!(!assess(b"error: ZordePrivateCompiledEntry is private\x1b[2J",E));
    }
    #[test] fn exact_byte_limits() {
        let subject=b"error: ZordePrivateCompiledEntry is private";
        let mut line=subject.to_vec();
        line.resize(MAX_LINE,b' ');
        assert!(assess(&line,E));
        line.push(b' ');
        assert!(!assess(&line,E));
        let mut log=b"note: ".to_vec();
        log.extend(std::iter::repeat_n(b'x',MAX_LINE-7));
        log.push(b'\n');
        log.extend(subject);
        assert!(assess(&log,E));
        for bad in [b"".as_slice(),b"\xff",b"\0",b"error: ZordePrivateCompiledEntry is private\0"] {
            assert!(!assess(bad,E));
        }
        assert!(!assess(subject,"unknown"));
        assert!(!assess(&vec![b'x';MAX_LOG+1],E));
    }
}
