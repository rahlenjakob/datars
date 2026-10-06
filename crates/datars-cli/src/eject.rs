//! `datars eject std/<recipe>`: copy a standard-library recipe into the project as editable
//! TypeScript (docs/07-extensibility.md — the control ladder's last rung). The copy gets a local id
//! (`@local/<name>/<name>`), so documents that use it embed its code as a package; unedited, it
//! renders exactly what the std recipe renders.

use std::path::Path;

/// The end of the balanced `{…}` block starting at the first `{` at or after `from`. Aware of
/// strings, comments and template literals with nested `${…}` code.
fn block_end(src: &str, from: usize) -> Option<usize> {
    #[derive(Clone, Copy, PartialEq)]
    enum Cx {
        /// Code; the brace depth at which this context began (for `${…}` inside templates).
        Code(i32),
        Template,
    }
    let b = src.as_bytes();
    let open = from + src[from..].find('{')?;
    let mut stack = vec![Cx::Code(0)];
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        match *stack.last()? {
            Cx::Template => match c {
                b'\\' => i += 1,
                b'`' => {
                    stack.pop();
                }
                b'$' if next == Some(b'{') => {
                    depth += 1;
                    stack.push(Cx::Code(depth));
                    i += 1;
                }
                _ => {}
            },
            Cx::Code(base) => match c {
                b'/' if next == Some(b'/') => {
                    i = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
                }
                b'/' if next == Some(b'*') => {
                    i = src[i + 2..].find("*/").map(|n| i + 2 + n + 1).unwrap_or(b.len());
                }
                b'"' | b'\'' => {
                    let q = c;
                    i += 1;
                    while i < b.len() && b[i] != q {
                        if b[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                b'`' => stack.push(Cx::Template),
                b'{' => depth += 1,
                b'}' => {
                    if base > 0 && depth == base {
                        stack.pop(); // back into the template
                    }
                    depth -= 1;
                    if depth == 0 && stack.len() == 1 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// Extract `name` from the std sources in `std_src` as a standalone module.
pub fn eject(std_src: &Path, name: &str) -> Result<String, String> {
    let mut files: Vec<_> = std::fs::read_dir(std_src).map_err(|e| format!("{}: {e}", std_src.display()))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ts")).collect();
    files.sort();
    let marker = format!("export const {name} = recipe<");
    let (file, src) = files.iter().find_map(|f| std::fs::read_to_string(f).ok().filter(|s| s.contains(&marker)).map(|s| (f.clone(), s))).ok_or_else(|| format!("no std recipe `{name}` (see `datars describe`)"))?;
    let start = src.find(&marker).ok_or("marker")?;
    let generic_end = src[start + marker.len()..].find('>').ok_or("recipe<…> without a type")? + start + marker.len();
    let params_ty = src[start + marker.len()..generic_end].trim().to_string();
    let end = block_end(&src, start).ok_or("unbalanced recipe")?;
    let end = end + src[end..].find(';').map(|i| i + 1).unwrap_or(0);
    let recipe_src = src[start..end].replace(&format!("id: \"@datars/std/{name}\""), &format!("id: \"@local/{name}/{name}\""));

    let mut out = format!(
        "// Ejected from @datars/std ({}) by `datars eject std/{name}`. It's yours now: edit freely.\n// Documents that use it embed this code as a package (id `@local/{name}/{name}`).\n\n",
        file.file_name().and_then(|f| f.to_str()).unwrap_or("")
    );
    // Imports: relative std modules come from the installed standard library.
    for line in src.lines().filter(|l| l.starts_with("import ")) {
        let l = line.replace("from \"./", "from \"@datars/std\"; // was \"./");
        let l = if l.contains("// was") { format!("{};", l.split(';').next().unwrap_or("")) } else { l };
        out.push_str(&l);
        out.push('\n');
    }
    out.push('\n');
    // The params type and the module's private helpers the recipe may use.
    if let Some(i) = src.find(&format!("export interface {params_ty} ")) {
        if let Some(e) = block_end(&src, i) {
            out.push_str(&src[i..e]);
            out.push_str("\n\n");
        }
    } else if let Some(i) = src.find(&format!("type {params_ty} ")) {
        let e = src[i..].find(";\n").map(|e| i + e + 1).unwrap_or(i);
        out.push_str(&src[i..e]);
        out.push_str("\n\n");
    }
    for (i, line) in src.lines().scan(0usize, |pos, l| {
        let p = *pos;
        *pos += l.len() + 1;
        Some((p, l))
    }) {
        if line.starts_with("const ") && !line.contains("recipe<") {
            out.push_str(line);
            out.push('\n');
        } else if line.starts_with("function ") {
            if let Some(e) = block_end(&src, i) {
                out.push_str(&src[i..e]);
                out.push('\n');
            }
        } else if line.starts_with("type ") && !line.starts_with(&format!("type {params_ty} ")) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(&recipe_src);
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::block_end;

    #[test]
    fn blocks_skip_strings_comments_and_template_code() {
        let src = "x = f({ a: \"}\", // don't }\n b: `t ${ {c: 1}.c } }`, /* } */ d: { e: '}' } }); rest";
        let end = block_end(src, 0).unwrap();
        assert_eq!(&src[end..], "); rest");
    }
}
