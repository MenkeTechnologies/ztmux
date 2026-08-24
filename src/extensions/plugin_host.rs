//! Native (Rust) plugin host — ztmux extension; no tmux C counterpart.
//!
//! tmux has no plugin ABI at all: every "plugin" in the ecosystem is a shell
//! script that TPM clones and executes, and that script drives the server the
//! only way it can — by shelling out to `tmux bind-key …`. ztmux keeps that
//! world working (see [`super::pkg`], which installs and runs `*.tmux` files
//! exactly like TPM) and adds the thing tmux never had: a **stable, versioned
//! C ABI** (the [`ztnative`] crate) so a third party ships a compiled
//! `cdylib` that the server `dlopen`s, and what it registers are real tmux
//! commands, real `#{…}` format variables, and real hook subscriptions —
//! resolved inside the server with no subprocess in the loop.
//!
//! ## Where plugin commands resolve
//!
//! [`crate::cmd_::cmd_find`] scans the ported [`crate::cmd_::CMD_TABLE`]
//! first and only consults [`command_entry`] when the static table has no
//! match, so a plugin can never shadow a tmux command. A plugin command that
//! resolves is an ordinary `cmd_entry` from there on: it parses its flags
//! through tmux's own `args_parse`, runs on the command queue, and works from
//! the command prompt, a key binding, `.tmux.conf`, and the CLI alike.
//!
//! ## ABI safety
//!
//! Everything crossing the boundary is `#[repr(C)]`. The host verifies the
//! plugin's `abi_version` matches [`ztnative::ABI_VERSION`] before trusting
//! any pointer it returns; a mismatch is refused (a wrong struct layout would
//! be undefined behaviour). Strings only travel with their allocator: what
//! the host hands out comes back through `free_cstring`, and a plugin's own
//! strings are copied out through an `emit` callback rather than handed over.
//!
//! Unload purges the registries BEFORE the `dlclose`, and every dispatch path
//! looks its handler up by name at call time — so a command left in a parsed,
//! queued command list after its plugin was unloaded fails cleanly instead of
//! calling into an unmapped page.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Mutex, OnceLock};

use ztnative::{
    ABI_VERSION, CommandFn, EmitFn, FormatFn, HookEvent, HookFn, HostApi, INIT_SYMBOL, InitFn,
    PluginInfo,
};

use crate::options_::{options_parse_get, options_set_string, options_to_string};
use crate::*;

/// One loaded plugin. Dropping `_lib` runs `dlclose`, so this is only ever
/// removed by [`unload`] AFTER its registrations are purged.
struct LoadedPlugin {
    name: String,
    version: String,
    path: String,
    /// Kept alive for the process lifetime; drop = `dlclose`.
    _lib: libloading::Library,
}

/// A registered tmux command: the plugin's handler, the leaked `cmd_entry`
/// `cmd_find` hands back, and the plugin that owns it.
struct PluginCommand {
    handler: CommandFn,
    entry: &'static cmd_entry,
    owner: String,
}

/// The context a [`CommandFn`] is given, behind the ABI's opaque `ctx`
/// pointer: the queue item it prints to, the parsed arguments it reads flags
/// from, and the item each `run()` chains after so queued commands keep their
/// order.
struct PluginCall {
    item: *mut cmdq_item,
    args: *mut args,
    after: *mut cmdq_item,
}

fn plugins() -> &'static Mutex<Vec<LoadedPlugin>> {
    static P: OnceLock<Mutex<Vec<LoadedPlugin>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(Vec::new()))
}

/// command name → registration. Consulted by `cmd_find` after `CMD_TABLE`.
fn commands() -> &'static Mutex<HashMap<String, PluginCommand>> {
    static C: OnceLock<Mutex<HashMap<String, PluginCommand>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `#{…}` key → (provider, owner). Consulted by `format_find` after the
/// built-in format table.
fn formats() -> &'static Mutex<HashMap<String, (FormatFn, String)>> {
    static F: OnceLock<Mutex<HashMap<String, (FormatFn, String)>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Everything subscribed to one hook: the handler and its owning plugin.
type HookSubscribers = HashMap<String, Vec<(HookFn, String)>>;

