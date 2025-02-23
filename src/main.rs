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
        comment_prefix
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
            println!("{}", cline);
        }
    }
}

struct CommentFilter {
    in_comment: bool,
    comment_prefix: Regex,
    start_comment: Regex,
    end_comment: Regex
}

impl CommentFilter {
    fn apply<'a>(&mut self, line: &'a str) -> Option<&'a str> {
        if !self.in_comment && self.start_comment.is_match(line) {
            self.in_comment = true;
            return None;
        } else if self.in_comment && self.end_comment.is_match(line) {
            self.in_comment = false;
            return Some("\n");
        }
        if self.in_comment {
            let maybe_cap = self.comment_prefix.captures(line);
            if let Some(capture) = maybe_cap {
                if let Some(cap_match) = capture.get(1) {
                    return Some(cap_match.as_str());
                }
            }
            return Some(line);
        } else {
            return None
        }
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
