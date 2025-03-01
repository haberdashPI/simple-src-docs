use either::{Either, Left, Right};
use regex::Regex;
use core::fmt;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::collections::HashMap;
use clap::Parser;
use walkdir::WalkDir;

/// Extracts doc strings into markdown files
///
/// Takes all passed files in lexicographic order and searches for comments that start with
/// `--start-comment` regex, and ending with `--end-comemnt` regex. Looks for `@file [file]`
/// on the line following `--start-comment` and if present the contents of the comment are
/// appended to the specified file path. The file and its directories are created at the
/// given `--dest`. Optionally, after the line with `@file` you can provide `@order [num]`
/// to influence the ordering of the comment content. Content is sorted from the lowest to
/// the highest `order`, breaking ties by pre-sorted ordering.
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
    // regex for the starting comment delimiter
    #[arg(short, long, default_value = r"^\s*/\*\*\s*$")]
    start_comment: String,
    // regex for the ending comment delimiter
    #[arg(short, long, default_value = r"^\s*\*/\s*$")]
    end_comment: String,
    // the prefix to be removed from each line between the start and end comment delimiter
    #[arg(short, long, default_value = r"^\s*\*+\s*(.*)$")]
    comment_prefix: String,

    // root directory where markdown files are generated
    #[arg(short, long, default_value = ".")]
    dest: PathBuf,

    // the source directory where comment will be extracted from
    source: PathBuf,
}

fn to_regex(x: &str) -> Regex {
    return match Regex::new(x) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Invalid regex {}", e);
            std::process::exit(1);
        }
    }
}

fn main() {
    let file_field = Regex::new(r"@file\s(.*)").unwrap();
    let order_field = Regex::new(r"@order\s(.*)").unwrap();
    let args = Args::parse();
    let destination = args.dest;
    if !destination.exists() {
        eprintln!("The destination path `{}` does not exist.", destination.to_str().unwrap());
        std::process::exit(1);
    }
    let start_comment = to_regex(&args.start_comment);
    let end_comment = to_regex(&args.end_comment);
    let comment_prefix = to_regex(&args.comment_prefix);

    let mut docmap = HashMap::new();
    for entry in WalkDir::new(args.source) {
        let file_entry = match entry {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error walking paths: {}", e);
                continue;
            }
        };
        let file = file_entry.path();
        if !file.is_file() { continue; }

        let io = match File::open(file) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1)
            }
        };
        let reader = io::BufReader::new(io);
        let str_lines = reader.lines().map(|x| match x {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1)
            }
        });
        let comments = Comments::new(str_lines, start_comment.clone(), end_comment.clone(), comment_prefix.clone());
        let docs = DocIterator::new(comments, file_field.clone(), order_field.clone());
        for doc in docs {
            let items = docmap.entry(doc.file).or_insert(Vec::new());
            items.push((doc.order, doc.body));
        }
    }

    println!("Writing doc files:");
    for (file, items) in &docmap {
        println!(" - {}", file);
        let path = destination.join(file);
        let dir = path.parent().unwrap();

        match fs::create_dir_all(dir) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Could not create directory {}", e);
                std::process::exit(1);
            }
        }

        let mut io = match File::create(&path) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Could not create path {}", e);
                std::process::exit(1);
            }
        };

        let mut sorted = items.clone();
        sorted.sort_by_key(|x| x.0);
        for (_, body) in sorted {
            match writeln!(io, "{}", body) {
                Err(e) => {
                    eprintln!("Failed to write {}", e);
                    std::process::exit(1);
                }
                Ok(_) => ()
            }
        }
    }
}

// Comments ////////////////////////////////////////////////////////////////////////////////

struct Comments<T: Iterator<Item = String>> {
    lines: T,
    in_comment: bool,
    comment_prefix: Regex,
    start_comment: Regex,
    end_comment: Regex,
    line_count: i64,
}

