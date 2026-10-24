; From tree-sitter-bash, in the names today's app colors shell with. The first capture of a node wins.

(comment) @comment

[
  (string)
  (raw_string)
  (heredoc_body)
  (heredoc_start)
] @string

(function_definition
  name: (word) @function)
(command_name) @function

((command
  argument: (word) @constant.other)
 (#match? @constant.other "^-"))
(command
  argument: (word) @string)
(for_statement
  value: (word) @string)
(file_redirect
  destination: (word) @string)

[
  (number)
  (file_descriptor)
] @number

((simple_expansion) @variable.parameter
 (#match? @variable.parameter "^\\$[0-9]+$"))
(simple_expansion) @variable
(variable_assignment
  name: (variable_name) @variable)
(for_statement
  variable: (variable_name) @variable)
(expansion
  (variable_name) @variable)

(expansion
  [
    "${"
    "}"
  ] @punctuation)

(variable_assignment
  "=" @operator)
(test_operator) @operator

[
  "case"
  "do"
  "done"
  "elif"
  "else"
  "esac"
  "export"
  "fi"
  "for"
  "function"
  "if"
  "in"
  "select"
  "then"
  "unset"
  "until"
  "while"
] @keyword

(test_command
  [
    "["
    "]"
    "[["
    "]]"
  ] @punctuation.bracket)

[
  "|"
  "&&"
  "||"
  ">"
  ">>"
  "<"
] @punctuation

[
  ";"
  ";;"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
] @punctuation.bracket
