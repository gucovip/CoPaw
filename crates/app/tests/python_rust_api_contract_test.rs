use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Route {
    method: String,
    path: String,
    source: String,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

fn normalize_path(path: &str) -> String {
    let mut p = path.trim();
    if p.is_empty() {
        return "/".to_string();
    }
    if !p.starts_with('/') {
        let owned = format!("/{p}");
        return normalize_path(&owned);
    }

    // Keep root as-is, normalize others by segment.
    if p != "/" {
        p = p.trim_end_matches('/');
    }

    let mut out = Vec::new();
    for seg in p.split('/').filter(|s| !s.is_empty()) {
        let norm = if (seg.starts_with('{') && seg.ends_with('}'))
            || seg.starts_with(':')
            || seg.starts_with('*')
        {
            "{}"
        } else {
            seg
        };
        out.push(norm);
    }

    if out.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", out.join("/"))
    }
}

fn join_paths(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let path = path.trim();

    if path.is_empty() {
        return if prefix.is_empty() {
            "/".to_string()
        } else {
            prefix.to_string()
        };
    }

    if prefix.is_empty() {
        if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        }
    } else if path.starts_with('/') {
        format!("{prefix}{path}")
    } else {
        format!("{prefix}/{path}")
    }
}

fn line_number(content: &str, byte_pos: usize) -> usize {
    content[..byte_pos].bytes().filter(|b| *b == b'\n').count() + 1
}

fn extract_balanced_call_args(content: &str, open_paren_idx: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    let mut i = open_paren_idx;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut quote = b'\0';
    let mut escaped = false;
    let mut start = None;

    while i < bytes.len() {
        let b = bytes[i];

        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == quote {
                in_str = false;
                quote = b'\0';
            }
            i += 1;
            continue;
        }

        if b == b'\'' || b == b'"' {
            in_str = true;
            quote = b;
            i += 1;
            continue;
        }

        if b == b'(' {
            depth += 1;
            if depth == 1 {
                start = Some(i + 1);
            }
        } else if b == b')' {
            if depth == 1 {
                let s = start?;
                return Some((content[s..i].to_string(), i));
            }
            if depth > 0 {
                depth = depth.saturating_sub(1);
            }
        }

        i += 1;
    }

    None
}

fn split_top_level_first_comma(s: &str) -> Option<(String, String)> {
    let bytes = s.as_bytes();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut depth_brace = 0usize;
    let mut in_str = false;
    let mut quote = b'\0';
    let mut escaped = false;

    for (i, b) in bytes.iter().enumerate() {
        let b = *b;

        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == quote {
                in_str = false;
                quote = b'\0';
            }
            continue;
        }

        if b == b'\'' || b == b'"' {
            in_str = true;
            quote = b;
            continue;
        }

        match b {
            b'(' => depth_paren += 1,
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_bracket += 1,
            b']' => depth_bracket = depth_bracket.saturating_sub(1),
            b'{' => depth_brace += 1,
            b'}' => depth_brace = depth_brace.saturating_sub(1),
            b',' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                return Some((s[..i].trim().to_string(), s[i + 1..].trim().to_string()));
            }
            _ => {}
        }
    }

    None
}

