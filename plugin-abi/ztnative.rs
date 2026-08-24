//! # `ztnative` — native plugin SDK for ztmux
//!
//! Every tmux plugin ever written is a shell script: TPM clones a repo, runs
//! its `*.tmux` file, and that file shells out to `tmux bind-key …`. This
//! crate is what makes ztmux the first terminal multiplexer that **hosts
//! third-party plugins written in a native compiled language** — a plugin is
//! an ordinary `cdylib` the server `dlopen`s, and what it registers are real
//! tmux commands, real `#{…}` format variables, and real hook subscriptions,
//! resolved inside the server with no subprocess, no `run-shell`, and no
//! `tmux` binary in the loop.
//!
//! The boundary between host and plugin is a hand-rolled, versioned **C ABI**
//! (`#[repr(C)]` structs + `extern "C"` fn pointers). Both sides depend on
//! THIS crate so they agree on the exact layout. Nothing about Rust's
//! unstable `repr(Rust)` layout, allocator, or panic ABI crosses the
//! boundary — only C-representable data. Strings only ever travel in the
//! direction of their allocator: host-allocated results come back to the host
//! through [`HostApi::free_cstring`], and a plugin hands its own strings over
//! through an [`EmitFn`] callback rather than by pointer hand-off.
//!
//! ## Writing a plugin
//!
//! ```ignore
//! use ztnative::{declare_plugin, Args, Ctx, Host};
//! use std::os::raw::c_int;
//!
//! fn hello(host: &Host, ctx: &Ctx, args: &Args) -> c_int {
//!     let who = ctx.arg("n").unwrap_or_else(|| "world".into());
//!     host.print(ctx, &format!("hello {who}, argv={:?}", args.rest()));
//!     0
//! }
//!
//! declare_plugin! {
//!     name: "hello",
//!     version: "0.1.0",
//!     commands: {
//!         "hello-world" => { alias: "hw", template: "n:", usage: "[-n name]", handler: hello },
//!     },
//! }
//! ```
//!
//! `Cargo.toml`:
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! [dependencies]
//! ztnative = "0.1"
//! ```
//!
//! `cargo build` produces `libhello.dylib` / `libhello.so`; then inside ztmux
//! `znative add owner/hello` installs it and `hello-world` is a live tmux
//! command — usable from the command prompt, a key binding, `.tmux.conf`, and
//! the CLI, exactly like `new-window`.
//!
//! ## Host API
//!
//! | Method | Purpose |
//! | --- | --- |
//! | [`print`](Host::print) / [`error`](Host::error) | write to the client that ran the command |
//! | [`run`](Host::run) | parse + queue a tmux command string |
//! | [`get_option`](Host::get_option) / [`set_option`](Host::set_option) | read/write an option, including the `@user` options plugins configure themselves with |
//! | [`format_expand`](Host::format_expand) | expand `#{…}` against the command's target |
//! | [`register_command`](Host::register_command) | add a tmux command |
//! | [`register_format`](Host::register_format) | provide a `#{…}` variable |
//! | [`register_hook`](Host::register_hook) | subscribe to a hook (`session-created`, `pane-exited`, …) |

// This file is compiled into the host AND copied into every plugin, so any one
// consumer uses only part of it: a plugin with no hooks never names `Hook`, the
// host never constructs `Args`. Unused-item lints on a shared header are noise,
// so the file states that once here rather than making every consumer repeat an
// `#[allow]` on the module.
#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

/// ABI version. Bumped on ANY change to [`HostApi`], [`PluginInfo`],
/// [`HookEvent`], [`CommandFn`], [`FormatFn`], [`HookFn`], or [`InitFn`]
/// layout/semantics. The host refuses to load a plugin whose `abi_version`
/// does not match its own — a mismatched struct layout is undefined
/// behaviour, so this is a hard gate, not a warning.
///
/// v2: [`FormatFn`] takes the opaque `ctx` a [`CommandFn`] gets, so a format
/// provider can call [`Host::format_expand`] against the tree being expanded —
/// without it a provider cannot see the client it is being drawn for, which is
/// most of what a status-line plugin needs.
pub const ABI_VERSION: u32 = 2;

/// The one symbol every plugin `cdylib` must export. The host resolves it
/// with `dlsym` after `dlopen`. Signature is [`InitFn`].
pub const INIT_SYMBOL: &[u8] = b"ztnative_init\0";