/// hook name → subscribers, in load order. Consulted by `notify_add`.
fn hooks() -> &'static Mutex<HookSubscribers> {
    static H: OnceLock<Mutex<HookSubscribers>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether any plugin has registered a format / a hook. `format_find` runs
/// for every `#{…}` on every status-line redraw and `notify_add` for every
/// notification, so the overwhelmingly common case — no native plugin loaded
/// at all — must cost one relaxed atomic load, not a mutex and a hash lookup.
static ANY_FORMATS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ANY_HOOKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Refresh the [`ANY_FORMATS`] / [`ANY_HOOKS`] fast-path flags from the
/// registries. Called after every load and unload, never on a dispatch path.
fn refresh_fast_paths() {
    use std::sync::atomic::Ordering;
    ANY_FORMATS.store(!formats().lock().unwrap().is_empty(), Ordering::Relaxed);
    ANY_HOOKS.store(!hooks().lock().unwrap().is_empty(), Ordering::Relaxed);
}

/// Staging for what a single `init` call registers. `init` runs before it
/// returns the plugin name, so registrations are buffered here and tagged
/// with the owning plugin afterwards. Serialised by [`load_lock`].
#[derive(Default)]
struct Staging {
    commands: Vec<(String, Option<String>, String, String, CommandFn)>,
    formats: Vec<(String, FormatFn)>,
    hooks: Vec<(String, HookFn)>,
}

fn staging() -> &'static Mutex<Staging> {
    static S: OnceLock<Mutex<Staging>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Staging::default()))
}

/// Serialises `load`/`unload` so the [`staging`] buffer is single-writer.
fn load_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

// ============================================================
// Host API callbacks — the `extern "C"` functions plugins call back through.
// One shared, leaked `HostApi` table for the whole server process.
// ============================================================

/// Decode the ABI's opaque `ctx` back into the running command's context.
/// Null (a hook or format callback) means there is no command in flight.
fn call_of<'a>(ctx: *mut c_void) -> Option<&'a mut PluginCall> {
    if ctx.is_null() {
        None
    } else {
        // Safe: the only non-null ctx a plugin ever sees is the &mut
        // PluginCall `plugin_cmd_exec` parks on its stack for the call.
        Some(unsafe { &mut *(ctx as *mut PluginCall) })
    }
}

/// Borrow a NUL-terminated C string argument, or `None` when null.
fn borrow<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        // Safe: host contract — every string a plugin passes is a valid,
        // NUL-terminated C string that outlives the call. Invalid UTF-8 is
        // rejected here rather than lossily repaired: a command name or
        // option key that is not text cannot match anything downstream.
        unsafe { CStr::from_ptr(p) }.to_str().ok()
    }
}

