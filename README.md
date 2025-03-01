# `simple-src-docs`

simple-src-docs is an extremely bare bones tool to facilitate generating documentation from your source files.

```
❯ simple-src-docs --help
Extracts doc strings into markdown files

Walks through all files in `[SOURCE]` and searches for comments that start with `--start-
comment` regex, and ending with `--end-comemnt` regex. Looks for `@file [file]` on the line
following `--start-comment` and if present the contents of the comment are appended to the
specified file path. The file and its directories are created at the given `--dest`.
Optionally, after the line with `@file` you can provide `@order [num]` to influence the
ordering of the comment content. Content is sorted from the lowest to the highest `order`,
breaking ties by pre-sorted ordering.

Usage: simple-src-docs [OPTIONS] <SOURCE>

Arguments:
  <SOURCE>


Options:
  -s, --start-comment <START_COMMENT>
          [default: ^\s*/\*\*\s*$]

  -e, --end-comment <END_COMMENT>
          [default: ^\s*\*/\s*$]

  -c, --comment-prefix <COMMENT_PREFIX>
          [default: ^\s*\*+\s*(.*)$]

  -d, --dest <DEST>
          [default: .]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Why?

This could easily have been written in just about any language or as a set of calls to a few command line utilities. It's a simple tool with few features. I'm planning a bigger project in rust, and since I haven't written much rust yet, I wanted to start with a simple project to get some practical experience before diving into something a little more ambitious. The tool also happens to be useful for my current project. I don't have any plans to expand the scope of this utility.

## Roadmap

1. make it work
2. write up some *very* basic unit tests
3. Write up some CI for test
4. Write up some CI for generating releases