/// A plugin-provided tmux command.
///
/// * `host` — the host API table (call back into the server through it).
/// * `ctx`  — opaque handle to the running command: the queue item it prints
///   to and the parsed flags it was given. Pass it back to
///   [`HostApi::print`] / [`HostApi::arg_get`] / … Valid only for the
///   duration of the call.
/// * `argc` / `argv` — the command's **positional** arguments as
///   NUL-terminated C strings; `argv[0]` is the command name, `argv[1..]` the
///   arguments. Flags are read through [`HostApi::arg_has`] /
///   [`HostApi::arg_get`], since tmux parses them out per the command's
///   template.
///
/// Returns 0 for success; any other value makes the command fail (tmux's
/// `CMD_RETURN_ERROR`).
pub type CommandFn = extern "C" fn(
    host: *const HostApi,
    ctx: *mut c_void,
    argc: usize,
    argv: *const *const c_char,
) -> c_int;

/// Callback a [`FormatFn`] hands its result to. The text is copied by the
/// host before this returns, so a plugin never gives up ownership of memory
/// its own allocator made.
pub type EmitFn = extern "C" fn(sink: *mut c_void, text: *const c_char);

/// A plugin-provided `#{…}` format variable.
///
/// Called during format expansion with the key being resolved. To provide a
/// value, call `emit(sink, value)` and return 0. Return non-zero to decline,
/// and the host continues its normal lookup as if the plugin had never
/// registered. Format expansion runs on every status-line redraw, so this
/// must be cheap and must not block.
///
/// `ctx` is the expansion in progress: [`HostApi::format_expand`] through it
/// resolves against the *same* tree, which is how a provider sees the client,
/// session, window and pane it is being drawn for (`#{client_prefix}`,
/// `#{pane_in_mode}`, …). Expanding a key back into the plugin that is
/// currently providing it yields nothing rather than recursing.
pub type FormatFn = extern "C" fn(
    host: *const HostApi,
    ctx: *mut c_void,
    key: *const c_char,
    sink: *mut c_void,
    emit: EmitFn,
) -> c_int;

/// A plugin-provided hook subscription. Called after the hook fires, with a
/// [`HookEvent`] describing what happened. The return value is ignored
/// (hooks cannot fail a command); return 0.
pub type HookFn = extern "C" fn(host: *const HostApi, event: *const HookEvent) -> c_int;

/// Signature of [`INIT_SYMBOL`]. Called exactly once, right after the dylib
/// is loaded. The plugin registers its commands / formats / hooks through the
/// host table and returns a pointer to a `'static` [`PluginInfo`] describing
/// itself (or null on failure).
///
/// `unsafe` because it dereferences the host table it is handed: the caller
/// promises `host` is a valid `*const HostApi` that outlives the call.
pub type InitFn = unsafe extern "C" fn(host: *const HostApi) -> *const PluginInfo;

/// What fired, handed to a [`HookFn`]. Every pointer is borrowed from the
/// host and is valid only for the duration of the call; copy what you keep.
/// A field that does not apply to the hook is null (strings) or -1 (ids).
#[repr(C)]
pub struct HookEvent {
    /// Hook name, e.g. `session-created`, `pane-exited`, `client-attached`.
    pub name: *const c_char,
    /// Client name, or null.
    pub client: *const c_char,
    /// Session name, or null.
    pub session: *const c_char,
    /// Window name, or null.
    pub window: *const c_char,
    /// Window id (the number in `@3`), or -1.
    pub window_id: c_int,
    /// Pane id (the number in `%7`), or -1.
    pub pane_id: c_int,
}

/// The host API table handed to the plugin. Every field is a C-ABI function
/// pointer into ztmux. Layout is frozen by [`ABI_VERSION`].
///
/// A single instance lives for the whole server process; plugins may store
/// the `*const HostApi` they are given and call through it from any handler.
#[repr(C)]
pub struct HostApi {
    /// Must equal [`ABI_VERSION`]. Checked by the plugin's own
    /// `declare_plugin!` glue before it trusts the rest of the table.
    pub abi_version: u32,
    /// Reserved for the host; opaque to plugins. Currently null.
    pub ctx: *mut c_void,

