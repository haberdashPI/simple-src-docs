# `simple-src-docs`

simple-src-docs is tool to facilitate generating documentation from your source files

```
Extracts doc strings into markdown files

Walks through all files in `<SOURCE>` and searches for comments that start with `<START_COMMENT>` regex, and
ending with `<END_COMEMNT>` regex. Looks for `@file [file]` on the line following `<START_COMMENT>` and if
present the contents of the comment are appended to the specified file path. The file and its directories are
created at the given `<DEST>`. Optionally, after the line with `@file` you can provide `@order [num]` to
influence the ordering of the comment content. Content is sorted from the lowest to the highest `order`,
breaking ties by pre-sorted ordering. Additional `@` prefixed tags will be ignored in the output, unless you
define an appropriate configuration template (See README.md for details)

Usage: simple-src-docs [OPTIONS] <SOURCE>

Arguments:
  <SOURCE>
          the source directory where comments will be extracted from

Options:
  -s, --start-comment <START_COMMENT>
          regex for the starting comment delimiter

          [default: ^\s*/\*\*\s*$]

  -e, --end-comment <END_COMMENT>
          regex for the ending comment delimiter

          [default: ^\s*\*/\s*$]

  -c, --comment-prefix <COMMENT_PREFIX>
          the prefix to be removed from each comment line between the start and end comment delimiter; the
          first capture group should denote the prefix, and the second the text to read

          [default: ^\s*\*+\s?(.*)$]

  -d, --dest <DEST>
          root directory where markdown files are generated

          [default: .]

  -v, --verbose
          if set, show detailed messages about document processing

      --config <CONFIG>
          location of file used to further configure simple-src-docs (see README.md), defaults to
          `<DEST>/.simple-src-docs.config.toml

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

The config file is a TOML file that can be used to configure how comments are identified and define tempplates that will transform documentation blocks based on tags (`@` prefixed identifiers followed by a line of text). It has the following specification:

- `header`: must contain a `version` string that is [semver](https://semver.org/) compatible with 0.2
- `template`: Object used to transform docs with a given set of tags. There are two fields:
   - `foreach`: an array of templates that are applied to each document block. All fields can be specified as [mustache template](https://mustache.github.io/) strings. Each mustache field in the template corresponds to one of the tags from the original document block. In addition the field `__body__` can be used at include
     the remaining text from a comment block.
        - `tags`: an array of strings. This template will apply to any document block where all the specified tags are present.
        - `file`: the file to store output in
        - `order`: the order of this template output relative to other document blocks
        - `output`: the resulting text output to write to the file
   - `all`: an array of templates that are applied across all document blocks. All fields can be specified as [mustache template](https://mustache.github.io/) strings. There is a single mustache variable named `items`, an array whose items correspond to the tags and the `__body__` in the original document block. This includes the same fields as
     `foreach`:
        - `tags`: an array of strings. This template will apply to any document block where all the specified tags are present.
        - `file`: the file to store output in
        - `order`: the order of this template output relative to other document blocks
        - `output`: the resulting text output to write to the file
- `comment`: defines how comments are found in a given type of file
   - `extension`: a [glob](https://github.com/olson-sean-k/wax/blob/master/README.md#patterns) expression describing what files this comment type describe, which will be prefixed with `(?i)`, making it case insensitive.
   - `start`: the starting regular expression for a multi-line comment
   - `each_line`: The prefix that must be present (for single-line comments) and that can be present for multi-line comments. The first capture group must contain the contents of the comment
   - `end`: The the ending regular expression for a multi-line-comment


Example config file

```toml
[header]
version = "0.2.1"

[[comment]]
extension = "*.md"
start = '<!--\s*START_DOCS\s*-->'
each_line = '<!--(.*)-->' # some lines will, and some lines wont' have this
stop = '<!--\s*STOP_DOCS\s*-->'

[[template.all]]
tags = ["bindingField"]
file = ".vitepress/bindings.mjs"
output = """
export const bindingItems = [
    {{#items}}
    { text: '{{bindingField}}', link: '/bindings/{{bindingField}}' },
    {{/items}}
]
"""

[[template.all]]
tags = ["bindingField", "description"]
file = "bindings/index.md"
order = 5
output = """
{{#items}}
- [`{{bindingField}}`](/bindings/{{bindingField}}.md): {{description}}
{{/items}}
"""

[[template.all]]
tags = ["userCommand"]
file = ".vitepress/commands.mjs"
output = """
export const userCommandItems = [
    {{#items}}
    { text: '{{userCommand}}', link: '/commands/index#user-commands' },
    {{/items}}
]
"""

[[template.all]]
tags = ["command"]
file = ".vitepress/commands.mjs"
output = """
export const commandItems = [
    {{#items}}
    { text: '{{command}}', link: '/commands/{{command}}.md' },
    {{/items}}
]
"""

[[template.all]]
tags = ["userCommand", "name"]
file = "commands/index.md"
output = """
## User Commands

User commands take no arguments and generally interact with the user-interface of VSCode.

{{#items}}
- `Master Key: {{name}}` (`master-key.{{userCommand}}`) — {{{__body__}}}
{{/items}}
"""

[[template.all]]
tags = ["command"]
file = "commands/index.md"
output = """
## Keybinding Commands

Keybinding commands usually have at least one argument and are expected to primarily be
used when defining keybindings in a [master keybinding TOML file](/bindings).

{{#items}}
{{#section}}

### {{.}}

{{/section}}
- [`master-key.{{command}}`](/commands/{{command}}.md)
{{/items}}
"""

[[template.foreach]]
tags = ["command"]
file = "commands/{{command}}.md"
output = """
# `master-key.{{command}}`

{{{__body__}}}
"""

[[template.foreach]]
tags = ["bindingField", "description"]
file = "bindings/{{bindingField}}.md"
output = """

# Binding Field `{{bindingField}}`

{{{__body__}}}
"""

[[template.foreach]]
tags = ["forBindingField"]
file = "bindings/{{forBindingField}}.md"
output = "{{{__body__}}}"
```

## Roadmap

- support multi-line tag values
- support filtering via the config file / command line args
- implement a "watch" mode version of the service (or use npm extension to do this for us
  in master key)
