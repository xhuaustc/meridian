//! Test-only scaffolding shared by the config-engine tests.
//!
//! Two pieces live here:
//!
//! - [`SidecarHarness`] lays out, in a throw-away prefix, the directory tree the
//!   generated config points at, then hands the whole thing to the nginx
//!   sidecar this repository ships (`scripts/prepare-nginx.sh`). Config
//!   assertions can therefore end in "the real nginx accepted it" instead of
//!   string matching alone.
//! - Invariant helpers over generated config *text*: count a directive inside a
//!   named block, assert no `location` block declares `add_header`, and assert
//!   every `add_header` carries the `always` flag so it also covers error
//!   responses.
//!
//! Nothing here is compiled into the shipped app.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::main_config;
use crate::nginx_manager::{parse_capabilities, Capabilities};
use crate::request_params::RequestDefaults;
use crate::store::models::{AccessList, AccessRule, Certificate, PortConflict, ProxyRule};

/// Prefix-relative directories the generated config points at. nginx has to find
/// them already present before `nginx -t`, otherwise it fails while setting up
/// its temp paths.
const PREFIX_DIRECTORIES: &[&str] = &[
    "logs",
    "conf.d",
    "stream.d",
    "temp/client_body",
    "temp/proxy",
    "html",
];

/// Print a skip line that survives libtest's output capture.
///
/// `eprintln!` from a *passing* test is swallowed unless the suite runs with
/// `--nocapture`, which would make a skipped sidecar test indistinguishable from
/// one that ran and passed. Writing to stderr directly bypasses that capture, so
/// the skip and its reason show up in every `cargo test` run.
pub fn skip(reason: &str) {
    use std::io::Write;

    let mut stderr = std::io::stderr();
    let _ = writeln!(
        stderr,
        "SKIP nginx-sidecar test: {reason}; run scripts/prepare-nginx.sh to build the sidecar"
    );
    let _ = stderr.flush();
}

/// The nginx binary this repository ships for the host platform.
///
/// Looks under `src-tauri/binaries/` for the Tauri sidecar output of
/// `scripts/prepare-nginx.sh` and only accepts names mentioning the host
/// architecture and OS: the release matrix writes several target triples into
/// that directory, and running the wrong one fails with "bad CPU type" instead
/// of a useful assertion.
pub fn repo_sidecar() -> Option<PathBuf> {
    let binaries = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&binaries)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("nginx-") && host_matches(name))
                && path.is_file()
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

fn host_matches(file_name: &str) -> bool {
    file_name.contains(std::env::consts::ARCH) && file_name.contains(host_os_token())
}

fn host_os_token() -> &'static str {
    if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// Ask a specific nginx binary what it supports (`nginx -V`), using the same
/// parser the production capability probe uses.
pub fn capabilities_of(sidecar: &Path) -> Capabilities {
    let output = Command::new(sidecar)
        .arg("-V")
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "could not run the nginx sidecar at {}: {error}; \
                 rebuild it with scripts/prepare-nginx.sh",
                sidecar.display()
            )
        });
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_capabilities(&text)
}

/// A throw-away nginx prefix plus the sidecar that validates it.
///
/// The prefix is `<temp>/meridian-config-test-<uuid>/nginx`, the same layout the
/// app uses at runtime (`<data_dir>/nginx`), so generated config that hardcodes
/// paths still resolves. `data_dir()` is what the production generators expect;
/// files are written relative to `prefix()`.
pub struct SidecarHarness {
    sidecar: PathBuf,
    data_dir: PathBuf,
    prefix: PathBuf,
}

impl SidecarHarness {
    /// Create a harness for the repository sidecar.
    ///
    /// Returns `None` and prints a visible skip line when the sidecar is absent.
    pub fn new() -> Option<Self> {
        let Some(sidecar) = repo_sidecar() else {
            skip(&format!(
                "no nginx-* sidecar for {}-{} under {}",
                std::env::consts::ARCH,
                host_os_token(),
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("binaries")
                    .display()
            ));
            return None;
        };
        Some(Self::for_sidecar(sidecar))
    }

