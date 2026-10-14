// SPDX-License-Identifier: AGPL-3.0-only
import { ArrowRight } from "lucide-react";
import { Head, Section } from "@/components/section";
import { KINDS } from "@/compare";
import { ToolIcon } from "@/logos";

const TOOLS = KINDS.flatMap(kind => kind.tools);

export function Compares() {
  return (
    <Section id="compare">
      <Head title="How wsp compares.">Side by side with the apps and clouds people ask about.</Head>
      <ul className="mt-12 grid grid-cols-2 gap-3 lg:grid-cols-5">
        {TOOLS.map(tool => (
          <li key={tool.slug}>
            <a
              href={`/compare/${tool.slug}`}
              className="group flex h-[76px] items-center gap-3 rounded-[14px] border border-border px-3.5 transition-colors sm:gap-3.5 sm:px-4 duration-150 hover:bg-raised"
            >
              <ToolIcon slug={tool.slug} className="size-8 rounded-[8px] sm:size-10 sm:rounded-[10px]" />
              <span className="line-clamp-2 min-w-0 flex-1 text-[14px] leading-snug text-foreground sm:text-[15px]">{tool.name}</span>
              <ArrowRight className="hidden size-4 shrink-0 text-faint sm:block transition-colors duration-150 group-hover:text-foreground" />
            </a>
          </li>
        ))}
      </ul>
    </Section>
  );
}
