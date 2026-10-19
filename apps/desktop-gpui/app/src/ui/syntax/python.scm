; From tree-sitter-python. The first capture of a node wins; names and attributes are plain outside a call's arguments.

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

((identifier) @variable.special
 (#match? @variable.special "^(self|cls)$"))

((identifier) @constant.other
 (#match? @constant.other "^[A-Z][A-Z_]*$"))

(attribute
  attribute: (identifier) @none)

((identifier) @type.builtin.python
 (#match? @type.builtin.python "^(bool|bytearray|bytes|classmethod|complex|dict|float|frozenset|int|list|object|property|set|slice|staticmethod|str|tuple|type|super)$"))

(call
  function: (identifier) @function)

((parameters
  (identifier) @variable.parameter.python)
 (#not-match? @variable.parameter.python "^(self|cls)$"))
(lambda_parameters
  (identifier) @variable.parameter.python)
(typed_parameter
  (identifier) @variable.parameter.python)
(default_parameter
  name: (identifier) @variable.parameter.python)
(typed_default_parameter
  name: (identifier) @variable.parameter.python)
(keyword_argument
  name: (identifier) @variable.parameter)
(default_parameter
  "=" @punctuation)
(typed_default_parameter
  "=" @punctuation)

; What a call is given, short of its own color, is drawn as the call
(call
  arguments: (argument_list) @function)

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
