build-ui:
    cd browser-source && pnpm build --outDir ../static

precommit:
    cargo check
    cargo test --bins
    cargo clippy -- -D warnings

e2e:
    ROM_PATH=./tests/fixtures/emerald.gba BIOS_PATH=./tests/fixtures/gba_bios.bin cargo test --features e2e