fn python_router_prefix(content: &str) -> String {
    let re = Regex::new(r#"APIRouter\((?s).*?prefix\s*=\s*[\"']([^\"']*)[\"']"#).unwrap();
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

fn extract_python_routes_from_file(
    file: &Path,
    global_prefix: &str,
    router_prefix: &str,
) -> Vec<Route> {
    let content = fs::read_to_string(file).expect("failed reading python file");
    let re = Regex::new(r"(?m)^\s*@(?P<obj>app|router)\.(?P<method>get|post|put|delete|patch|api_route)\s*\(").unwrap();
    let string_arg_re = Regex::new(r#"^\s*[\"']([^\"']*)[\"']"#).unwrap();
    let kw_path_re = Regex::new(r#"\bpath\s*=\s*[\"']([^\"']*)[\"']"#).unwrap();
    let methods_re = Regex::new(r"\bmethods\s*=\s*\[([^\]]*)\]").unwrap();
    let method_token_re = Regex::new(r#"[\"']([A-Za-z]+)[\"']"#).unwrap();

    let mut routes = Vec::new();

    for caps in re.captures_iter(&content) {
        let m = caps.get(0).unwrap();
        let open_paren = m.end() - 1;
        let method = caps.name("method").unwrap().as_str();

        let Some((args, _end)) = extract_balanced_call_args(&content, open_paren) else {
            continue;
        };

        let raw_path = string_arg_re
            .captures(&args)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .or_else(|| {
                kw_path_re
                    .captures(&args)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            });

        let Some(raw_path) = raw_path else {
            continue;
        };

        let methods: Vec<String> = if method == "api_route" {
            let Some(methods_caps) = methods_re.captures(&args) else {
                continue;
            };
            method_token_re
                .captures_iter(methods_caps.get(1).unwrap().as_str())
                .filter_map(|c| c.get(1).map(|g| g.as_str().to_uppercase()))
                .collect()
        } else {
            vec![method.to_uppercase()]
        };

        if methods.is_empty() {
            continue;
        }

        let line = line_number(&content, m.start());
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(file)
            .display()
            .to_string();
        let base = format!("{}{}", global_prefix, router_prefix);
        let full_path = join_paths(&base, &raw_path);

        for http_method in methods {
            routes.push(Route {
                method: http_method,
                path: normalize_path(&full_path),
                source: format!("{}:{}", rel, line),
            });
        }
    }

    routes
}

fn collect_python_api_routes() -> Vec<Route> {
    let root = repo_root();
    let mut files: Vec<(PathBuf, String, bool)> = vec![
        (root.join("src/copaw/app/_app.py"), "".to_string(), false),
        (root.join("src/copaw/app/crons/api.py"), "/api".to_string(), true),
        (root.join("src/copaw/app/runner/api.py"), "/api".to_string(), true),
    ];

    let routers_dir = root.join("src/copaw/app/routers");
    let entries = fs::read_dir(&routers_dir).expect("failed reading routers dir");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("py")
            && path.file_name().and_then(|n| n.to_str()) != Some("__init__.py")
        {
            files.push((path, "/api".to_string(), true));
        }
    }

    let mut routes = Vec::new();

    for (file, global_prefix, has_router_prefix) in files {
        let content = fs::read_to_string(&file).expect("failed reading python route file");
        let router_prefix = if has_router_prefix {
            python_router_prefix(&content)
        } else {
            String::new()
        };

        routes.extend(extract_python_routes_from_file(
            &file,
            &global_prefix,
            &router_prefix,
        ));
    }

    routes
        .into_iter()
        .filter(|r| r.path == "/api" || r.path.starts_with("/api/"))
        .collect()
}

fn collect_rust_routes_from_file(file: &Path, mount_prefix: &str) -> Vec<Route> {
    let content = fs::read_to_string(file).expect("failed reading rust route file");
    let mut out = Vec::new();
    let method_re = Regex::new(r"\b(get|post|put|delete|patch)\s*\(").unwrap();
    let string_lit_re = Regex::new(r#"^\s*\"([^\"]*)\""#).unwrap();

    let mut search_start = 0usize;
    while let Some(rel_idx) = content[search_start..].find(".route(") {
        let start = search_start + rel_idx;
        let open_paren = start + ".route".len();
        let Some((args, end_idx)) = extract_balanced_call_args(&content, open_paren) else {
            break;
        };
        search_start = end_idx + 1;

        let Some((path_expr, handler_expr)) = split_top_level_first_comma(&args) else {
            continue;
        };

        let Some(path_caps) = string_lit_re.captures(&path_expr) else {
            continue;
        };

        let raw_path = path_caps.get(1).unwrap().as_str();
        let full_path = join_paths(mount_prefix, raw_path);

        let mut methods: BTreeSet<String> = BTreeSet::new();
        for caps in method_re.captures_iter(&handler_expr) {
            methods.insert(caps.get(1).unwrap().as_str().to_uppercase());
        }

        if methods.is_empty() {
            continue;
        }

        let line = line_number(&content, start);
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(file)
            .display()
            .to_string();

        for method in methods {
            out.push(Route {
                method,
                path: normalize_path(&full_path),
                source: format!("{}:{}", rel, line),
            });
        }
    }

    out
}

fn collect_rust_routes() -> Vec<Route> {
    let root = repo_root();
    let route_files: [(&str, &str); 12] = [
        ("crates/app/src/routes/config.rs", "/api"),
        ("crates/app/src/routes/models.rs", "/api/models"),
        ("crates/app/src/routes/cron.rs", "/api/cron"),
        ("crates/app/src/routes/mcp.rs", "/api/mcp"),
        ("crates/app/src/routes/envs.rs", "/api/envs"),
        ("crates/app/src/routes/skills.rs", "/api/skills"),
        ("crates/app/src/routes/local_models.rs", "/api/local-models"),
        ("crates/app/src/routes/ollama_models.rs", "/api/ollama-models"),
        ("crates/app/src/routes/workspace.rs", "/api/workspace"),
        ("crates/app/src/routes/console.rs", "/api/console"),
        ("crates/app/src/routes/agent.rs", "/api/agent"),
        ("crates/app/src/routes/chats.rs", "/api/chats"),
    ];

    let mut routes = Vec::new();
    for (path, mount_prefix) in route_files {
        routes.extend(collect_rust_routes_from_file(
            &root.join(path),
            mount_prefix,
        ));
    }

    routes.push(Route {
        method: "GET".to_string(),
        path: "/".to_string(),
        source: "crates/app/src/main.rs:326".to_string(),
    });
    routes.push(Route {
        method: "GET".to_string(),
        path: "/api/version".to_string(),
        source: "crates/app/src/main.rs:327".to_string(),
    });
    routes.push(Route {
        method: "GET".to_string(),
        path: "/logo.png".to_string(),
        source: "crates/app/src/main.rs:328".to_string(),
    });
    routes.push(Route {
        method: "GET".to_string(),
        path: "/copaw-symbol.svg".to_string(),
        source: "crates/app/src/main.rs:329".to_string(),
    });

    routes
}

#[test]
fn python_api_routes_have_semantic_match_in_rust() {
    let py_routes = collect_python_api_routes();
    let rs_routes = collect_rust_routes();

    let rs_set: HashSet<(String, String)> = rs_routes
        .into_iter()
        .map(|r| (r.method, normalize_path(&r.path)))
        .collect();

    let mut missing = Vec::new();

    for r in py_routes {
        let key = (r.method.clone(), normalize_path(&r.path));
        if !rs_set.contains(&key) {
            missing.push(format!("{} {} (from {})", r.method, r.path, r.source));
        }
    }

    missing.sort();

    assert!(
        missing.is_empty(),
        "Python API routes missing in Rust (semantic compare):\n{}",
        missing.join("\n")
    );
}
