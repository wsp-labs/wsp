; Function and method definitions
;--------------------------------

(function_expression
  name: (identifier) @function)
(function_declaration
  name: (identifier) @function)
(method_definition
  name: (property_identifier) @function.method)

(pair
  key: (property_identifier) @function.method
  value: [(function_expression) (arrow_function)])

(assignment_expression
  left: (member_expression
    property: (property_identifier) @function.method)
  right: [(function_expression) (arrow_function)])

(variable_declarator
  name: (identifier) @function
  value: [(function_expression) (arrow_function)])

(assignment_expression
  left: (identifier) @function
  right: [(function_expression) (arrow_function)])

; Function and method calls
;--------------------------

(call_expression
  function: (identifier) @function)

(call_expression
  function: (member_expression
    property: (property_identifier) @function.method))

; Classes
;--------

(extends_clause
  value: (identifier) @type)
(enum_declaration
  name: (identifier) @type)

; Parameters
;-----------

(required_parameter
  pattern: (identifier) @variable.parameter)
(optional_parameter
  pattern: (identifier) @variable.parameter)
(required_parameter
  pattern: (object_pattern
    (shorthand_property_identifier_pattern) @variable.parameter))
(required_parameter
  pattern: (array_pattern
    (identifier) @variable.parameter))
(arrow_function
  parameter: (identifier) @variable.parameter)

; Special identifiers
;--------------------

([
  (identifier)
  (shorthand_property_identifier)
  (shorthand_property_identifier_pattern)
 ] @constant
 (#match? @constant "^_*[A-Z_][A-Z\\d_]*$"))

(lexical_declaration
  (variable_declarator
    name: (identifier) @function
    value: [(function_expression) (arrow_function)]))

(lexical_declaration
  "const"
  (variable_declarator
    name: [
      (identifier) @constant
      (array_pattern (identifier) @constant)
      (object_pattern (shorthand_property_identifier_pattern) @constant)
    ]))

((identifier) @variable.special
 (#match? @variable.special "^(arguments|module)$")
 (#is-not? local))

(member_expression
  object: (identifier) @_module
  property: (property_identifier) @variable.special
  (#eq? @_module "module"))

((identifier) @function.builtin
 (#eq? @function.builtin "require")
 (#is-not? local))

; Literals
;---------

(this) @variable.special
(super) @variable.special

[
  (true)
  (false)
  (null)
  (undefined)
] @constant.builtin

(comment) @comment

[
  (string)
  (template_string)
] @string

(template_string
  "`" @punctuation)

(regex
  "/" @string)
(regex_pattern) @string.regex
(regex_flags) @keyword
(number) @number

; Tokens
;-------

(ternary_expression
  [
    "?"
    ":"
  ] @keyword)
(property_signature
  "?" @keyword)
(optional_parameter
  "?" @keyword)
(public_field_definition
  "?" @keyword)

[
  ";"
  (optional_chain)
  "."
  ","
  ":"
] @punctuation.delimiter

[
  "-"
  "--"
  "-="
  "+"
  "++"
  "+="
  "*"
  "*="
  "**"
  "**="
  "/"
  "/="
  "%"
  "%="
  "<"
  "<="
  "<<"
  "<<="
  "="
  "=="
  "==="
  "!"
  "!="
  "!=="
  ">"
  ">="
  ">>"
  ">>="
  ">>>"
  ">>>="
  "~"
  "^"
  "&"
  "|"
  "^="
  "&="
  "|="
  "&&"
  "||"
  "??"
  "&&="
  "||="
  "??="
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
]  @punctuation.bracket

(template_substitution
  "${" @punctuation.special
  "}" @punctuation.special) @embedded

(type_alias_declaration
  "type" @keyword.storage)

[
  "=>"
  "class"
  "const"
  "enum"
  "function"
  "get"
  "interface"
  "let"
  "namespace"
  "set"
  "var"
] @keyword.storage

[
  "as"
  "async"
  "await"
  "break"
  "case"
  "catch"
  "continue"
  "debugger"
  "default"
  "delete"
  "do"
  "else"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "if"
  "import"
  "in"
  "instanceof"
  "new"
  "of"
  "return"
  "static"
  "switch"
  "target"
  "throw"
  "try"
  "typeof"
  "void"
  "while"
  "with"
  "yield"
] @keyword

; Types
;------

(type_identifier) @type
(predefined_type) @type.builtin

(type_arguments
  "<" @punctuation.bracket
  ">" @punctuation.bracket)

; Keywords
;---------

[ "abstract"
  "declare"
  "export"
  "implements"
  "keyof"
  "private"
  "protected"
  "public"
  "type"
  "readonly"
  "override"
  "satisfies"
] @keyword

; Variables, last: the first capture of a node wins
;----------

(identifier) @variable

; Properties
;-----------

(property_identifier) @property
(shorthand_property_identifier) @property
(shorthand_property_identifier_pattern) @property
(private_property_identifier) @property
