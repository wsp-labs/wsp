// SPDX-License-Identifier: AGPL-3.0-only
// Each compared tool's own app icon, by its /compare slug, as its site serves it (Codex and Emdash drawn from their
// marks). They sit in public/logos and not in the bundle, which would inline these small files in the browser alone.
import { cn } from "@/lib/utils";

export function ToolIcon({ slug, className }: { slug: string; className?: string }) {
  return <img src={`/logos/${slug}.png`} alt="" width={40} height={40} loading="lazy" className={cn("size-10 shrink-0 rounded-[10px]", className)} />;
}