    /// Build a scratch prefix, populate it with `build`, and let the repository
    /// sidecar judge it with `nginx -t`.
    ///
    /// Returns the closure's value when nginx accepts the config, and `None`
    /// after printing the reason when the sidecar has not been built, so every
    /// test stays explicit about the skip:
    /// `let Some(..) = SidecarHarness::with_configs(..) else { return };`.
    pub fn with_configs<T>(build: impl FnOnce(&SidecarHarness) -> T) -> Option<T> {
        let harness = Self::new()?;
        let value = build(&harness);
        harness.assert_accepts();
        Some(value)
    }

    fn for_sidecar(sidecar: PathBuf) -> Self {
        let data_dir =
            std::env::temp_dir().join(format!("meridian-config-test-{}", uuid::Uuid::new_v4()));
        let prefix = data_dir.join("nginx");
        for relative in PREFIX_DIRECTORIES {
            std::fs::create_dir_all(prefix.join(relative))
                .unwrap_or_else(|error| panic!("could not create the test nginx prefix: {error}"));
        }
        let harness = Self {
            sidecar,
            data_dir,
            prefix,
        };
        harness.ensure_include_targets();
        harness
    }

    /// The `data_dir` the production generators take.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The nginx prefix: where `nginx.conf` lives and what `-p` points at.
    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// The sidecar that judges these configs.
    pub fn sidecar(&self) -> &Path {
        &self.sidecar
    }

    /// `nginx -V` of the sidecar, parsed by the production parser.
    pub fn capabilities(&self) -> Capabilities {
        capabilities_of(self.sidecar())
    }

    /// Write `contents` to a prefix-relative path, creating parent directories.
    pub fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.prefix().join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", parent.display()));
        }
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
        path
    }

    /// Read a prefix-relative file back.
    pub fn read(&self, relative: &str) -> String {
        let path = self.prefix().join(relative);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
    }

    /// Write the real generated `nginx.conf` for these capabilities.
    pub fn write_main_config(&self, worker_processes: &str, capabilities: Capabilities) -> String {
        let config =
            main_config::generate_main_config(self.data_dir(), worker_processes, capabilities);
        self.write("nginx.conf", &config);
        config
    }

    /// Run the production generator into this prefix.
    pub fn generate_all(
        &self,
        rules: &[ProxyRule],
        certs: &[Certificate],
        access_lists: &[(AccessList, Vec<AccessRule>)],
        worker_processes: &str,
        capabilities: Capabilities,
    ) -> Vec<PortConflict> {
        super::generate_all_configs_with_settings(
            self.data_dir(),
            rules,
            certs,
            access_lists,
            worker_processes,
            &RequestDefaults::builtin(),
            capabilities,
        )
        .expect("config generation failed")
    }

    /// Run the sidecar over the prefix, panicking with nginx's own diagnostics
    /// when it rejects the config.
    pub fn assert_accepts(&self) -> String {
        self.ensure_include_targets();
        let output = self.run_nginx_test();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            output.status.success(),
            "the nginx sidecar rejected the generated config\n  sidecar: {}\n  conf:    {}\n  stderr:  {stderr}",
            self.sidecar.display(),
            self.prefix().join("nginx.conf").display(),
        );
        // Visible with `cargo test -- --nocapture`: records *which* binary judged
        // the config, so "nginx really ran" can be verified, not assumed.
        eprintln!(
            "nginx accepted {} with {}",
            self.prefix().display(),
            self.sidecar.display()
        );
        stderr
    }

    fn run_nginx_test(&self) -> Output {
        Command::new(self.sidecar())
            .arg("-t")
            .arg("-c")
            .arg(self.prefix().join("nginx.conf"))
            .arg("-p")
            .arg(self.prefix())
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "could not run the nginx sidecar at {}: {error}; \
                     rebuild it with scripts/prepare-nginx.sh",
                    self.sidecar.display()
                )
            })
    }

    /// nginx rejects an `include` glob that matches nothing on some builds and
    /// platforms, and config generation deletes every `*.conf` before writing
    /// the new ones. The placeholder therefore has to be re-created after
    /// generation, right before `nginx -t`.
    fn ensure_include_targets(&self) {
        for directory in ["conf.d", "stream.d"] {
            let dir = self.prefix().join(directory);
            let has_config = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "conf")
                });
            if !has_config {
                std::fs::write(
                    dir.join("00-harness-placeholder.conf"),
                    "# Placeholder so the generated `include *.conf` is not an empty glob.\n",
                )
                .expect("could not write the include placeholder");
            }
        }
    }
}

