; From gpui-kit. A key, and a constant straight inside an array, take the colors today's app gives them.

(pair
  key: (string) @property.json)

(array
  [
    (true)
    (false)
    (null)
  ] @constant.builtin.json)

(comment) @comment

(string) @string
(escape_sequence) @string.escape

(number) @number

[
  (true)
  (false)
] @boolean

(null) @constant.builtin

[
  ","
  ":"
  "{"
  "}"
  "["
  "]"
] @punctuation