/// Hand a `String` out to a plugin as a C string it returns through
/// `free_cstring`. Null on an interior NUL (which no tmux value contains).
fn hand_out(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

extern "C" fn host_register_command(
    _host: *const HostApi,
    name: *const c_char,
    alias: *const c_char,
    template: *const c_char,
    usage: *const c_char,
    handler: CommandFn,
) -> c_int {
    let Some(name) = borrow(name) else { return 1 };
    if name.is_empty() {
        return 1;
    }
    staging().lock().unwrap().commands.push((
        name.to_string(),
        borrow(alias).map(str::to_string),
        borrow(template).unwrap_or("").to_string(),
        borrow(usage).unwrap_or("").to_string(),
        handler,
    ));
    0
}

extern "C" fn host_register_format(
    _host: *const HostApi,
    key: *const c_char,
    handler: FormatFn,
) -> c_int {
    let Some(key) = borrow(key) else { return 1 };
    if key.is_empty() {
        return 1;
    }
    staging().lock().unwrap().formats.push((key.to_string(), handler));
    0
}

extern "C" fn host_register_hook(
    _host: *const HostApi,
    hook: *const c_char,
    handler: HookFn,
) -> c_int {
    let Some(hook) = borrow(hook) else { return 1 };
    if hook.is_empty() {
        return 1;
    }
    staging().lock().unwrap().hooks.push((hook.to_string(), handler));
    0
}

extern "C" fn host_print(_host: *const HostApi, ctx: *mut c_void, text: *const c_char) {
    let Some(text) = borrow(text) else { return };
    match call_of(ctx) {
        // A command in flight prints to the client that ran it.
        Some(call) if !call.item.is_null() => unsafe {
            crate::cmd_::cmd_queue::cmdq_print_(call.item, format_args!("{text}"));
        },
        // A hook or format callback has no client; the log is the only sink.
        _ => log_debug!("plugin: {}", text),
    }
}

extern "C" fn host_error(_host: *const HostApi, ctx: *mut c_void, text: *const c_char) {
    let Some(text) = borrow(text) else { return };
    match call_of(ctx) {
        Some(call) if !call.item.is_null() => unsafe {
            crate::cmd_::cmd_queue::cmdq_error_(call.item, format_args!("{text}"));
        },
        _ => log_debug!("plugin error: {}", text),
    }
}

extern "C" fn host_run(_host: *const HostApi, ctx: *mut c_void, command: *const c_char) -> c_int {
    let Some(command) = borrow(command) else {
        return 1;
    };
    unsafe {
        match call_of(ctx) {
            // Inside a command: chain each queued list after the previous one
            // so several `run()` calls execute in the order they were made.
            Some(call) if !call.after.is_null() => match cmd_parse_from_string(command, None) {
                Ok(cmdlist) => {
                    let new_item = cmdq_get_command(cmdlist, cmdq_get_state(call.item));
                    call.after = cmdq_insert_after(call.after, new_item);
                    cmd_list_free(cmdlist);
                    0
                }
                Err(error) => {
                    log_debug!("plugin run: {}", _s(error));
                    free_(error);
                    1
                }
            },
            // Outside a command (a hook): append to the global queue.
            _ => {
                let mut error: *mut u8 = null_mut();
                let status = cmd_parse_and_append(
                    command,
                    None,
                    null_mut(),
                    null_mut(),
                    &raw mut error,
                );
                if status == cmd_parse_status::CMD_PARSE_ERROR {
                    log_debug!("plugin run: {}", _s(error));
                    free_(error);
                    return 1;
                }
                0
            }
        }
    }
}

extern "C" fn host_get_option(_host: *const HostApi, name: *const c_char) -> *mut c_char {
    let Some(name) = borrow(name) else {
        return std::ptr::null_mut();
    };
    unsafe {
        // The order `set-option` resolves in for a plugin's own `@options`:
        // session scope first (where `set -g @x` lands), then window, then
        // server. `options_parse_get` handles `name[idx]` array syntax too.
        let mut idx = 0;
        let mut o = options_parse_get(GLOBAL_S_OPTIONS, name, &raw mut idx, 0);
        if o.is_null() {
            o = options_parse_get(GLOBAL_W_OPTIONS, name, &raw mut idx, 0);
        }
        if o.is_null() {
            o = options_parse_get(GLOBAL_OPTIONS, name, &raw mut idx, 0);
        }
        if o.is_null() {
            return std::ptr::null_mut();
        }
        let value = options_to_string(o, idx, 1);
        if value.is_null() {
            return std::ptr::null_mut();
        }
        // Copy into a string owned by the host's `free_cstring` contract and
        // release the xmalloc'd original.
        let out = hand_out(cstr_to_str(value).to_string());
        free_(value);
        out
    }
}

extern "C" fn host_set_option(
    _host: *const HostApi,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    let (Some(name), Some(value)) = (borrow(name), borrow(value)) else {
        return 1;
    };
    unsafe {
        // Global session scope — what `set-option -g` writes, and where every
        // `@user` option a plugin configures itself with is read from.
        let o = options_set_string!(GLOBAL_S_OPTIONS, name, false, "{}", value);
        i32::from(o.is_null())
    }
}

extern "C" fn host_format_expand(
    _host: *const HostApi,
    ctx: *mut c_void,
    text: *const c_char,
) -> *mut c_char {
    let Some(text) = borrow(text) else {
        return std::ptr::null_mut();
    };
    let fmt = cstring_truncating(text.to_string());
    unsafe {
        let expanded = match call_of(ctx) {
            Some(call) if !call.item.is_null() => {
                format_single_from_target(call.item, fmt.as_ptr().cast())
            }
            // No target: server-wide formats only.
            _ => {
                let ft = format_create(null_mut(), null_mut(), 0, format_flags::empty());
                let out = format_expand(ft, fmt.as_ptr().cast());
                format_free(ft);
                out
            }
        };
        if expanded.is_null() {
            return std::ptr::null_mut();
        }
        let out = hand_out(cstr_to_str(expanded).to_string());
        free_(expanded);
        out
    }
}

extern "C" fn host_arg_has(_host: *const HostApi, ctx: *mut c_void, flag: c_char) -> c_int {
    let Some(call) = call_of(ctx) else { return 0 };
    if call.args.is_null() || !(flag as u8).is_ascii() {
        return 0;
    }
    // Safe: `args` is the parsed argument set of the command in flight.
    c_int::from(unsafe { args_has(call.args, flag as u8 as char) })
}

extern "C" fn host_arg_get(_host: *const HostApi, ctx: *mut c_void, flag: c_char) -> *mut c_char {
    let Some(call) = call_of(ctx) else {
        return std::ptr::null_mut();
    };
    if call.args.is_null() || !(flag as u8).is_ascii() {
        return std::ptr::null_mut();
    }
    unsafe {
        let value = args_get(call.args, flag as u8);
        if value.is_null() {
            return std::ptr::null_mut();
        }
        hand_out(cstr_to_str(value).to_string())
    }
}

extern "C" fn host_free_cstring(_host: *const HostApi, s: *mut c_char) {
    if !s.is_null() {
        // Reclaim ownership of a string we handed out via `into_raw`.
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// The single process-wide host table. Leaked so its address is `'static` —
/// plugins may retain the `*const HostApi` and call through it at any time.
fn host_api() -> *const HostApi {
    static API: OnceLock<usize> = OnceLock::new();
    let addr = API.get_or_init(|| {
        let boxed = Box::new(HostApi {
            abi_version: ABI_VERSION,
            ctx: std::ptr::null_mut(),
            register_command: host_register_command,
            register_format: host_register_format,
            register_hook: host_register_hook,
            print: host_print,
            error: host_error,
            run: host_run,
            get_option: host_get_option,
            set_option: host_set_option,
            format_expand: host_format_expand,
            arg_has: host_arg_has,
            arg_get: host_arg_get,
            free_cstring: host_free_cstring,
        });
        Box::into_raw(boxed) as usize
    });
    *addr as *const HostApi
}

// ============================================================
// The `cmd_entry` a plugin command resolves through.
// ============================================================

/// The `exec` every plugin command shares. Which command this is comes from
/// the entry's own name, and the handler is looked up by that name on each
/// call — so a command whose plugin was unloaded while it sat parsed in a
/// queued command list fails cleanly instead of jumping into an unmapped
/// page.
unsafe fn plugin_cmd_exec(self_: *mut cmd, item: *mut cmdq_item) -> cmd_retval {
    unsafe {
        let entry = cmd_get_entry(self_);
        let args = cmd_get_args(self_);

        let Some(handler) = commands().lock().unwrap().get(entry.name).map(|c| c.handler) else {
            cmdq_error_(
                item,
                format_args!("{}: plugin command is no longer loaded", entry.name),
            );
            return cmd_retval::CMD_RETURN_ERROR;
        };

        // argv[0] is the command name; the rest are the positional arguments
        // tmux parsed out. Flags are read back through `arg_has`/`arg_get`.
        let mut owned: Vec<CString> = Vec::with_capacity(args_count(args) as usize + 1);
        owned.push(cstring_truncating(entry.name.to_string()));
        for i in 0..args_count(args) {
            let value = args_string(args, i);
            if value.is_null() {
                continue;
            }
            owned.push(cstring_truncating(cstr_to_str(value).to_string()));
        }
        let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();

        let mut call = PluginCall {
            item,
            args,
            after: item,
        };
        let ctx = (&raw mut call).cast::<c_void>();
        let rc = handler(host_api(), ctx, ptrs.len(), ptrs.as_ptr());
        // `owned`/`ptrs` outlive the call.
        if rc == 0 {
            cmd_retval::CMD_RETURN_NORMAL
        } else {
            cmd_retval::CMD_RETURN_ERROR
        }
    }
}

/// Build the `'static` `cmd_entry` a plugin command resolves through. Leaked
/// deliberately: a parsed `cmd` holds the entry by reference for as long as
/// it sits in a command list, key binding, or menu, which can outlive the
/// plugin — the entry stays valid and [`plugin_cmd_exec`] reports the
/// unloaded plugin instead.
fn leak_entry(
    name: &str,
    alias: Option<&str>,
    template: &str,
    usage: &str,
) -> &'static cmd_entry {
    Box::leak(Box::new(cmd_entry {
        name: String::leak(name.to_string()),
        alias: alias.map(|a| &*String::leak(a.to_string())),
        args: args_parse::new(String::leak(template.to_string()), 0, -1, None),
        usage: String::leak(usage.to_string()),
        source: cmd_entry_flag::zeroed(),
        target: cmd_entry_flag::zeroed(),
        flags: cmd_flag::empty(),
        exec: plugin_cmd_exec,
    }))
}

// ============================================================
// Public API — driven by the `znative` command.
// ============================================================

/// Load a plugin `cdylib` from `path`. Returns the plugin's name on success.
/// Loading a plugin whose name is already present is refused (unload first).
pub(crate) fn load(path: &str) -> Result<String, String> {
    let _guard = load_lock().lock().unwrap();

    let expanded = expand_tilde(path);
    // Safe in the only sense dlopen can be: the file is a plugin the user
    // installed, and its `ztnative_init` is checked for ABI agreement below
    // before any pointer it returns is trusted.
    let lib = unsafe { libloading::Library::new(&expanded) }
        .map_err(|e| format!("cannot load `{path}`: {e}"))?;

    let init: libloading::Symbol<InitFn> = unsafe {
        lib.get(INIT_SYMBOL).map_err(|_| {
            format!(
                "`{}`: not a ztmux plugin (no {})",
                path,
                String::from_utf8_lossy(&INIT_SYMBOL[..INIT_SYMBOL.len() - 1])
            )
        })?
    };

    *staging().lock().unwrap() = Staging::default();
    // Safe: `host_api()` is the process-wide table this host owns, valid for
    // the lifetime of the process — the contract `InitFn` asks for.
    let info_ptr: *const PluginInfo = unsafe { init(host_api()) };
    if info_ptr.is_null() {
        *staging().lock().unwrap() = Staging::default();
        return Err(format!("`{path}`: plugin init failed (ABI mismatch or error)"));
    }
    // Safe: non-null and, once the version matches, laid out as this host's
    // own `PluginInfo` — both sides compile the same `ztnative` struct.
    let info = unsafe { &*info_ptr };
    if info.abi_version != ABI_VERSION {
        *staging().lock().unwrap() = Staging::default();
        return Err(format!(
            "`{}`: ABI version {} != host {}",
            path, info.abi_version, ABI_VERSION
        ));
    }
    let name = cstr_or(info.name, "unknown");
    let version = cstr_or(info.version, "?");

    if plugins().lock().unwrap().iter().any(|p| p.name == name) {
        *staging().lock().unwrap() = Staging::default();
        return Err(format!("plugin `{name}` already loaded"));
    }

    // A plugin may not take a name the tmux port owns: `cmd_find` scans
    // CMD_TABLE first, so such a command could never be dispatched, and
    // silently registering it would be a lie.
    let staged = std::mem::take(&mut *staging().lock().unwrap());
    for (cmd_name, ..) in &staged.commands {
        if crate::cmd_::cmd_find(cmd_name).is_ok() {
            return Err(format!(
                "plugin `{name}`: `{cmd_name}` is already a tmux command"
            ));
        }
    }

    {
        let mut reg = commands().lock().unwrap();
        for (cmd_name, alias, template, usage, handler) in staged.commands {
            let entry = leak_entry(&cmd_name, alias.as_deref(), &template, &usage);
            reg.insert(
                cmd_name,
                PluginCommand {
                    handler,
                    entry,
                    owner: name.clone(),
                },
            );
        }
    }
    {
        let mut reg = formats().lock().unwrap();
        for (key, handler) in staged.formats {
            reg.insert(key, (handler, name.clone()));
        }
    }
    {
        let mut reg = hooks().lock().unwrap();
        for (hook, handler) in staged.hooks {
            reg.entry(hook).or_default().push((handler, name.clone()));
        }
    }

    refresh_fast_paths();

    plugins().lock().unwrap().push(LoadedPlugin {
        name: name.clone(),
        version: version.clone(),
        path: expanded,
        _lib: lib,
    });

    log_debug!("loaded native plugin {} {} from {}", name, version, path);
    Ok(name)
}

/// Read the name and version a plugin cdylib declares, without keeping it
/// loaded and without committing anything it registers.
///
/// This is what lets a native plugin be installed under its own identity
/// rather than its repository's basename: the store path and index key are
/// decided before the plugin is installed, but the authoritative name lives
/// inside the compiled artifact. The plugin's `ztnative_init` does run — its
/// registrations are staged and thrown away — so a plugin that does work in
/// `init` beyond registering does that work once more at install time.
pub(crate) fn probe(path: &str) -> Result<(String, String), String> {
    let _guard = load_lock().lock().unwrap();

    let expanded = expand_tilde(path);
    // Safe: same contract as `load` — a plugin file the user is installing,
    // whose `PluginInfo` is trusted only after the ABI version matches.
    let lib = unsafe { libloading::Library::new(&expanded) }
        .map_err(|e| format!("cannot load `{path}`: {e}"))?;
    let init: libloading::Symbol<InitFn> = unsafe {
        lib.get(INIT_SYMBOL)
            .map_err(|_| format!("`{path}`: not a ztmux plugin"))?
    };

    *staging().lock().unwrap() = Staging::default();
    // Safe: same contract as in `load` — the process-wide host table.
    let info_ptr: *const PluginInfo = unsafe { init(host_api()) };
    // Nothing was committed; drop what init staged so a later `load` starts
    // from an empty buffer.
    *staging().lock().unwrap() = Staging::default();
    if info_ptr.is_null() {
        return Err(format!("`{path}`: plugin init failed"));
    }
    let info = unsafe { &*info_ptr };
    if info.abi_version != ABI_VERSION {
        return Err(format!(
            "`{}`: ABI version {} != host {}",
            path, info.abi_version, ABI_VERSION
        ));
    }
    // Copy both strings out before `lib` drops: they live in the dylib, which
    // `dlclose` unmaps.
    let identity = (cstr_or(info.name, "unknown"), cstr_or(info.version, "?"));
    drop(lib);
    Ok(identity)
}

/// Unload a plugin by name: purge its registrations FIRST (so no live
/// function pointer into the dylib survives), then drop the `Library`
/// (`dlclose`).
pub(crate) fn unload(name: &str) -> Result<(), String> {
    let _guard = load_lock().lock().unwrap();

    if !plugins().lock().unwrap().iter().any(|p| p.name == name) {
        return Err(format!("plugin `{name}` not loaded"));
    }

    commands().lock().unwrap().retain(|_, c| c.owner != name);
    formats().lock().unwrap().retain(|_, (_, o)| o != name);
    {
        let mut reg = hooks().lock().unwrap();
        for subs in reg.values_mut() {
            subs.retain(|(_, o)| o != name);
        }
        reg.retain(|_, subs| !subs.is_empty());
    }

    refresh_fast_paths();

    // Now it is safe to dlclose.
    let mut ps = plugins().lock().unwrap();
    if let Some(pos) = ps.iter().position(|p| p.name == name) {
        let p = ps.remove(pos);
        log_debug!("unloaded native plugin {}", name);
        drop(p); // explicit: dlclose here, after the registry purge.
    }
    Ok(())
}

/// Command-resolution hook for [`crate::cmd_::cmd_find`]. Returns the entry a
/// plugin registered for `name` (or its alias), or `None` — consulted only
/// after `CMD_TABLE` misses, so the port always wins.
pub(crate) fn command_entry(name: &str) -> Option<&'static cmd_entry> {
    let reg = commands().lock().unwrap();
    if let Some(c) = reg.get(name) {
        return Some(c.entry);
    }
    reg.values()
        .find(|c| c.entry.alias == Some(name))
        .map(|c| c.entry)
}

/// Format-resolution hook for `format_find`. Returns the value a plugin
/// provides for `#{key}`, or `None` to let the host resolve the key as usual.
/// Consulted after the built-in format table, so a plugin cannot shadow a
/// tmux format.
pub(crate) fn dispatch_format(key: &str) -> Option<String> {
    if !ANY_FORMATS.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let handler = formats().lock().unwrap().get(key).map(|(f, _)| *f)?;
    let ckey = CString::new(key).ok()?;

    // The provider hands its value back through this sink, so a string its
    // allocator made is copied rather than passed across the boundary.
    extern "C" fn emit(sink: *mut c_void, text: *const c_char) {
        if sink.is_null() || text.is_null() {
            return;
        }
        // Safe: `sink` is the `Option<String>` parked on the stack below.
        let slot = unsafe { &mut *(sink as *mut Option<String>) };
        *slot = Some(
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        );
    }

    let mut out: Option<String> = None;
    let sink = (&raw mut out).cast::<c_void>();
    if handler(host_api(), ckey.as_ptr(), sink, emit as EmitFn) != 0 {
        return None;
    }
    out
}

/// Hook-dispatch entry point for `notify_add`. Every plugin subscribed to
/// `name` is called, in load order, with the names and ids the notification
/// carried. Cheap when nothing subscribed — the common case is one lock and
/// a miss.
pub(crate) fn dispatch_hook(
    name: &str,
    client: Option<&str>,
    session: Option<&str>,
    window: Option<&str>,
    window_id: i32,
    pane_id: i32,
) {
    if !ANY_HOOKS.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let subs: Vec<HookFn> = {
        let reg = hooks().lock().unwrap();
        match reg.get(name) {
            None => return,
            Some(subs) => subs.iter().map(|(f, _)| *f).collect(),
        }
    };

    let cname = CString::new(name).unwrap_or_default();
    let cclient = client.and_then(|s| CString::new(s).ok());
    let csession = session.and_then(|s| CString::new(s).ok());
    let cwindow = window.and_then(|s| CString::new(s).ok());
    let ptr = |c: &Option<CString>| c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

    let event = HookEvent {
        name: cname.as_ptr(),
        client: ptr(&cclient),
        session: ptr(&csession),
        window: ptr(&cwindow),
        window_id,
        pane_id,
    };
    for handler in subs {
        handler(host_api(), &raw const event);
    }
}

/// `(name, version, path)` for each loaded plugin, sorted by name.
pub(crate) fn list() -> Vec<(String, String, String)> {
    let mut v: Vec<(String, String, String)> = plugins()
        .lock()
        .unwrap()
        .iter()
        .map(|p| (p.name.clone(), p.version.clone(), p.path.clone()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// The commands, formats, and hooks `name` currently has registered — what
/// `znative info` reports for a loaded native plugin.
pub(crate) fn registrations(name: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut cmds: Vec<String> = commands()
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, c)| c.owner == name)
        .map(|(n, _)| n.clone())
        .collect();
    let mut fmts: Vec<String> = formats()
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, (_, o))| o == name)
        .map(|(k, _)| k.clone())
        .collect();
    let mut hks: Vec<String> = hooks()
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, subs)| subs.iter().any(|(_, o)| o == name))
        .map(|(h, _)| h.clone())
        .collect();
    cmds.sort();
    fmts.sort();
    hks.sort();
    (cmds, fmts, hks)
}

