use std::fs::File;
use std::io::{self, BufRead};
use regex::Regex;
use std::env;
// use std::collections::HashMap;

fn main() {
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
    let mut comment_filter = CommentFilter{
        in_comment: false,
        start_comment,
        end_comment,
        comment_prefix,
        line_count: 0
    };
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(r) => r,
            Err(e) => {
                println!("Error reading file: {}", e);
                std::process::exit(1)
            }
        };
        let filtered = comment_filter.apply(line.as_ref());
        if let Some(cline) = filtered {
            println!("{}", cline.line);
        }
    }
}

struct CommentFilter {
    in_comment: bool,
    comment_prefix: Regex,
    start_comment: Regex,
    end_comment: Regex,
    line_count: i64,
}

struct CommentResult<'a> {
    line: &'a str,
    line_num: i64,
}

impl CommentFilter {
    fn apply<'a>(&mut self, line: &'a str) -> Option<CommentResult<'a>> {
        if !self.in_comment && self.start_comment.is_match(line) {
            self.in_comment = true;
            self.line_count = 0;
            return None;
        } else if self.in_comment && self.end_comment.is_match(line) {
            self.in_comment = false;
            let result = Some(CommentResult{
                line: "\n",
                line_num: self.line_count+1
            });
            self.line_count = 0;
            return result;
        }
        if self.in_comment {
            self.line_count += 1;
            let maybe_cap = self.comment_prefix.captures(line);
            if let Some(capture) = maybe_cap {
                if let Some(cap_match) = capture.get(1) {
                    return Some(CommentResult{
                        line: cap_match.as_str(),
                        line_num: self.line_count,
                    });
                }
            }
            return Some(CommentResult{
                line: line,
                line_num: self.line_count,
            });
        } else {
            return None
        }
    }
}

struct DocFilter {
    file_field: Regex,
    order_field: Regex,
    pending_result: Option<DocResult>,
}

struct DocResult<'a> {
    file: &'a str,
    order: i64,
    body: String,
    line_count: i64,
}

impl DocFilter {
    fn apply<'a>(&mut self, comment: &'a CommentResult) -> Option<DocResult<'a>> {
        let old_result = self.pending_result;
        if comment.line_num == 1 {
            if let Some(cap) = self.file_field.captures(comment.line) {
                if let Some(cmatch) = cap.get(1) {
                    self.pending_result = Some(DocResult {
                        file: cmatch.as_str(),
                        order: 0,
                        body: String::new(),
                        line_count: comment.line_num,
                    });
                }
            }
            self.pending_result = None;
        }
        if comment.line_num == 2 {
            if let Some(cap) = self.order_field.captures(comment.line) {
                if let Some(cmatch) = cap.get(1) {
                    let order = match cmatch.as_str().parse::<i64>() {
                        Ok(order) => order,
                        Err(_) => {
                            println!("Non-numeric order ({}), ignoring.", comment.line);
                            0
                        }
                    }
                    let old_result = self.pending_result.unwrap();
                    self.pending_result = Some(DocResult {
                        file: old_result.file,
                        order: old_result.order,
                        body: old_result.body,
                        line_count: comment.line_num,
                    });
                }
            }
            self.pending_result = None;
        }

        // TODO: rather than returning an Option filters should
        // return a `Filtered` value that can be Some, None or Final
        if let Some(x) = old_result {
            if let Some(y) = self.pending_result {
                return self.pending_result;
            }
        }
        return None
    }
}

// struct DocResults {

// }

// let file_field = r".*@file\s+(?<file>[^@]+)"
// let order_field = r".*@order\s+(?<order>[^@]+)"

// fn process_file(file: File, doc: HashMap<String, DocResults>, start_comment: Regex, end_comment: Regex) {
//     let mut in_comment = false;
//     let mut in_docs = false;
//     let mut new_docs = false;
//     let lines = io::BufReader::new(file).lines();
//     let mut Option<&str> cur_file = None;
//     let mut number cur_order = 0;
//     while let Some(line) = lines.next() {
//         if (!in_comment && start_comment.is_match(line)) {
//             in_comment = true;
//             if let Some(line) = lines.next() {
//                 if let Some(captured) = file_field.captures(line) {
//                     cur_file = captured["file"];
//                     in_docs = true;
//                     new_docs = true;
//                 }
//             }
//         } else {
//             in_comment = false;
//             in_docs = false;
//             new_docs = false;
//         }
//         if (new_docs) {
//             if let Some(line) = lines.next() {
//                 if let Some(captured) = file_field.captures(line) {
//                     let cur_order_str = captured["order"]
//                     cur_order = cur_order_str.parse()?;
//                 } else {
//                     update_docs(doc, cur_file)
//                 }
//             }
//         }
//         new_docs = false;
//     }

//     return doc
// }