    /// Register a tmux command. `name` is the command (`hello-world`),
    /// `alias` its short form or null, `template` the tmux flag template the
    /// server parses arguments with (`"bt:"` — a trailing `:` means the flag
    /// takes a value; empty for no flags), `usage` the one-line usage string
    /// shown on a syntax error. Returns 0 on success. All strings are copied
    /// by the host. Registering a name that a built-in tmux command already
    /// owns fails — the port's own table always wins.
    pub register_command: extern "C" fn(
        host: *const HostApi,
        name: *const c_char,
        alias: *const c_char,
        template: *const c_char,
        usage: *const c_char,
        handler: CommandFn,
    ) -> c_int,
    /// Register a `#{…}` format variable. `key` is the bare key (`plugin_x`,
    /// or `@x` for a user-style key). Returns 0 on success. Consulted after
    /// the built-in format table, so a plugin can never shadow a tmux format.
    pub register_format:
        extern "C" fn(host: *const HostApi, key: *const c_char, handler: FormatFn) -> c_int,
    /// Subscribe to a hook by name (`session-created`, `pane-exited`, …).
    /// Several plugins may subscribe to the same hook; all are called, in
    /// load order. Returns 0 on success.
    pub register_hook:
        extern "C" fn(host: *const HostApi, hook: *const c_char, handler: HookFn) -> c_int,

    /// Write a line to the client that ran the command (tmux's `cmdq_print`).
    /// A newline is added. With a null `ctx` (a hook or format callback,
    /// which has no client) the text goes to the server log instead.
    pub print: extern "C" fn(host: *const HostApi, ctx: *mut c_void, text: *const c_char),
    /// Report an error to the client that ran the command (tmux's
    /// `cmdq_error`). Does not itself fail the command — return non-zero
    /// from the handler for that.
    pub error: extern "C" fn(host: *const HostApi, ctx: *mut c_void, text: *const c_char),
    /// Parse `command` as tmux command text and queue it. This is the whole
    /// tmux command language — `bind-key`, `set-option`, `display-popup`,
    /// `;`-separated lists, everything. Returns 0 if it parsed and queued,
    /// non-zero on a parse error.
    pub run: extern "C" fn(host: *const HostApi, ctx: *mut c_void, command: *const c_char) -> c_int,

    /// Read an option by name — server, session, or window scope, in the
    /// order tmux resolves them, including the `@user` options a plugin
    /// configures itself with (`@my-plugin-key`). Returns a freshly
    /// allocated C string the caller MUST release with `free_cstring`, or
    /// null when the option is not set.
    pub get_option: extern "C" fn(host: *const HostApi, name: *const c_char) -> *mut c_char,
    /// Set an option globally (`set-option -gq`). Works for `@user` options
    /// and for real tmux options alike. Returns 0 on success.
    pub set_option:
        extern "C" fn(host: *const HostApi, name: *const c_char, value: *const c_char) -> c_int,
    /// Expand `#{…}` in `text` against the running command's target. Returns
    /// a freshly allocated C string the caller MUST release with
    /// `free_cstring`. With a null `ctx` the expansion has no target and only
    /// server-wide formats resolve.
    pub format_expand:
        extern "C" fn(host: *const HostApi, ctx: *mut c_void, text: *const c_char) -> *mut c_char,

    /// True (non-zero) when the command was given flag `flag` — one of the
    /// letters in the template it registered.
    pub arg_has: extern "C" fn(host: *const HostApi, ctx: *mut c_void, flag: c_char) -> c_int,
    /// The value given to flag `flag` (a template letter followed by `:`).
    /// Returns a freshly allocated C string the caller MUST release with
    /// `free_cstring`, or null when the flag was not given.
    pub arg_get: extern "C" fn(host: *const HostApi, ctx: *mut c_void, flag: c_char) -> *mut c_char,

    /// Release a string previously returned by `get_option`, `arg_get`, or
    /// `format_expand`.
    pub free_cstring: extern "C" fn(host: *const HostApi, s: *mut c_char),
}

/// What a plugin returns from its [`InitFn`]. The strings must have
/// `'static` lifetime (typically string literals via the `declare_plugin!`
/// macro).
#[repr(C)]
pub struct PluginInfo {
    /// Must equal [`ABI_VERSION`]. Redundant with the host-side check, but
    /// lets the host reject a plugin that lied about its ABI.
    pub abi_version: u32,
    /// Plugin name, NUL-terminated. Used for `znative list` and unload.
    pub name: *const c_char,
    /// Plugin version, NUL-terminated. Informational.
    pub version: *const c_char,
}

