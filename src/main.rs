use either::{Either, Left, Right};
use regex::Regex;
use core::fmt;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

// use std::collections::HashMap;
fn main() {
    let file_field = Regex::new(r"@file\s(.*)").unwrap();
    let order_field = Regex::new(r"@order\s(.*)").unwrap();

    // TODO: make these command line arguments
    let start_comment = Regex::new(r"^\s*/\*\*\s*$").unwrap();
    let end_comment = Regex::new(r"^\s*\*/\s*$").unwrap();
    let comment_prefix = Regex::new(r"^\s*\*+\s*(.*)$").unwrap();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Required filename argument missing.");
        std::process::exit(1);
    }
    let filename = &args[1];
    println!("File name is: {}.", filename);
    let file = match File::open(filename) {
        Ok(f) => f,
        Err(e) => {
            println!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };
    let reader = io::BufReader::new(file);
    let str_lines = reader.lines().map(|x| match x {
        Ok(r) => r,
        Err(e) => {
            println!("Error reading file: {}", e);
            std::process::exit(1)
        }
    });
    let comments = Comments::new(str_lines, start_comment, end_comment, comment_prefix);
    let docs = DocIterator::new(comments, file_field, order_field);
    for doc in docs {
        println!("{}", doc);
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
            self.in_comment = false;
            self.line_count = 0;
            return self.next();
        }
        if self.in_comment {
            self.line_count += 1;
            let maybe_cap = self.comment_prefix.captures(value.as_str());
            if let Some(capture) = maybe_cap {
                if let Some(cap_match) = capture.get(1) {
                    return Some(CommentResult {
                        value: String::from(cap_match.as_str()),
                        line: self.line_count,
                    });
                }
            }
            return Some(CommentResult {
                value,
                line: self.line_count,
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
                    println!("Non-numeric `@order` value ({}), ignoring.", comment.value,);
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

        while let Some(comment) = self.comments.next() {
            body.push_str(&comment.value);
            body.push('\n');
        }

        return Some(DocResult { file, order, body });
    }
}
