# `simple-src-docs`

simple-src-docs is an extremely bare bones tool to facilitate generating documentation from your source files.

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

          [default: ^\s*\*+\s*(.*)$]

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

The config file is a TOML file with the following sections:

- `header`: must contain a `version` string that is [semver](https://semver.org/) compatible with 0.1
- `doc`: Generates additional docs. Has three fields:
  - `file`: the file to render docs to
  - `order`: (optional), as per the `@order` field in comments, this
    determines what order the body will be inserted into the file
  - `body`: The text to insert into `file`
- `template`: Transforms docs using the given set of tags into the `output` docs.
  Can use additional arbitrary `@` prefixed tags listed in a doc
  - `tags`: an array of strings; for the template to be applied these tags must
    be present
  - `output`: an array of objects with the following fields. All values
    can be a [mustache template](https://mustache.github.io/), and
    the value of any of the tags in the input doc section will be
    inserted into this template.
        - `file`: the file to store output in
        - `order`: the ordering of the doc in the file
        - `body`: the content to write to the file


Example config file

```toml
[header]
version = 0.1

[[doc]]
file = ".vitpress/commands.mjs"
order = -1000
body = """
export default commandLines = [
"""

[[doc]]
file = ".vitepress/commands.mjs"
order = 10000
body = """
];
"""

[[template]]
tags = ["command"]

[[template.output]]
file = ".vitepress/commands.mjs"
order = "{{order}}"
body = """
{ text: '{{command}}', link: '/commands/{{command}}' },
"""

[[template.output]]
file = "commands/index.md"
order = "{{order}}"
body = """
 - [`master-key.{{command}}'](/commands/{{command}})
"""

[[template.output]]
file = "commands/{{command}}.md"
body = """
# `master-key.{{command}}`

{{__body__}}
"""
```