// PluginInfo is only ever pointed at `'static` data; it carries no interior
// mutability. Marking it Sync lets the macro place it in a `static`.
unsafe impl Sync for PluginInfo {}

// ============================================================
// Ergonomic wrappers for plugin authors. None of this crosses the ABI; it is
// convenience over the raw pointers above.
// ============================================================

/// Safe wrapper over `*const HostApi`. Cheap to construct; borrows the host
/// table.
pub struct Host {
    api: *const HostApi,
}

impl Host {
    /// Wrap a raw host pointer.
    ///
    /// # Safety
    /// `api` must be the non-null `*const HostApi` the host handed to the
    /// plugin (in `ztnative_init` or a handler call) and must remain valid
    /// for the lifetime of this `Host`.
    pub unsafe fn from_raw(api: *const HostApi) -> Self {
        Host { api }
    }

    #[inline]
    fn t(&self) -> &HostApi {
        // Safe: constructed only from a valid host pointer.
        unsafe { &*self.api }
    }

    /// Register a tmux command. Usually done for you by `declare_plugin!`.
    /// `template` is the tmux flag template (`"bt:"`), `usage` the one-line
    /// usage string.
    pub fn register_command(
        &self,
        name: &str,
        alias: Option<&str>,
        template: &str,
        usage: &str,
        handler: CommandFn,
    ) -> bool {
        let (Ok(cn), Ok(ct), Ok(cu)) = (
            CString::new(name),
            CString::new(template),
            CString::new(usage),
        ) else {
            return false;
        };
        let ca = alias.and_then(|a| CString::new(a).ok());
        let ap = ca.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        (self.t().register_command)(self.api, cn.as_ptr(), ap, ct.as_ptr(), cu.as_ptr(), handler)
            == 0
    }

    /// Register a `#{…}` format variable. Usually done for you by
    /// `declare_plugin!`.
    pub fn register_format(&self, key: &str, handler: FormatFn) -> bool {
        let Ok(c) = CString::new(key) else {
            return false;
        };
        (self.t().register_format)(self.api, c.as_ptr(), handler) == 0
    }

    /// Subscribe to a hook. Usually done for you by `declare_plugin!`.
    pub fn register_hook(&self, hook: &str, handler: HookFn) -> bool {
        let Ok(c) = CString::new(hook) else {
            return false;
        };
        (self.t().register_hook)(self.api, c.as_ptr(), handler) == 0
    }

    /// Write a line to the client that ran the command.
    pub fn print(&self, ctx: &Ctx, text: &str) {
        if let Ok(c) = CString::new(text) {
            (self.t().print)(self.api, ctx.raw, c.as_ptr());
        }
    }

    /// Report an error to the client that ran the command.
    pub fn error(&self, ctx: &Ctx, text: &str) {
        if let Ok(c) = CString::new(text) {
            (self.t().error)(self.api, ctx.raw, c.as_ptr());
        }
    }

    /// Parse and queue tmux command text. Returns true if it parsed.
    pub fn run(&self, ctx: &Ctx, command: &str) -> bool {
        match CString::new(command) {
            Ok(c) => (self.t().run)(self.api, ctx.raw, c.as_ptr()) == 0,
            Err(_) => false,
        }
    }

    /// Read an option (including `@user` options), or `None` when unset.
    pub fn get_option(&self, name: &str) -> Option<String> {
        let c = CString::new(name).ok()?;
        self.take_string((self.t().get_option)(self.api, c.as_ptr()))
    }

    /// Set an option globally. Returns true on success.
    pub fn set_option(&self, name: &str, value: &str) -> bool {
        let (Ok(cn), Ok(cv)) = (CString::new(name), CString::new(value)) else {
            return false;
        };
        (self.t().set_option)(self.api, cn.as_ptr(), cv.as_ptr()) == 0
    }

    /// Expand `#{…}` against the running command's target.
    pub fn format_expand(&self, ctx: &Ctx, text: &str) -> Option<String> {
        let c = CString::new(text).ok()?;
        self.take_string((self.t().format_expand)(self.api, ctx.raw, c.as_ptr()))
    }

    /// Copy a host-allocated C string into a `String` and hand the original
    /// back to the host's allocator.
    fn take_string(&self, raw: *mut c_char) -> Option<String> {
        if raw.is_null() {
            return None;
        }
        // Safe: host contract says this is a valid C string owned by it.
        let s = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        (self.t().free_cstring)(self.api, raw);
        Some(s)
    }
}

/// Opaque handle to the command currently running: what it prints to, and the
/// flags it was parsed with. Handed to a [`CommandFn`]; [`Ctx::none`] is the
/// no-command context a hook or format callback uses.
pub struct Ctx {
    raw: *mut c_void,
    api: *const HostApi,
}

impl Ctx {
    /// Wrap the raw context pointer a handler was called with.
    ///
    /// # Safety
    /// `raw` must be the pointer the host passed to this handler, and `api`
    /// the host table; both are valid only for the duration of the call.
    pub unsafe fn from_raw(api: *const HostApi, raw: *mut c_void) -> Self {
        Ctx { raw, api }
    }

    /// The empty context — no client to print to, no flags to read.
    pub fn none(api: *const HostApi) -> Self {
        Ctx {
            raw: std::ptr::null_mut(),
            api,
        }
    }

    /// True when the command was given `flag`.
    pub fn has(&self, flag: char) -> bool {
        if !flag.is_ascii() {
            return false;
        }
        // Safe: `api` is the valid host table this Ctx was built from.
        let t = unsafe { &*self.api };
        (t.arg_has)(self.api, self.raw, flag as c_char) != 0
    }

    /// The value given to `flag`, or `None` when it was not given.
    pub fn arg(&self, flag: &str) -> Option<String> {
        let f = flag.chars().next()?;
        if !f.is_ascii() {
            return None;
        }
        let t = unsafe { &*self.api };
        let raw = (t.arg_get)(self.api, self.raw, f as c_char);
        if raw.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        (t.free_cstring)(self.api, raw);
        Some(s)
    }
}

/// Safe view over a command's `(argc, argv)`. `argv[0]` is the command name;
/// the rest are the positional arguments (flags come from [`Ctx`]).
pub struct Args {
    items: Vec<String>,
}

impl Args {
    /// Decode a raw `(argc, argv)` pair into owned `String`s.
    ///
    /// # Safety
    /// `argv` must point to `argc` valid, NUL-terminated C strings, as
    /// guaranteed by the host when it invokes a [`CommandFn`].
    pub unsafe fn from_raw(argc: usize, argv: *const *const c_char) -> Self {
        // Explicit blocks rather than relying on an `unsafe fn` body being
        // implicitly unsafe: this file is compiled into hosts and plugins on
        // both the 2021 and 2024 editions, and 2024 requires them.
        let mut items = Vec::with_capacity(argc);
        if !argv.is_null() {
            for i in 0..argc {
                let p = unsafe { *argv.add(i) };
                if p.is_null() {
                    break;
                }
                items.push(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned());
            }
        }
        Args { items }
    }

    /// The command name (`argv[0]`), or `""` if somehow empty.
    pub fn name(&self) -> &str {
        self.items.first().map_or("", String::as_str)
    }

    /// The positional arguments (everything after `argv[0]`).
    pub fn rest(&self) -> &[String] {
        if self.items.is_empty() {
            &[]
        } else {
            &self.items[1..]
        }
    }

    /// All of `argv`, name included.
    pub fn to_vec(&self) -> &[String] {
        &self.items
    }
}

/// Safe view over a [`HookEvent`].
pub struct Hook {
    /// Hook name, e.g. `session-created`.
    pub name: String,
    /// Client name, if the hook carried one.
    pub client: Option<String>,
    /// Session name, if the hook carried one.
    pub session: Option<String>,
    /// Window name, if the hook carried one.
    pub window: Option<String>,
    /// Window id (the number in `@3`), if the hook carried one.
    pub window_id: Option<i32>,
    /// Pane id (the number in `%7`), if the hook carried one.
    pub pane_id: Option<i32>,
}

