// SPDX-License-Identifier: AGPL-3.0-only
import { ArrowLeft } from "lucide-react";
import { useEffect, useState, type MouseEvent } from "react";
import { Section } from "@/components/section";
import { day, POSTS, type Post } from "@/blog";

export function BlogIndex() {
  return (
    <main>
      <Section className="pt-24 sm:pt-40">
        <div className="mx-auto max-w-[680px]">
          <h1 className="heading text-[34px] sm:text-[48px]">Blog</h1>
          <p className="lede mt-5 text-[17px]">What we build, what broke, and what we learned shipping wsp.</p>
          {POSTS.length === 0 ? (
            <p className="mt-16 text-[15px] text-muted-foreground">The first post is on its way.</p>
          ) : (
            <ol className="mt-14">
              {POSTS.map(post => (
                <li key={post.slug} className="border-t border-rule">
                  <a href={`/blog/${post.slug}`} className="group block py-7">
                    <time dateTime={post.date} className="text-[14px] text-muted-foreground">
                      {day(post.date)}
                    </time>
                    <h2 className="mt-2 text-[20px] font-medium tracking-[-0.01em] text-foreground transition-colors duration-150 group-hover:text-foreground/80">{post.title}</h2>
                    <p className="mt-2 text-[15.5px] leading-relaxed text-muted-foreground">{post.summary}</p>
                  </a>
                </li>
              ))}
            </ol>
          )}
        </div>
      </Section>
    </main>
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
        <article className="mx-auto max-w-[680px]">
          <a href="/blog" className="hidden items-center gap-1.5 text-[14px] text-muted-foreground transition-colors duration-150 hover:text-foreground sm:inline-flex">
            <ArrowLeft className="size-3.5" /> All posts
          </a>
          <h1 className="heading sm:mt-4 text-[32px] leading-[1.1] sm:text-[48px]">{post.title}</h1>
          <p className="lede mt-4 text-[17px] sm:mt-5 sm:text-[18px]">{post.summary}</p>
          <p className="mt-5 text-[14px] text-muted-foreground sm:mt-6">
            {post.author !== null && (post.author.url === null ? `${post.author.name}, ` : <><a href={post.author.url} className="transition-colors duration-150 hover:text-foreground">{post.author.name}</a>, </>)}
            <time dateTime={post.date}>{day(post.date)}</time>
          </p>
          {post.cover !== null && (
            <img src={post.cover} alt={post.coverAlt} onClick={() => setZoomed({ src: post.cover!, alt: post.coverAlt })} className="mt-8 w-full cursor-zoom-in max-w-none rounded-[12px] border border-border sm:mt-10 lg:-mx-[100px] lg:w-[calc(100%+200px)]" />
          )}
          <div className="legal post mt-10 sm:mt-12" onClick={open} dangerouslySetInnerHTML={{ __html: post.html }} />
        </article>
      </Section>
      {zoomed !== null && <Lightbox zoomed={zoomed} close={() => setZoomed(null)} />}
    </main>
  );
}