impl<T: Iterator<Item = String>> Comments<T> {
    fn new(
        lines: T,
        start_comment: Regex,
        end_comment: Regex,
        comment_prefix: Regex,
    ) -> Comments<T> {
        return Comments {
            lines,
            in_comment: false,
            start_comment,
            end_comment,
            comment_prefix,
            line_count: 0,
        };
    }
}
struct CommentResult {
    value: String,
    line: i64,
    last: bool,
}

impl<T: Iterator<Item = String>> Iterator for Comments<T> {
    type Item = CommentResult;
    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.lines.next() {
            Some(x) => x,
            None => return None,
        };

        if !self.in_comment && self.start_comment.is_match(value.as_str()) {
            self.in_comment = true;
            self.line_count = 0;
            return self.next();
        } else if self.in_comment && self.end_comment.is_match(value.as_str()) {
            let result = Some(CommentResult {
                value: String::new(),
                line: self.line_count,
                last: true
            });
            self.in_comment = false;
            self.line_count = 0;
            return result;
        }
        if self.in_comment {
            self.line_count += 1;
            let maybe_cap = self.comment_prefix.captures(value.as_str());
            if let Some(capture) = maybe_cap {
                if let Some(cap_match) = capture.get(1) {
                    return Some(CommentResult {
                        value: String::from(cap_match.as_str()),
                        line: self.line_count,
                        last: false,
                    });
                }
            }
            return Some(CommentResult {
                value,
                line: self.line_count,
                last: false,
            });
        } else {
            return self.next();
        }
    }
}

// Docs ////////////////////////////////////////////////////////////////////////////////////

struct DocIterator<T: Iterator<Item = String>> {
    comments: Comments<T>,
    file_field: Regex,
    order_field: Regex,
}

impl fmt::Display for DocResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "{}[{}]:\n{}\n", self.file, self.order, self.body);
    }
}

struct DocResult {
    file: String,
    order: i64,
    body: String,
}

fn get_capture(x: &String, re: &Regex) -> Option<String> {
    return match re.captures(x) {
        Some(cap) => match cap.get(1) {
            Some(cmatch) => Some(String::from(cmatch.as_str())),
            None => None,
        },
        None => None,
    };
}

impl<T: Iterator<Item = String>> DocIterator<T> {
    fn new(comments: Comments<T>, file_field: Regex, order_field: Regex) -> DocIterator<T> {
        return DocIterator {
            comments,
            file_field,
            order_field,
        };
    }
    fn read_file_field(&mut self) -> Option<String> {
        // read @file (maybe turn into a function)
        let mut comment = match self.comments.next() {
            Some(c) => c,
            None => return None,
        };
        let mut capture = get_capture(&comment.value, &self.file_field);
        while comment.line != 1 || capture.is_none() {
            comment = match self.comments.next() {
                Some(c) => c,
                None => return None,
            };
            capture = get_capture(&comment.value, &self.file_field);
        }
        return capture;
    }

    fn read_order_field(&mut self) -> Either<i64, String> {
        let comment = match self.comments.next() {
            Some(x) => x,
            None => return Left(0),
        };
        match get_capture(&comment.value, &self.order_field) {
            Some(num_str) => match num_str.parse() {
                Ok(order) => return Left(order),
                Err(_) => {
                    println!("Non-numeric `@order` value `{}`, ignoring.", num_str);
                    return Left(0);
                }
            },
            None => return Right(comment.value),
        };
    }
}

impl<T: Iterator<Item = String>> Iterator for DocIterator<T> {
    type Item = DocResult;
    fn next(&mut self) -> Option<DocResult> {
        let mut body = String::new();

        let file = match self.read_file_field() {
            Some(f) => f,
            None => return None,
        };

        let order_or_first_line = self.read_order_field();
        let order = match order_or_first_line {
            Left(x) => x,
            Right(line) => {
                body.push_str(&line);
                body.push('\n');
                0
            }
        };

        for comment in &mut self.comments {
            if comment.last { break }
            body.push_str(&comment.value);
            body.push('\n');
        }

        return Some(DocResult { file, order, body });
    }
}
