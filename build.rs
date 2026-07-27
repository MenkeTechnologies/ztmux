fn main() {
    println!("cargo::rerun-if-changed=src/cmd_parse.lalrpop");
    lalrpop::process_root().unwrap();

    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        use std::process::Command;

        fn brew_link_prefix(target: &str) -> PathBuf {
            let output = Command::new("brew")
                .arg("--prefix")
                .arg(target)
                .output()
                .expect("homebrew is not installed");

            assert!(output.status.success(), "`brew --prefix {target}` failed");
            let path = String::from_utf8(output.stdout).unwrap();
            PathBuf::from(path.trim()).join("lib")
        }

        println!("cargo::rerun-if-env-changed=TMUX_RS_DISABLE_HOMEBREW_LIBS");
        if matches!(
            std::env::var("TMUX_RS_DISABLE_HOMEBREW_LIBS"),
            Err(std::env::VarError::NotPresent)
        ) {
            println!(
                "cargo::rustc-link-search={}",
                brew_link_prefix("libevent").display()
            );
        }
    }

    let static_linking = is_static_linking();

    // Everywhere but macOS (which uses the Homebrew prefix above), ask pkg-config
    // where libevent lives. It emits the link lib and search paths itself, so on a
    // successful probe there is nothing left to do. On failure we fall back to a
    // bare `-levent_core` and let `require_libevent()` produce an actionable error
    // if the library is genuinely absent.
    //
    // The target OS comes from cargo's env, not `cfg!`: build scripts are compiled
    // for the host, so `cfg!(target_os = ...)` here answers the wrong question.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        if pkg_config::Config::new()
            .statik(static_linking)
            .probe("libevent_core")
            .is_ok()
        {
            return;
        }
        require_libevent(static_linking);
    }

    if static_linking {
        println!("cargo::rustc-link-lib=static=event_core");
    } else {
        println!("cargo::rustc-link-lib=event_core");
    }
}

/// fail the build with install instructions when libevent's development files are missing
///
/// without this the only symptom is `/usr/bin/ld: cannot find -levent_core` several
/// hundred lines into a link command, which says nothing about which package to install.
/// only a positive absence triggers the error: if the search is inconclusive (cross
/// compiling, or the caller supplied their own `-L` via `RUSTFLAGS`/`LIBRARY_PATH`) the
/// build proceeds and the linker has the final say.
fn require_libevent(static_linking: bool) {
    use std::env;
    use std::path::Path;

    let inconclusive = env::var("HOST") != env::var("TARGET")
        || env::var("LIBRARY_PATH").is_ok_and(|v| !v.is_empty())
        || env::var("CARGO_ENCODED_RUSTFLAGS").is_ok_and(|v| v.contains("-L"));
    if inconclusive {
        return;
    }

    // debian/ubuntu put libraries in a multiarch dir keyed by `<arch>-<os>-<env>`,
    // e.g. aarch64-unknown-linux-gnu -> /usr/lib/aarch64-linux-gnu.
    let target = env::var("TARGET").unwrap_or_default();
    let multiarch: Vec<String> = match target.split('-').collect::<Vec<_>>()[..] {
        [arch, _vendor, os, abi] => vec![format!("{arch}-{os}-{abi}")],
        _ => Vec::new(),
    };

    let mut dirs: Vec<String> = Vec::new();
    for prefix in ["/usr/lib", "/usr/local/lib", "/lib"] {
        dirs.extend(multiarch.iter().map(|m| format!("{prefix}/{m}")));
        dirs.push(prefix.to_string());
    }
    dirs.push("/usr/lib64".to_string());
    dirs.push("/usr/local/lib64".to_string());

    // only the unversioned name counts: `ld -levent_core` resolves the symlink the
    // -dev package installs, not the versioned libevent_core.so.7 the runtime package
    // ships. macOS never reaches here, so .dylib is not a candidate.
    let names: &[&str] = if static_linking {
        &["libevent_core.a"]
    } else {
        &["libevent_core.so"]
    };
    if dirs
        .iter()
        .any(|d| names.iter().any(|n| Path::new(d).join(n).exists()))
    {
        return;
    }

    panic!(
        "libevent development files not found (looked for {names:?} in {dirs:?}).\n\
         ztmux links tmux's event loop, libevent, exactly like tmux does. Install it:\n\
         \x20 Debian/Ubuntu: sudo apt-get install -y libevent-dev libncurses-dev pkg-config\n\
         \x20 Fedora/RHEL:   sudo dnf install -y libevent-devel ncurses-devel pkgconf-pkg-config\n\
         \x20 Arch:          sudo pacman -S libevent ncurses pkgconf\n\
         \x20 Alpine:        sudo apk add libevent-dev ncurses-dev pkgconf\n\
         The runtime package (e.g. libevent-core-2.1-7) is not enough: linking needs the\n\
         libevent_core.so symlink and headers from the -dev/-devel package."
    );
}

/// determine how external c libraries should be linked
///
/// default to static linking on mac and dynamic linking on linux
/// this can be configured with the static or dynamic feature flags
///
/// because feature flags are additive, both can be set at the same time
/// if this is the case, follow the platform default rules
fn is_static_linking() -> bool {
    let mut static_linking;
    if cfg!(target_os = "macos") {
        static_linking = true;
        if cfg!(feature = "dynamic") {
            static_linking = false;
        }
        if cfg!(feature = "static") {
            static_linking = true;
        }
    } else {
        static_linking = false;
        if cfg!(feature = "static") {
            static_linking = true;
        }
        if cfg!(feature = "dynamic") {
            static_linking = false;
        }
    }
    static_linking
}
