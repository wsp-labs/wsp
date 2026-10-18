; From tree-sitter-python. The first capture of a node wins; plain names and attributes stay uncolored.

(class_definition
  name: (identifier) @type)
(class_definition
  superclasses: (argument_list
    (identifier) @type))

(function_definition
  name: (identifier) @function)

(decorator) @function
(decorator
  (identifier) @function)

(call
  function: (attribute attribute: (identifier) @function.method))
(call
  function: (identifier) @function)

((identifier) @variable.special
 (#match? @variable.special "^(self|cls)$"))

((identifier) @constant
 (#match? @constant "^[A-Z][A-Z_]*$"))

; Literals

[
  (none)
  (true)
  (false)
] @constant.builtin

[
  (integer)
  (float)
] @number

(comment) @comment
(string) @string
(escape_sequence) @string.escape

(interpolation
  "{" @punctuation.special
  "}" @punctuation.special) @embedded

[
  "-"
  "-="
  "!="
  "*"
  "**"
  "**="
  "*="
  "/"
  "//"
  "//="
  "/="
  "&"
  "&="
  "%"
  "%="
  "^"
  "^="
  "+"
  "+="
  "<"
  "<<"
  "<<="
  "<="
  "<>"
  "="
  ":="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "|"
  "|="
  "~"
  "@="
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  "."
  ":"
  ";"
  "->"
] @punctuation.delimiter

[
  "class"
  "def"
  "lambda"
] @keyword.storage

[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "case"
  "continue"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "in"
  "is"
  "is not"
  "match"
  "nonlocal"
  "not"
  "not in"
  "or"
  "pass"
  "print"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
] @keyword
