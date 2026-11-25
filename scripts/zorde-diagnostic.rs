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

// Only SGR decorations surrounding a complete line may be removed.
// Interior escapes could splice a subject or a denial phrase; reject them.
fn strip_sgr(s: &str) -> Option<&str> {
    let rest=s.strip_prefix("\x1b[")?;
    for (i,b) in rest.bytes().enumerate() {
        match b {
            b'm' if i<=32 => return Some(&rest[i+1..]),
            b'0'..=b'9' | b';' if i<32 => (),
            _ => return None,
        }
    }
    None
}
fn normalize(raw: &str) -> Option<String> {
    let mut out=String::with_capacity(raw.len());
    for (i,physical) in raw.split('\n').enumerate() {
        if i>0 { out.push('\n'); }
        let mut line=physical.strip_suffix('\r').unwrap_or(physical);
        // A final bare CR is not CRLF; reject in assess before normalization.
        while line.starts_with('\x1b') { line=strip_sgr(line)?; }
        let text=if let Some(at)=line.find('\x1b') {
            let mut tail=&line[at..];
            while tail.starts_with('\x1b') { tail=strip_sgr(tail)?; }
            if !tail.bytes().all(|b|matches!(b,b' '|b'\t')) { return None; }
            &line[..at]
        } else { line };
        if text.chars().any(|c|
            (c.is_control() && c!='\t') || matches!(c,'\u{2028}'|'\u{2029}')) {
            return None;
        }
        out.push_str(text);
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
    if t.len()==2 {
        return [".","::"].iter().any(|sep|
            t[0].strip_suffix(sep).is_some_and(|owner|exact(owner,OWNER))
                && exact(t[1],MEMBER)
            || exact(t[0],OWNER) && t[1].strip_prefix(sep).is_some_and(|member|exact(member,MEMBER)));
    }
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
    if !fixture_valid(fixture) || raw.is_empty() || raw.len()>MAX_LOG || raw.contains(&0) || raw.ends_with(b"\r") {
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
    #[test] fn legacy_combined_gate_positive_matrix() {
        for (text,fixture) in [
            ("error: private class ZordePrivateCompiledEntry",E),
            ("error: private type \"ZordePrivateCompiledEntry\"",E),
            ("error: cannot access ZordePrivateCompiledEntry\n",E),
            ("error: cannot import \x60ZordePrivateCompiledEntry\x60",E),
            ("error: ZordePrivateCompiledEntry is not accessible",E),
            ("error: ZordePrivateCompiledEntry is inaccessible",E),
            ("error: ZordePrivateCompiledEntry cannot be accessed",E),
            ("error: ZordePrivateCompiledEntry has private access",E),
            ("error: ZordePrivateCompiledEntry\tIS PRIVATE\r\n",E),
            ("note: evaluating visibility\nerror: private class 'ZordePrivateCompiledEntry'",E),
            ("error: ZordeScalarRegistry. entries is private",S),
            ("error: ZordeScalarRegistry ::entries is private",S),
            ("error: ZordeScalarRegistry member entries is private",S),
            ("error: cannot access field 'entries' in ZordeScalarRegistry",S),
            ("error: access denied to entries on ZordeScalarRegistry",S),
            ("\x1b[31merror: ZordePrivateCompiledEntry not public\x1b[0m",E),
            ("\x1b[1m\x1b[31merror: ZordePrivateCompiledEntry not public\x1b[0m\x1b[39m\r\n",E),
        ] { assert!(assess(text.as_bytes(),fixture),"{text:?}"); }
    }
    #[test] fn legacy_rejections_and_intentional_stricter_outcomes() {
        // These include inputs accepted by the old unqualified same-line
        // verifier alone but refused by the combined owner-aware gate.
        for (text,fixture) in [
            ("error: cannot access member entries",S),
            ("error: field entries is private",S),
            ("error: access denied to 'entries'",S),
            ("error: ZordeScalarRegistry is private",S),
            ("error: private field entriesSuffix",S),
            ("error: reentries is private",S),
            ("error: entries_shadow is private",S),
            ("error: entries2 is private",S),
            ("error: preentries is private",S),
            ("error: entries unknown; Other is private",S),
            ("error: entries unexpected; cannot access Other",S),
            ("error: other is private, entries mentioned",S),
            ("warning: entries visible; Other is private",S),
            ("error: private class Other ZordePrivateCompiledEntry",E),
            ("error: ZordePrivateCompiledEntry not found; access denied to Other",E),
            ("error: cannot access Other; ZordePrivateCompiledEntry unknown",E),
            ("error: ZordePrivateCompiledEntry undefined identifier",E),
            ("error: unknown symbol ZordePrivateCompiledEntry",E),
            ("error: SomeOtherType is private",E),
            ("error: Other is private\nZordePrivateCompiledEntry",E),
            ("error: private type OtherClass\nignored\nwarning: ZordePrivateCompiledEntry",E),
            ("ZordePrivateCompiledEntry\nprivate class Unexpected",E),
            ("entries\nerror: symbol elsewhere is private",S),
            ("error: private type\nOtherClass\nZordePrivateCompiledEntry",E),
            ("error: Other field is private\nentries",S),
            ("error: ZordePrivateCompiledEntry is private",S),
            ("error: ZordeScalarRegistry.entries is private",E),
            ("error: ZordePrivateCompiledEntry is private\ninternal compiler error",E),
            ("error: ZordePrivateCompiledEntry is private\nexception in thread",E),
            ("error: ZordePrivateCompiledEntry is private\nsegmentation fault",E),
            ("error: ZordePrivateCompiledEntry is private\nmodule not found",E),
        ] { assert!(!assess(text.as_bytes(),fixture),"{text:?}"); }
    }
    #[test] fn decoration_must_not_manufacture_a_token_or_a_line() {
        for text in [
            "error: ZordePrivate\x1b[31mCompiledEntry is private",
            "error: \x1b[31mZordePrivateCompiledEntry is private",
            "error: ZordePrivateCompiledEntry \x1b[31mis private",
            "error: ZordePrivateCompiledEntry is private\x1b[0m extra",
            "error: ZordePrivateCompiledEntry is private\x1b[",
            "error: ZordePrivateCompiledEntry is private\x1b[2J",
            "error: ZordePrivateCompiledEntry is private\r",
            "error: ZordePrivateCompiledEntry is private\rerror: Other",
            "error: ZordePrivateCompiledEntry is private\u{0085}",
        ] { assert!(!assess(text.as_bytes(),E),"{text:?}"); }
    }
    #[test] fn exact_physical_total_and_multibyte_limits() {
        let subject=b"error: ZordePrivateCompiledEntry is private";
        let mut log=Vec::new();
        while log.len()+MAX_LINE+1+subject.len()<=MAX_LOG {
            log.extend(std::iter::repeat_n(b'x',MAX_LINE));
            log.push(b'\n');
        }
        let remaining=MAX_LOG-log.len()-subject.len();
        if remaining>0 {
            log.extend(std::iter::repeat_n(b'x',remaining-1));
            log.push(b'\n');
        }
        log.extend(subject);
        assert_eq!(log.len(),MAX_LOG);
        assert!(assess(&log,E));
        log.push(b' ');
        assert!(!assess(&log,E));
        let note=format!("{}","é".repeat(2048));
        assert_eq!(note.len(),MAX_LINE);
        assert!(assess(format!("{note}\n{}",std::str::from_utf8(subject).unwrap()).as_bytes(),E));
        assert!(!assess(format!("{note}x\n{}",std::str::from_utf8(subject).unwrap()).as_bytes(),E));
        let mut crlf=subject.to_vec();
        crlf.resize(MAX_LINE,b' ');
        crlf.extend_from_slice(b"\r\n");
        assert!(assess(&crlf,E));
        assert!(!assess(b"error: ZordePrivateCompiledEntry is private\n\xff",E));
    }

}
