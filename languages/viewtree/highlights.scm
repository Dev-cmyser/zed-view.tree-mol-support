; Highlights for view.tree syntax

; Component names (starting with $)
(component_name) @type

; Binding operators
(binding_operator) @operator

; Comments - recursively apply to all commented content
(commented_line) @comment
(comment_marker) @comment
(comment_content) @comment
(commented_child_line) @comment

; Typed collections
(typed_list) @type.builtin
(typed_dict) @type.builtin

; List marker
(list_marker) @keyword

; Dictionary markers
(dict_marker) @keyword

; Property modifiers
(property_modifier) @operator

; Localized strings - entire node as special
(localized_string) @string.special

; Localized text (inside @ strings) - also special
(localized_text) @string.special

; String literals - general rule
(string_literal) @string

; Numbers
(number) @number

; Booleans
(boolean) @boolean

; Null
(null) @constant.builtin

; Plain identifiers (properties, variables)
(identifier) @variable