/// Plugin name → version for every loaded plugin, for the `znative list`
/// "loaded" column.
pub(crate) fn loaded_version(name: &str) -> Option<String> {
    plugins()
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.version.clone())
}

/// The socket this server is listening on, when it has one.
///
/// A script plugin drives the server by running `tmux`, and ztmux adopts
/// `$TMUX` only for a socket in its own default directory (see
/// `socket_from_environment`) — so on a `-S`/`-L` server the plugin's calls
/// would land on the default server instead. Handing the real socket to the
/// shim closes that gap.
pub(crate) fn socket_path() -> Option<String> {
    // Safe: SOCKET_PATH is set once during server start-up and only ever read
    // afterwards; this runs on the server's own thread.
    unsafe {
        let p = crate::tmux::SOCKET_PATH;
        (!p.is_null()).then(|| cstr_to_str(p).to_string())
    }
}

fn cstr_or(p: *const c_char, dflt: &str) -> String {
    if p.is_null() {
        dflt.to_string()
    } else {
        // Safe: `PluginInfo`'s strings are `'static` C strings in the plugin.
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// Expand a leading `~/` so a path typed at the command prompt loads.
pub(crate) fn expand_tilde(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if !home.is_empty() => std::path::PathBuf::from(home)
            .join(rest)
            .to_string_lossy()
            .into_owned(),
        _ => path.to_string(),
    }
}