impl Hook {
    /// Copy a borrowed [`HookEvent`] into owned data.
    ///
    /// # Safety
    /// `e` must be the event pointer the host passed to a [`HookFn`], valid
    /// for the duration of the call.
    pub unsafe fn from_raw(e: *const HookEvent) -> Hook {
        fn s(p: *const c_char) -> Option<String> {
            if p.is_null() {
                None
            } else {
                // Safe: the host's contract is that every non-null string in a
                // `HookEvent` is a valid C string for the duration of the call.
                Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
            }
        }
        if e.is_null() {
            return Hook {
                name: String::new(),
                client: None,
                session: None,
                window: None,
                window_id: None,
                pane_id: None,
            };
        }
        // Safe: non-null, and the host guarantees the event outlives the call.
        let event = unsafe { &*e };
        Hook {
            name: s(event.name).unwrap_or_default(),
            client: s(event.client),
            session: s(event.session),
            window: s(event.window),
            window_id: (event.window_id >= 0).then_some(event.window_id),
            pane_id: (event.pane_id >= 0).then_some(event.pane_id),
        }
    }
}

/// Declare a plugin.
///
/// Takes the plugin's identity, the tmux commands it adds, the `#{…}` formats
/// it provides and the hooks it subscribes to, and expands to the
/// `#[no_mangle] extern "C" fn ztnative_init` the host resolves with `dlsym`,
/// plus the `'static` `PluginInfo` it returns. (Plain code spans, not intra-doc
/// links: this macro is exported at its crate's root while the types live in
/// the `ztnative` module, and rustdoc refuses a public doc that links into a
/// private one.)
///
/// * `commands:` — each `"name" => { alias, template, usage, handler }` adds a
///   tmux command. `alias` may be omitted. A handler is
///   `fn(&Host, &Ctx, &Args) -> c_int`.
/// * `formats:` — each `"key" => provider` provides `#{key}`. A provider is
///   `fn(&Host, &Ctx, &str) -> Option<String>`; `None` declines and the host
///   resolves the key normally. The `Ctx` is the expansion in progress, so
///   `host.format_expand(ctx, "#{client_prefix}")` answers for the client being
///   drawn.
/// * `hooks:` — each `"hook-name" => handler` subscribes to a hook. A handler
///   is `fn(&Host, &Ctx, &Hook)`; the context is the empty one, so `run` queues
///   globally and `print` goes to the server log.
/// * `on_load:` — a `fn(&Host, &Ctx)` run once, after registration, when the
///   plugin is loaded. This is where a plugin that configures the server does
///   its work; `#[macro_export]`-style plugins that only add commands do not
///   need it.
///
/// All three sections are optional.
///
/// ```ignore
/// declare_plugin! {
///     name: "greet",
///     version: "0.1.0",
///     commands: {
///         "greet" => { template: "", usage: "[name]", handler: greet },
///     },
///     formats: { "greet_count" => greet_count },
///     hooks:   { "session-created" => on_session },
/// }
/// ```
#[macro_export]
macro_rules! declare_plugin {
    (
        name: $name:literal,
        version: $version:literal,
        $(commands: {
            $($cmd:literal => {
                $(alias: $alias:literal,)?
                template: $template:literal,
                usage: $usage:literal,
                handler: $handler:path $(,)?
            }),+ $(,)?
        } $(,)?)?
        $(formats: { $($fkey:literal => $fprovider:path),+ $(,)? } $(,)?)?
        $(hooks: { $($hname:literal => $hhandler:path),+ $(,)? } $(,)?)?
        $(on_load: $onload:path $(,)?)?
    ) => {
        static __ZTNATIVE_PLUGIN_INFO: $crate::ztnative::PluginInfo = $crate::ztnative::PluginInfo {
            abi_version: $crate::ztnative::ABIVERSION_FOR_MACRO,
            // `as_bytes().as_ptr()` rather than `str::as_ptr()`: identical
            // pointer, but it does not trip the lint some hosts (ztmux's own
            // tree among them) put on taking a raw pointer out of a `str`.
            name: concat!($name, "\0").as_bytes().as_ptr() as *const ::std::os::raw::c_char,
            version: concat!($version, "\0").as_bytes().as_ptr()
                as *const ::std::os::raw::c_char,
        };

        /// Plugin entry point, resolved by the host with `dlsym` after
        /// `dlopen`. Registers everything `declare_plugin!` was given and
        /// returns this plugin's `PluginInfo`.
        ///
        /// # Safety
        /// `host` must be the valid `*const HostApi` the host passes in, and
        /// must stay valid for as long as the plugin is loaded -- the host's
        /// side of the ABI contract. Called exactly once, by the host.
        #[no_mangle]
        pub unsafe extern "C" fn ztnative_init(
            host: *const $crate::ztnative::HostApi,
        ) -> *const $crate::ztnative::PluginInfo {
            if host.is_null() {
                return ::std::ptr::null();
            }
            // Verify the host speaks our ABI before touching the table.
            let ver = unsafe { (*host).abi_version };
            if ver != $crate::ztnative::ABI_VERSION {
                return ::std::ptr::null();
            }
            let h = unsafe { $crate::ztnative::Host::from_raw(host) };
            $($(
                {
                    // One trampoline per command: adapts the C-ABI CommandFn
                    // to the ergonomic fn(&Host,&Ctx,&Args).
                    extern "C" fn __cmd(
                        host: *const $crate::ztnative::HostApi,
                        ctx: *mut ::std::os::raw::c_void,
                        argc: usize,
                        argv: *const *const ::std::os::raw::c_char,
                    ) -> ::std::os::raw::c_int {
                        let h = unsafe { $crate::ztnative::Host::from_raw(host) };
                        let c = unsafe { $crate::ztnative::Ctx::from_raw(host, ctx) };
                        let a = unsafe { $crate::ztnative::Args::from_raw(argc, argv) };
                        $handler(&h, &c, &a)
                    }
                    #[allow(unused_mut, unused_assignments)]
                    let mut __alias: Option<&str> = None;
                    $(__alias = Some($alias);)?
                    h.register_command($cmd, __alias, $template, $usage, __cmd);
                }
            )+)?
            $($(
                {
                    // One trampoline per format key. The provider's String is
                    // copied by the host through `emit` — plugin-allocated
                    // memory never crosses the boundary by pointer.
                    extern "C" fn __fmt(
                        host: *const $crate::ztnative::HostApi,
                        ctx: *mut ::std::os::raw::c_void,
                        key: *const ::std::os::raw::c_char,
                        sink: *mut ::std::os::raw::c_void,
                        emit: $crate::ztnative::EmitFn,
                    ) -> ::std::os::raw::c_int {
                        let h = unsafe { $crate::ztnative::Host::from_raw(host) };
                        let c = unsafe { $crate::ztnative::Ctx::from_raw(host, ctx) };
                        let k = if key.is_null() {
                            ::std::string::String::new()
                        } else {
                            unsafe { ::std::ffi::CStr::from_ptr(key) }
                                .to_string_lossy()
                                .into_owned()
                        };
                        match $fprovider(&h, &c, &k) {
                            Some(v) => match ::std::ffi::CString::new(v) {
                                Ok(c) => {
                                    emit(sink, c.as_ptr());
                                    0
                                }
                                Err(_) => 1,
                            },
                            None => 1,
                        }
                    }
                    h.register_format($fkey, __fmt);
                }
            )+)?
            $($(
                {
                    extern "C" fn __hook(
                        host: *const $crate::ztnative::HostApi,
                        event: *const $crate::ztnative::HookEvent,
                    ) -> ::std::os::raw::c_int {
                        let h = unsafe { $crate::ztnative::Host::from_raw(host) };
                        // A hook has no command in flight, so it gets the empty
                        // context: `run` queues globally and `print` goes to the
                        // server log, since there is no client to print to.
                        let c = $crate::ztnative::Ctx::none(host);
                        let e = unsafe { $crate::ztnative::Hook::from_raw(event) };
                        $hhandler(&h, &c, &e);
                        0
                    }
                    h.register_hook($hname, __hook);
                }
            )+)?
            $(
                {
                    // Everything above is registered by now, so a plugin that
                    // has work to do at load -- applying settings, binding
                    // keys -- does it here. The context is the empty one: the
                    // plugin is being loaded by a command, not running as one,
                    // so `run` queues globally and `print` goes to the log.
                    let c = $crate::ztnative::Ctx::none(host);
                    $onload(&h, &c);
                }
            )?
            &__ZTNATIVE_PLUGIN_INFO as *const $crate::ztnative::PluginInfo
        }
    };
}

// The macro can't name `ABI_VERSION` inside a `const` initializer of a
// downstream crate without importing it; re-export under a stable path the
// macro hard-codes so users need only the names in the doc example.
#[doc(hidden)]
pub const ABIVERSION_FOR_MACRO: u32 = ABI_VERSION;
