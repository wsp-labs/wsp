; JSX, ahead of the JavaScript or TypeScript query: the first capture of a node wins

(jsx_opening_element
  name: (identifier) @tag
  (#match? @tag "^[a-z][^.]*$"))
(jsx_closing_element
  name: (identifier) @tag
  (#match? @tag "^[a-z][^.]*$"))
(jsx_self_closing_element
  name: (identifier) @tag
  (#match? @tag "^[a-z][^.]*$"))

(jsx_opening_element
  name: (identifier) @type)
(jsx_closing_element
  name: (identifier) @type)
(jsx_self_closing_element
  name: (identifier) @type)

(jsx_attribute
  (property_identifier) @attribute)
(jsx_attribute
  "=" @operator)

(jsx_expression
  [
    "{"
    "}"
  ] @punctuation.special)

(jsx_opening_element
  [
    "<"
    ">"
  ] @punctuation.bracket)
(jsx_closing_element
  [
    "</"
    ">"
  ] @punctuation.bracket)
(jsx_self_closing_element
  [
    "<"
    "/>"
  ] @punctuation.bracket)
