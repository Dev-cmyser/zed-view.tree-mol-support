; Основные элементы view.tree
((identifier) @variable)
((identifier) @type)

; mol definitions
((node_id) @function)
(reference) @variable.special

; String literals
(string) @string

; Special symbols
["$" "*" "/" "<=" "-" "@" "\\" ] @punctuation.special

; Keywords
((word) @keyword
  (#match? @keyword "^(arg|minimal_width|minimal_height|plugins|title|tools|uri|text|sub|rows|body)$"))
