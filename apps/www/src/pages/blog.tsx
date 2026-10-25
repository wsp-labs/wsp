// SPDX-License-Identifier: AGPL-3.0-only
import { useEffect, useState, type MouseEvent } from "react";
import { Section } from "@/components/section";
import { Field } from "@/components/field";
import { Install } from "@/components/download";
import { day, POSTS, type Post } from "@/blog";

export function BlogIndex() {
  const [lead, ...rest] = POSTS;
  return (
    <main>
      <Section className="pt-24 sm:pt-36">
        <h1 className="heading text-[34px] sm:text-[48px]">Blog</h1>
        <p className="lede mt-4 max-w-[52ch] text-[17px]">What we build, what broke, and what we learned shipping wsp.</p>
        {lead === undefined ? (
          <p className="mt-16 text-[15px] text-muted-foreground">The first post is on its way.</p>
        ) : (
          <>
            <a href={`/blog/${lead.slug}`} className="group mt-12 grid overflow-hidden rounded-[20px] border border-border bg-[rgb(236_230_220/0.02)] transition-colors duration-150 hover:border-[#3a342e] md:grid-cols-[1.25fr_1fr]">
              <Picture post={lead} className="aspect-[1200/630] self-center" />
              <div className="flex flex-col justify-center gap-4 p-7 sm:p-10">
                <Meta post={lead} />
                <h2 className="heading text-[26px] leading-[1.15] sm:text-[34px]">{lead.title}</h2>
                <p className="text-[15.5px] leading-relaxed text-muted-foreground">{lead.summary}</p>
                <span className="text-[14px] text-foreground/80 group-hover:text-foreground">Read the post</span>
              </div>
            </a>
            {rest.length > 0 && (
              <ol className="mt-6 grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                {rest.map(post => (
                  <li key={post.slug}>
                    <a href={`/blog/${post.slug}`} className="group flex h-full flex-col overflow-hidden rounded-[16px] border border-border transition-colors duration-150 hover:border-[#3a342e]">
                      <Picture post={post} className="aspect-[1200/630]" />
                      <div className="flex flex-1 flex-col gap-3 p-6">
                        <Meta post={post} />
                        <h2 className="text-[18px] font-medium leading-snug tracking-[-0.01em] text-foreground">{post.title}</h2>
                        <p className="text-[14.5px] leading-relaxed text-muted-foreground">{post.summary}</p>
                      </div>
                    </a>
                  </li>
                ))}
              </ol>
            )}
          </>
        )}
      </Section>
    </main>
  );
}

/** A post's picture on the index: its preview image, its cover, or the card the build drew for it. */
function Picture({ post, className }: { post: Post; className?: string }) {
  const src = post.image ?? post.cover ?? `/og-blog-${post.slug}.png`;
  return (
    <div className={`overflow-hidden bg-[#1a1816] ${className ?? ""}`}>
      <img src={src} alt="" loading="lazy" className="size-full object-cover transition-transform duration-300 group-hover:scale-[1.02]" />
    </div>
  );
}

function Meta({ post }: { post: Post }) {
  return (
    <p className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[13px] text-muted-foreground">
      <time dateTime={post.date}>{day(post.date)}</time>
      <span>{minutes(post.html)} min read</span>
    </p>
  );
}

/** A YouTube card's press swaps its thumbnail for the player, so YouTube loads only for a reader who asks for it. */
function play(event: MouseEvent<HTMLDivElement>) {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-youtube]");
  if (button === null) return;
  const frame = document.createElement("iframe");
  frame.src = `https://www.youtube-nocookie.com/embed/${button.dataset["youtube"]}?autoplay=1`;
  frame.title = button.getAttribute("aria-label") ?? "YouTube video";
  frame.allow = "autoplay; encrypted-media; picture-in-picture; fullscreen";
  frame.allowFullscreen = true;
  button.replaceWith(frame);
}

type Zoomed = { src: string; alt: string };

/** A picture opened over the dimmed page at its own size, up to the window; a click anywhere or Esc closes it. */
function Lightbox({ zoomed, close }: { zoomed: Zoomed; close: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);
  return (
    <div role="dialog" aria-modal="true" aria-label={zoomed.alt || "Picture"} onClick={close} className="lightbox fixed inset-0 z-50 flex cursor-zoom-out items-center justify-center bg-background/90 p-4 backdrop-blur-sm sm:p-10">
      <img src={zoomed.src} alt={zoomed.alt} className="max-h-full max-w-full rounded-[12px] border border-border object-contain" />
    </div>
  );
}