impl Drop for SidecarHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

/// A minimal HTTP proxy rule: plain HTTP, no TLS, proxying to loopback.
///
/// Config tickets only need "a rule that generates a server block"; use the
/// fields directly for the shape under test.
pub fn http_rule(id: &str, domain: &str) -> ProxyRule {
    ProxyRule {
        id: id.to_string(),
        name: format!("test {domain}"),
        proxy_type: "http".to_string(),
        enabled: true,
        listen_port: 18080,
        listen_host: "0.0.0.0".to_string(),
        domain: Some(domain.to_string()),
        path_prefix: None,
        upstream_host: "127.0.0.1".to_string(),
        upstream_port: 18081,
        upstream_scheme: "http".to_string(),
        tls_mode: "none".to_string(),
        certificate_id: None,
        access_list_id: None,
        websocket: false,
        keep_alive: false,
        force_https: false,
        custom_headers: None,
        upstream_targets: None,
        request: Default::default(),
        sort_order: 0,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

/// A minimal TCP stream proxy rule.
pub fn stream_rule(id: &str, listen_port: u16) -> ProxyRule {
    ProxyRule {
        proxy_type: "stream_tcp".to_string(),
        listen_port,
        domain: None,
        upstream_port: 15432,
        ..http_rule(id, "")
    }
}

// --- config text invariants ------------------------------------------------
//
// The helpers below work on generated config *text*. They are deliberately
// small: enough structure to scope a count to a block and to catch the header
// regressions this project cares about, not a full nginx parser.

/// A directive or a nested block inside a parsed config.
#[derive(Debug, Clone)]
enum Node<'a> {
    /// A directive statement, trimmed, without its trailing `;`.
    Directive(&'a str),
    Block(ConfigBlock<'a>),
}

/// One block of an nginx config, e.g. `server { ... }` or `location /api { ... }`.
#[derive(Debug, Clone)]
pub struct ConfigBlock<'a> {
    header: &'a str,
    body: &'a str,
    nodes: Vec<Node<'a>>,
}

impl<'a> ConfigBlock<'a> {
    /// The trimmed header that opened the block, e.g. `server` or `location /api`.
    pub fn header(&self) -> &'a str {
        self.header
    }

    /// The raw text between this block's braces, nested blocks included.
    pub fn body(&self) -> &'a str {
        self.body
    }

    /// Directives written directly in this block, in source order.
    pub fn directives(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            Node::Directive(statement) => Some(*statement),
            Node::Block(_) => None,
        })
    }

    /// Blocks written directly inside this block.
    pub fn blocks(&self) -> impl Iterator<Item = &ConfigBlock<'a>> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            Node::Block(block) => Some(block),
            Node::Directive(_) => None,
        })
    }

    /// How many times `directive` appears directly in this block.
    ///
    /// Directives in nested blocks are not counted; use
    /// [`ConfigBlock::count_recursive`] for that.
    pub fn count(&self, directive: &str) -> usize {
        self.directives()
            .filter(|statement| directive_name(statement) == directive)
            .count()
    }

    /// How many times `directive` appears in this block or any nested block.
    pub fn count_recursive(&self, directive: &str) -> usize {
        self.count(directive)
            + self
                .blocks()
                .map(|block| block.count_recursive(directive))
                .sum::<usize>()
    }

    /// Whether `directive` appears directly in this block.
    pub fn has(&self, directive: &str) -> bool {
        self.count(directive) > 0
    }
}

/// Parse `config` and return every block whose header starts with `header`.
///
/// `header` may be a bare block name (`"server"`, `"location"`) or a name plus
/// arguments (`"location /api"`). Depth-first order.
pub fn blocks<'a>(config: &'a str, header: &str) -> Vec<ConfigBlock<'a>> {
    let mut found = Vec::new();
    collect_blocks(&parse_nodes(config), header, &mut found);
    found
}

/// The first block whose header starts with `header`, depth-first.
pub fn block<'a>(config: &'a str, header: &str) -> Option<ConfigBlock<'a>> {
    blocks(config, header).into_iter().next()
}

