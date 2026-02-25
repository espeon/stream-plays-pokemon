import type { InputRecord } from "../types";
import { ArrowDownIcon, ArrowUpIcon, ArrowLeftIcon, ArrowRightIcon } from "@phosphor-icons/react";

// map input to component
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const INPUT_LABELS: Record<string, () => any> = {
  "a": () => <span className="aspect-square px-1.5 rounded-full bg-accent">A</span>,
  "b": () => <span className="aspect-square px-1.5 rounded-full bg-accent">B</span>,
  "start": () => <span className="px-1.5 rounded-full bg-accent">START</span>,
  "select": () => <span className="aspect-square px-1.5 rounded-full bg-accent">SELECT</span>,
  "up": () => <span className="px-1 rounded-full bg-accent"><ArrowUpIcon className="h-4 w-3.75 mb-1 inline" weight="bold" /></span>,
  "down": () => <span className="px-1 rounded-full bg-accent"><ArrowDownIcon  className="h-4 w-3.75 mb-1 inline" weight="bold" /></span>,
  "left": () => <span className="px-1 rounded-full bg-accent"><ArrowLeftIcon  className="h-4 w-3.75 mb-1 inline" weight="bold"  /></span>,
  "right": () => <span className="px-1 rounded-full bg-accent"><ArrowRightIcon  className="h-4 w-3.75 mb-1 inline" weight="bold"  /></span>,
}

export default function InputRow({ record, index }: { record: InputRecord; index: number }) {
  const opacity = Math.max(0.2, 1 - index * 0.045);
  return (
    <div className="flex items-baseline gap-1.5" style={{ opacity }}>
      <span className="text-white/80 truncate max-w-40 leading-none">{record.user}</span>
      <span className="font-medium text-base text-foreground/60">{INPUT_LABELS[record.input]()}</span>
    </div>
  );
}
