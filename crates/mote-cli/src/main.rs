//! The `mote` binary. All logic lives in the library (`mote_cli`).

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::process::ExitCode {
    mote_cli::main()
}