/// Every directive named `name` in `config`, at any depth, without its `;`.
pub fn directives_named<'a>(config: &'a str, name: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    collect_directives(&parse_nodes(config), name, &mut found);
    found
}

/// Count `directive` across all blocks whose header starts with `header`.
///
/// This is the "how many times does the generator write X inside a server (or
/// location) block" assertion: only directives written directly in the matching
/// blocks are counted, so a nested `location` cannot inflate a server-level
/// count.
pub fn count_directive_in_blocks(config: &str, header: &str, directive: &str) -> usize {
    blocks(config, header)
        .iter()
        .map(|block| block.count(directive))
        .sum()
}

/// Assert that no `location` block declares `add_header` itself.
///
/// nginx header inheritance is not additive: a `location` that declares any
/// `add_header` drops every header set in the enclosing `server` block. Keeping
/// headers at server level is what stops "we added one header here" from
/// silently removing other headers elsewhere.
pub fn assert_no_add_header_in_locations(config: &str) {
    let offenders: Vec<&str> = blocks(config, "location")
        .iter()
        .filter(|block| block.count_recursive("add_header") > 0)
        .map(|block| block.header())
        .collect();
    assert!(
        offenders.is_empty(),
        "location blocks must not declare `add_header`: doing so silently drops the \
         server-level headers for that location; offending block(s): {}",
        offenders.join(", ")
    );
}

/// Assert that every `add_header` in `config` carries the `always` flag.
///
/// Without `always`, nginx only applies a header to 2xx/3xx responses, so error
/// pages silently lose headers the operator configured. Vacuously true when the
/// config has no `add_header` at all.
pub fn assert_every_add_header_applies_to_error_responses(config: &str) {
    for statement in directives_named(config, "add_header") {
        assert!(
            statement.split_whitespace().last() == Some("always"),
            "`add_header` without the `always` flag only reaches 2xx/3xx responses, \
             so the header silently disappears from error pages: `{statement}`"
        );
    }
}

fn directive_name(statement: &str) -> &str {
    statement.split_whitespace().next().unwrap_or_default()
}

fn header_matches(header: &str, wanted: &str) -> bool {
    header == wanted
        || header
            .strip_prefix(wanted)
            .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
}

fn collect_blocks<'a>(nodes: &[Node<'a>], wanted: &str, found: &mut Vec<ConfigBlock<'a>>) {
    for node in nodes {
        if let Node::Block(block) = node {
            if header_matches(block.header, wanted) {
                found.push(block.clone());
            }
            collect_blocks(&block.nodes, wanted, found);
        }
    }
}

fn collect_directives<'a>(nodes: &[Node<'a>], name: &str, found: &mut Vec<&'a str>) {
    for node in nodes {
        match node {
            Node::Directive(statement) if directive_name(statement) == name => {
                found.push(statement);
            }
            Node::Directive(_) => {}
            Node::Block(block) => collect_directives(&block.nodes, name, found),
        }
    }
}

fn parse_nodes(body: &str) -> Vec<Node<'_>> {
    let bytes = body.as_bytes();
    let mut nodes = Vec::new();
    let mut statement_start = 0;
    let mut in_comment = false;
    let mut comment_is_leading = false;
    let mut quote: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_comment {
            if byte == b'\n' {
                in_comment = false;
                if comment_is_leading {
                    // The comment was trivia before a statement, not part of one.
                    statement_start = index + 1;
                    comment_is_leading = false;
                }
            }
        } else if let Some(delimiter) = quote {
            if delimiter == b'"' && byte == b'\\' {
                index += 1; // Skip the escaped character.
            } else if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'#' => {
                    in_comment = true;
                    comment_is_leading = body[statement_start..index].trim().is_empty();
                }
                b'\'' | b'"' => quote = Some(byte),
                b';' => {
                    let statement = body[statement_start..index].trim();
                    if !statement.is_empty() {
                        nodes.push(Node::Directive(statement));
                    }
                    statement_start = index + 1;
                }
                b'{' => {
                    let header = body[statement_start..index].trim();
                    let end = matching_brace(body, index)
                        .unwrap_or_else(|| panic!("unbalanced `{{` after `{header}`"));
                    let inner = &body[index + 1..end];
                    nodes.push(Node::Block(ConfigBlock {
                        header,
                        body: inner,
                        nodes: parse_nodes(inner),
                    }));
                    index = end;
                    statement_start = index + 1;
                }
                // A stray `}` only shows up if the caller handed in a fragment
                // that starts inside a block; stop rather than loop forever.
                b'}' => break,
                _ => {}
            }
        }
        index += 1;
    }

    nodes
}

