// SPDX-License-Identifier: AGPL-3.0-only
// Each compared tool's own app icon, by its /compare slug, as its site serves it (Codex and Emdash drawn from their
// marks). They sit in public/logos and not in the bundle, which would inline these small files in the browser alone.
import { Tilde } from "@/components/brand";
import { cn } from "@/lib/utils";

export function ToolIcon({ slug, className }: { slug: string; className?: string }) {
  return <img src={`/logos/${slug}.png`} alt="" width={40} height={40} loading="lazy" className={cn("size-10 shrink-0 rounded-[10px]", className)} />;
}

/** wsp's own tile, the size of a tool's icon: the tilde of its brand avatar on the page's raised ground. */
export function WspIcon({ className }: { className?: string }) {
  return (
    <span className={cn("grid size-10 shrink-0 place-items-center rounded-[10px] border border-border bg-raised text-foreground", className)}>
      <Tilde className="w-[58%]" />
      <span className="sr-only">wsp</span>
    </span>
  );
}
