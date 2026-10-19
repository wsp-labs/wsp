; From tree-sitter-css, in the names today's app colors CSS with. The first capture of a node wins.

(comment) @comment

(pseudo_class_selector
  (class_name) @attribute.pseudo)
(pseudo_element_selector
  (tag_name) @attribute.pseudo)

[
  (tag_name)
  (nesting_selector)
  (universal_selector)
] @tag

(id_name) @attribute.id
[
  (class_name)
  (attribute_name)
] @attribute

((property_name) @variable
 (#match? @variable "^--"))
((plain_value) @variable
 (#match? @variable "^--"))

(property_name) @property.css
(feature_name) @property.name

(function_name) @function

[
  (color_value)
  (plain_value)
] @constant.other

(string_value) @string

[
  (integer_value)
  (float_value)
] @number
(unit) @keyword.unit

[
  "@media"
  "@import"
  "@charset"
  "@namespace"
  "@supports"
  "@keyframes"
  (at_keyword)
  (to)
  (from)
  (important)
] @keyword

(binary_expression
  [
    "+"
    "-"
    "*"
    "/"
  ] @operator)

[
  "and"
  "or"
  "not"
  "only"
] @operator

[
  "~"
  ">"
  "+"
  "="
  "^="
  "|="
  "~="
  "$="
  "*="
] @punctuation

(id_selector
  "#" @punctuation.delimiter)

[
  "."
  ","
  ":"
  "::"
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket
