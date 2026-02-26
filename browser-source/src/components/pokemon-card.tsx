import { useEffect, useState } from "react";
import type { PartyPokemon } from "../types";

const TYPE_COLORS: Record<string, string> = {
  normal:   "bg-[#A8A878] text-white",
  fire:     "bg-[#F08030] text-white",
  water:    "bg-[#6890F0] text-white",
  electric: "bg-[#F8D030] text-black",
  grass:    "bg-[#78C850] text-white",
  ice:      "bg-[#98D8D8] text-black",
  fighting: "bg-[#C03028] text-white",
  poison:   "bg-[#A040A0] text-white",
  ground:   "bg-[#E0C068] text-black",
  flying:   "bg-[#A890F0] text-white",
  psychic:  "bg-[#F85888] text-white",
  bug:      "bg-[#A8B820] text-white",
  rock:     "bg-[#B8A038] text-white",
  ghost:    "bg-[#705898] text-white",
  dragon:   "bg-[#7038F8] text-white",
  dark:     "bg-[#705848] text-white",
  steel:    "bg-[#B8B8D0] text-black",
};

const typeCache = new Map<number, string[]>();

function usePokeTypes(dexNum: number): string[] {
  const [types, setTypes] = useState<string[]>(() => typeCache.get(dexNum) ?? []);

  useEffect(() => {
    if (typeCache.has(dexNum)) {
      setTypes(typeCache.get(dexNum)!);
      return;
    }
    let cancelled = false;
    fetch(`https://pokeapi.co/api/v2/pokemon/${dexNum}`)
      .then((r) => r.json())
      .then((data) => {
        const t: string[] = (data.types as { slot: number; type: { name: string } }[])
          .sort((a, b) => a.slot - b.slot)
          .map((e) => e.type.name);
        typeCache.set(dexNum, t);
        if (!cancelled) setTypes(t);
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [dexNum]);

  return types;
}

function statusLabel(status: number): string {
  if (status === 0) return "";
  if ((status & 0x07) > 0) return "SLP";
  if (status & 0x08) return "PSN";
  if (status & 0x10) return "BRN";
  if (status & 0x20) return "FRZ";
  if (status & 0x40) return "PAR";
  if (status & 0x80) return "PSN"; // bad poison
  return "";
}

function PokemonRow({ mon }: { mon: PartyPokemon }) {
  const hpPct = mon.max_hp > 0 ? mon.current_hp / mon.max_hp : 0;
  const fainted = mon.current_hp === 0;
  const hpColor = hpPct > 0.5 ? "bg-emerald-500" : hpPct > 0.2 ? "bg-yellow-400" : "bg-red-500";
  const status = statusLabel(mon.status);
  const species = mon.species - 25; // first 25 are dummy entries in the game's data, so actual species start at index 25
  const types = usePokeTypes(species);

  return (
    <div className={`flex flex-col gap-1.5 min-w-0 px-2 rounded-2xl bg-muted ${fainted ? "opacity-40" : ""}`}>
      <div className="flex flex-col items-start justify-around min-w-0">
        <div className="h-32 mt-2 self-center p-4 border rounded-full mb-1" style={{ backgroundColor: (TYPE_COLORS[types[0]]?.split(" ")[0].replace('bg-[', '').replace(']', '') ?? "#000") + "aa" }}>
          <img src={`https://raw.githubusercontent.com/HybridShivam/Pokemon/refs/heads/master/assets/thumbnails/${species}.png`} alt={mon.nickname} className="h-full" />
        </div>
        <div className="font-semibold text-sm truncate flex justify-start items-center gap-1 w-full">{mon.nickname || `#${mon.species}`}        {types.length > 0 &&
            types.map((t) => (
              <span key={t} className={`text-[9px] px-1.5 py-px rounded font-medium uppercase tracking-wide ${TYPE_COLORS[t] ?? "bg-zinc-600 text-white"}`}>{t}</span>
            )
        )}</div>
        <div className="text-foreground text-xs tabular-nums shrink-0">Lv. {mon.level} {status && (
          <span className="text-[10px] px-1 py-px rounded bg-yellow-800/60 text-yellow-300 shrink-0">{status}</span>
        )}</div>
      </div>
      <div className="flex items-center gap-1">
        <div className="flex-1 h-1.5 rounded-full bg-zinc-700 overflow-hidden">
          <div className={`h-full rounded-full ${hpColor}`} style={{ width: `${hpPct * 100}%` }} />
        </div>
        <span className="text-[10px] tabular-nums text-white/40 shrink-0">
          {mon.current_hp}/{mon.max_hp}
        </span>
      </div>
    </div>
  );
}

export function PartyPanel({ party }: { party: PartyPokemon[] }) {
  if (party.length === 0) {
    return (
      <div className="w-full h-full flex items-center justify-center rounded-xl bg-muted/60 border border-white/8">
        <span className="text-white/15 text-xs uppercase tracking-widest">party</span>
      </div>
    );
  }

  const slots = Array.from({ length: 6 }, (_, i) => party[i] ?? null);

  return (
    <div className="w-full h-full rounded-xl bg-muted/70 border border-white/8 p-3 grid grid-cols-6 gap-3 overflow-hidden">
      {slots.map((mon, i) =>
        mon ? (
          <PokemonRow key={i} mon={mon} />
        ) : (
          <div key={i} className="flex-1 min-w-0 rounded-2xl bg-muted/30" />
        )
      )}
    </div>
  );
}
