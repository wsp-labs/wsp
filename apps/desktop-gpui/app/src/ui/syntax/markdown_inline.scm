; From gpui-kit. Emphasis keeps its delimiters in its own color, as today's app draws them.
[
  (code_span)
] @text.code.span

(code_span_delimiter) @punctuation.delimiter

((emphasis) @emphasis
  (#set! highlight.allow-overlap))

((strong_emphasis) @emphasis.strong
  (#set! highlight.allow-overlap))

[
  (link_destination)
  (uri_autolink)
] @link_uri

[
  (link_label)
  (link_text)
  (image_description)
] @link_text

(inline_link
  [
    "["
    "]"
  ] @punctuation)
