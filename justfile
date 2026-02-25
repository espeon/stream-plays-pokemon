setup:
    python3 -m venv .venv
    .venv/bin/pip install -r requirements.txt
    git clone --depth 1 --branch master https://github.com/pret/pokeemerald
    cd browser-source && pnpm install

build-ui:
    cd browser-source && pnpm build --outDir ../static

extract-names:
    python3 mapextract/extract_map_names.py

extract-maps:
    cd mapextract && .venv/bin/python extract_maps.py
    mkdir -p browser-source/public/maps
    python3 -c 'import glob,re,shutil; [shutil.copy(f,"browser-source/public/maps/"+re.search(r"map_([0-9A-Fa-f]+)_",f).group(1)+".png") for f in glob.glob("mapextract/output/maps/map_*.png")]'

precommit:
    cargo check
    cargo test --bins
    cargo clippy -- -D warnings

e2e:
    ROM_PATH=./emerald.gba BIOS_PATH=./gba_bios.bin cargo test --features e2e