export function BlogPost({ post }: { post: Post }) {
  const [zoomed, setZoomed] = useState<Zoomed | null>(null);
  const open = (event: MouseEvent<HTMLDivElement>) => {
    play(event);
    const img = (event.target as HTMLElement).closest("img");
    if (img === null || img.closest("a, button") !== null) return;
    setZoomed({ src: img.currentSrc || img.src, alt: img.alt });
  };
  return (
    <main>
      <Section className="pt-24 sm:pt-32">
        <article className="relative mx-auto max-w-[680px] lg:max-w-[760px]">
          <a href="/blog" className="crumbs inline-block">Blog</a>
          <h1 className="heading mt-4 text-[32px] leading-[1.1] sm:text-[48px]">{post.title}</h1>
          <div className="mt-8 flex items-center justify-between gap-4 border-b border-rule pb-6 text-[14px] text-muted-foreground">
            {post.author !== null ? (
              <a href={post.author.url ?? "/blog"} className="flex items-center gap-2.5 transition-colors duration-150 hover:text-foreground">
                {post.author.avatar !== null && <img src={post.author.avatar} alt="" className="size-6 rounded-full" />}
                {post.author.name}
              </a>
            ) : (
              <span>wsp labs</span>
            )}
            <span className="flex items-center gap-4">
              <time dateTime={post.date}>{day(post.date)}</time>
              <span>{minutes(post.html)} min read</span>
            </span>
          </div>
          {post.cover !== null && (
            <img src={post.cover} alt={post.coverAlt} onClick={() => setZoomed({ src: post.cover!, alt: post.coverAlt })} className="mt-8 w-full cursor-zoom-in max-w-none rounded-[12px] border border-border sm:mt-10 lg:-mx-[100px] lg:w-[calc(100%+200px)]" />
          )}
          <div className="legal post mt-10 sm:mt-12" onClick={open} dangerouslySetInnerHTML={{ __html: post.html }} />
        </article>
      </Section>
      <More current={post.slug} />
      <PostClose />
      {zoomed !== null && <Lightbox zoomed={zoomed} close={() => setZoomed(null)} />}
    </main>
  );
}

/** Reading time at 230 words a minute, off the post's own text. */
function minutes(html: string): number {
  const words = html.replace(/<[^>]+>/g, " ").split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.round(words / 230));
}

/** Up to three other posts, newest first; nothing while this is the only one. */
function More({ current }: { current: string }) {
  const others = POSTS.filter(p => p.slug !== current && !p.draft).slice(0, 3);
  if (others.length === 0) return null;
  return (
    <Section className="pt-0 sm:pt-0">
      <div className="flex items-baseline justify-between border-t border-rule pt-10">
        <h2 className="heading text-[24px] sm:text-[28px]">More from the blog</h2>
        <a href="/blog" className="text-[14px] text-muted-foreground transition-colors duration-150 hover:text-foreground">All posts</a>
      </div>
      <ol className="mt-8 grid gap-6 md:grid-cols-3">
        {others.map(p => (
          <li key={p.slug}>
            <a href={`/blog/${p.slug}`} className="group block">
              <p className="text-[17px] font-medium tracking-[-0.01em] text-foreground group-hover:underline">{p.title}</p>
              <p className="mt-2 text-[14px] text-muted-foreground">{day(p.date)}</p>
            </a>
          </li>
        ))}
      </ol>
    </Section>
  );
}

/** The ask under every post: a card, not a page section, so it reads as the site's and not the post's. */
function PostClose() {
  return (
    <Section className="pt-0 sm:pt-0">
      <div className="relative isolate overflow-hidden rounded-[20px] border border-border px-6 py-16 text-center sm:py-20">
        <div className="absolute inset-0 -z-10 [mask-image:radial-gradient(80%_90%_at_50%_100%,black_15%,transparent_75%)]">
          <Field seed={13} />
        </div>
        <h2 className="display mx-auto max-w-[16ch] text-[32px] sm:text-[48px]">Put your other computers to work.</h2>
        <p className="lede mx-auto mt-4 max-w-[46ch] text-[16px] sm:text-[17px]">Run Claude Code, Codex, OpenCode and Cursor on every computer you own. Free and open source.</p>
        <Install align="center" className="mt-8" />
      </div>
    </Section>
  );
}
