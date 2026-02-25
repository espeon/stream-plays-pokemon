//! Headless smoke-test: load a ROM, run N frames, write each as a JPEG.
//!
//! Usage:
//!   cargo run --bin render-frames -- \
//!     --bios path/to/gba_bios.bin \
//!     --rom  path/to/emerald.gba  \
//!     --out  /tmp/frames          \
//!     --frames 300                \
//!     --every 60

use std::path::PathBuf;

use anyhow::Context;
use rustboyadvance_ng::prelude::{GameBoyAdvance, GamepakBuilder, NullAudio};
use stream_plays_emerald::emulator::frame::{encode_jpeg, to_rgb, DISPLAY_HEIGHT, DISPLAY_WIDTH};

struct Args {
    bios: PathBuf,
    rom: PathBuf,
    out: PathBuf,
    frames: u64,
    every: u64,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut bios = None;
    let mut rom = None;
    let mut out = PathBuf::from("/tmp/gba-frames");
    let mut frames = 300u64;
    let mut every = 1u64;

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bios" => bios = Some(PathBuf::from(args.next().context("--bios needs a value")?)),
            "--rom" => rom = Some(PathBuf::from(args.next().context("--rom needs a value")?)),
            "--out" => out = PathBuf::from(args.next().context("--out needs a value")?),
            "--frames" => frames = args.next().context("--frames needs a value")?.parse()?,
            "--every" => every = args.next().context("--every needs a value")?.parse()?,
            other => anyhow::bail!("unknown flag: {other}"),
        }
    }

    Ok(Args {
        bios: bios.context("--bios is required")?,
        rom: rom.context("--rom is required")?,
        out,
        frames,
        every,
    })
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;

    std::fs::create_dir_all(&args.out)
        .with_context(|| format!("creating output dir {}", args.out.display()))?;

    let bios = std::fs::read(&args.bios)
        .with_context(|| format!("reading bios {}", args.bios.display()))?
        .into_boxed_slice();

    let cartridge = GamepakBuilder::new()
        .file(&args.rom)
        .without_backup_to_file()
        .build()
        .map_err(|e| anyhow::anyhow!("loading ROM: {e}"))?;

    let mut gba = GameBoyAdvance::new(bios, cartridge, NullAudio::new());
    gba.skip_bios();

    println!(
        "running {} frames, saving every {}th to {}",
        args.frames,
        args.every,
        args.out.display()
    );

    let mut saved = 0u64;
    for i in 0..args.frames {
        gba.frame();

        if i % args.every == 0 {
            let raw = gba.get_frame_buffer();
            let rgb = to_rgb(raw);
            let jpeg = encode_jpeg(&rgb, DISPLAY_WIDTH, DISPLAY_HEIGHT, 85)
                .map_err(|e| anyhow::anyhow!("jpeg encode: {e}"))?;

            let path = args.out.join(format!("frame_{i:06}.jpg"));
            std::fs::write(&path, &jpeg)?;
            saved += 1;

            if saved.is_multiple_of(30) || i == 0 {
                println!("  wrote {}", path.display());
            }
        }
    }

    println!("done — wrote {saved} frames to {}", args.out.display());
    Ok(())
}
