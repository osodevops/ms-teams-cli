//! Windows reserves 1 MiB of stack for the main thread; Linux and macOS give
//! it 8 MiB. Building clap's command tree for this many subcommands uses
//! almost all of that 1 MiB in an unoptimized build (measured at just under
//! it on 2026-09-06), so on Windows every debug or test invocation of `teams`
//! — `--help` included — overflowed the stack as soon as one more flag was
//! added to `message`, and `cargo test` failed there while passing elsewhere.
//!
//! Reserving the same 8 MiB the Unix targets get removes the cliff. It is
//! address space, not committed memory, so an idle process costs nothing
//! extra. rustup does the same for the same reason (clap's debug-mode stack
//! use, clap-rs/clap#5134). A build script survives CI overriding `RUSTFLAGS`, which a
//! `.cargo/config.toml` `rustflags` entry would not.

use std::env;

const MAIN_THREAD_STACK_BYTES: u32 = 8 * 1024 * 1024;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => println!("cargo:rustc-link-arg=/STACK:{MAIN_THREAD_STACK_BYTES}"),
        Ok("gnu") => println!("cargo:rustc-link-arg=-Wl,--stack,{MAIN_THREAD_STACK_BYTES}"),
        _ => {}
    }
}
