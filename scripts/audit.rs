use regex::Regex;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

fn code_only(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            out.push(' ');
            continue;
        }
        if matches!(ch, '\'' | '"' | '\x60') {
            quote = Some(ch);
            out.push(' ');
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            break;
        }
        out.push(ch);
    }
    out
}

fn collect_ores(base: &Path, out: &mut Vec<PathBuf>) {
    if !base.exists() {
        return;
    }
    for entry in fs::read_dir(base).expect("read source root") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect_ores(&path, out);
        } else if path.extension().is_some_and(|e| e == "ores") {
            out.push(path);
        }
    }
}

fn imported_names(raw: &str) -> Vec<&str> {
    let mut raw = raw.trim();
    if raw == "*" {
        return Vec::new();
    }
    if raw.starts_with('(') && raw.ends_with(')') {
        raw = &raw[1..raw.len() - 1];
    }
    raw.split(',')
        .filter_map(|part| {
            let token = part.trim();
            if token.is_empty() {
                return None;
            }
            Some(token.split_once(" as ").map_or(token, |(name, _)| name.trim()))
        })
        .collect()
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(p) => out.push(p),
        }
    }
    out
}

fn declared(kind: &str, name: &str, text: &str) -> bool {
    let n = regex::escape(name);
    let pattern = match kind {
        "class" => format!(r"\b(?:pub\s+define\s+class|define\s+(?:pub\s+)?class)\s+{n}\b"),
        "fnc" => format!(r"\b(?:pub\s+)?(?:async\s+|nlex\s+|pure\s+)*fnc\s+{n}\b"),
        "interface" => format!(r"\b(?:pub\s+define\s+interface|define\s+(?:pub\s+)?interface)\s+{n}\b"),
        "trait" => format!(r"\b(?:pub\s+define\s+trait|define\s+(?:pub\s+)?trait)\s+{n}\b"),
        "struct" => format!(r"\b(?:pub\s+define\s+struct|define\s+(?:pub\s+)?struct)\s+{n}\b"),
        "types" => format!(r"\b(?:type|define\s+(?:pub\s+)?(?:class|interface|trait|struct))\s+{n}\b"),
        _ => return true,
    };
    Regex::new(&pattern).expect("declaration regex").is_match(text)
}

fn fail(errors: &mut Vec<String>, root: &Path, path: &Path, line_no: usize, message: impl AsRef<str>) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    errors.push(format!("{}:{line_no}: {}", rel.display(), message.as_ref()));
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for rel in ["src", "tests", "examples"] {
        collect_ores(&root.join(rel), &mut files);
    }
    files.sort();

    let import_re = Regex::new(
        r#"^\s*import\s+(class|fnc|interface|trait|struct|types)\s+(.+?)\s+from\s+['"]([^'"]+)['"]\s*;\s*$"#,
    )
    .unwrap();
    let amp_mut = Regex::new(r"&\s*mut\b").unwrap();
    let amp_unary = Regex::new(r"(?:^|[=(,:;<\[]|\breturn\s+)\s*&\s*[A-Za-z_][A-Za-z0-9_]*\b").unwrap();
    let ptr_decl_upper = Regex::new(r"\b[A-Z][A-Za-z0-9_<>,]*\*\s*[A-Za-z_][A-Za-z0-9_]*\b").unwrap();
    let ptr_decl_builtin = Regex::new(r"\b(?:int|float|bool|String|string)\s*\*\s*[A-Za-z_][A-Za-z0-9_]*\b").unwrap();
    let ptr_deref = Regex::new(r"(?:^|[=(,:;])\s*\*[A-Za-z_][A-Za-z0-9_]*\b").unwrap();
    let callable_channel = Regex::new(r"Channel\s*<[^>]*(?:Fnc|routine|->)").unwrap();

    let mut errors = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("read Oreslang source");
        for (line_index, raw) in text.lines().enumerate() {
            let line_no = line_index + 1;
            let code = code_only(raw);

            if amp_mut.is_match(&code) {
                fail(&mut errors, root, path, line_no, "Rust-style &mut ownership syntax is forbidden");
            }
            if amp_unary.is_match(&code) {
                fail(&mut errors, root, path, line_no, "pointer-style unary/type ampersand ownership syntax is forbidden; use rt borrow/take/copy/share");
            }
            if ptr_decl_upper.is_match(&code) || ptr_decl_builtin.is_match(&code) {
                fail(&mut errors, root, path, line_no, "C-style pointer declaration syntax is forbidden");
            }
            if ptr_deref.is_match(&code) {
                fail(&mut errors, root, path, line_no, "unary pointer dereference syntax is forbidden");
            }
            if callable_channel.is_match(&code) {
                fail(&mut errors, root, path, line_no, "callable/executable channel payload is forbidden");
            }

            let Some(caps) = import_re.captures(raw) else { continue };
            let kind = caps.get(1).unwrap().as_str();
            let raw_names = caps.get(2).unwrap().as_str();
            let target = caps.get(3).unwrap().as_str();
            if !target.starts_with('.') {
                continue;
            }

            let mut target_path = normalize(&path.parent().unwrap().join(target));
            if target_path.extension().is_none_or(|e| e != "ores") {
                target_path.set_extension("ores");
            }
            if !target_path.starts_with(root) {
                fail(&mut errors, root, path, line_no, format!("relative import escapes repository: {target}"));
                continue;
            }
            if !target_path.is_file() {
                fail(&mut errors, root, path, line_no, format!("relative import target does not exist: {target}"));
                continue;
            }

            let target_text = fs::read_to_string(&target_path).expect("read import target");
            for name in imported_names(raw_names) {
                if !declared(kind, name, &target_text) {
                    let rel = target_path.strip_prefix(root).unwrap_or(&target_path);
                    fail(
                        &mut errors,
                        root,
                        path,
                        line_no,
                        format!("imported {kind} {name:?} is not declared in {}", rel.display()),
                    );
                }
            }
        }
    }

    if !errors.is_empty() {
        eprintln!("static audit failed:");
        for error in errors {
            eprintln!("  {error}");
        }
        std::process::exit(1);
    }

    println!(
        "static audit passed: {} Oreslang source/test/example files",
        files.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_and_comments_are_removed_before_pointer_checks() {
        assert_eq!(code_only(r#"x = "&mut nope"; // &mut nope"#), "x =            ; ");
    }

    #[test]
    fn imported_name_aliases_preserve_source_name() {
        assert_eq!(imported_names("(Foo as Bar, Baz)"), vec!["Foo", "Baz"]);
    }
}