/// Index of the `}` matching the `{` at `open`, skipping quoted text and comments.
fn matching_brace(body: &str, open: usize) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut depth = 0usize;
    let mut in_comment = false;
    let mut quote: Option<u8> = None;
    let mut index = open;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_comment {
            if byte == b'\n' {
                in_comment = false;
            }
        } else if let Some(delimiter) = quote {
            if delimiter == b'"' && byte == b'\\' {
                index += 1;
            } else if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'#' => in_comment = true,
                b'\'' | b'"' => quote = Some(byte),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVER: &str = r#"
server {
    listen 80;
    server_name app.local;
    add_header X-Frame-Options DENY always;
    # add_header X-Commented-Out nope always;
    location / {
        add_header X-Only-Here yes always;
    }
}
"#;

    #[test]
    fn counts_only_the_directives_written_directly_in_the_block() {
        let server = block(SERVER, "server").expect("server block");
        assert_eq!(server.count("add_header"), 1);
        assert_eq!(server.count_recursive("add_header"), 2);
        assert_eq!(
            count_directive_in_blocks(SERVER, "location", "add_header"),
            1
        );
        assert_eq!(count_directive_in_blocks(SERVER, "server", "listen"), 1);
    }

    #[test]
    fn ignores_commented_out_directives() {
        let headers = directives_named(SERVER, "add_header");
        assert_eq!(headers.len(), 2);
        assert!(headers
            .iter()
            .all(|statement| !statement.contains("X-Commented-Out")));
    }

    #[test]
    fn finds_blocks_by_name_and_by_name_with_arguments() {
        assert_eq!(blocks(SERVER, "location").len(), 1);
        assert_eq!(
            block(SERVER, "location /")
                .expect("location block")
                .header(),
            "location /"
        );
        assert!(block(SERVER, "location /admin").is_none());
        assert!(block(SERVER, "server")
            .expect("server")
            .body()
            .contains("listen 80"));
        assert!(block(SERVER, "server").expect("server").has("server_name"));
    }

    #[test]
    fn quoted_values_do_not_confuse_the_block_scanner() {
        let config = r#"
server {
    add_header X-Json '{"a": "b; c"}' always;
    location / {
        # a `}` in a comment closes nothing
        return 200 "ok {not a block}";
    }
}
"#;
        let headers = directives_named(config, "add_header");
        assert_eq!(headers, vec![r#"add_header X-Json '{"a": "b; c"}' always"#]);
        let server = block(config, "server").expect("server block");
        assert_eq!(server.blocks().count(), 1);
        assert_eq!(server.count("return"), 0);
        assert_eq!(server.count_recursive("return"), 1);
    }

    #[test]
    fn accepts_add_headers_that_all_carry_the_always_flag() {
        let config =
            "server {\n    add_header X-A 1 always;\n    add_header X-B \"two words\" always;\n}";
        assert_every_add_header_applies_to_error_responses(config);
        assert_no_add_header_in_locations(config);
    }

    #[test]
    #[should_panic(expected = "always")]
    fn rejects_add_header_without_the_always_flag() {
        assert_every_add_header_applies_to_error_responses("server {\n    add_header X-A 1;\n}");
    }

    #[test]
    #[should_panic(expected = "location")]
    fn rejects_add_header_inside_a_location_block() {
        assert_no_add_header_in_locations(
            "server {\n    location / {\n        add_header X-A 1 always;\n    }\n}",
        );
    }

    #[test]
    #[should_panic(expected = "always")]
    fn does_not_mistake_a_quoted_always_for_the_flag() {
        assert_every_add_header_applies_to_error_responses(
            "server {\n    add_header X-A \"always\";\n}",
        );
    }
}
