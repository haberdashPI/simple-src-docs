use clap::Parser;
use either::{Either, Left, Right};
use mustache;
use mustache::MapBuilder;
use regex::Regex;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, Write};
use std::num::ParseFloatError;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use validator::{Validate, ValidationError};
use semver::{Version,VersionReq};
use walkdir::WalkDir;

/// Extracts doc strings into markdown files
///
/// Walks through all files in `<SOURCE>` and searches for comments that start with
/// `<START_COMMENT>` regex, and ending with `<END_COMEMNT>` regex. Looks for `@file [file]`
/// on the line following `<START_COMMENT>` and if present the contents of the comment are
/// appended to the specified file path. The file and its directories are created at the
/// given `<DEST>`. Optionally, after the line with `@file` you can provide `@order [num]` to
/// influence the ordering of the comment content. Content is sorted from the lowest to the
/// highest `order`, breaking ties by pre-sorted ordering. Additional `@` prefixed
/// tags will be ignored in the output, unless you define an appropriate configuration
/// template (See README.md for details)
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
    /// regex for the starting comment delimiter
    #[arg(short, long, default_value = r"^\s*/\*\*\s*$")]
    start_comment: String,
    /// regex for the ending comment delimiter
    #[arg(short, long, default_value = r"^\s*\*/\s*$")]
    end_comment: String,
    /// the prefix to be removed from each comment line between the start and end comment
    /// delimiter; the first capture group should denote the prefix, and the second the text
    /// to read
    #[arg(short, long, default_value = r"^\s*\*+\s*(.*)$")]
    comment_prefix: String,

    /// root directory where markdown files are generated
    #[arg(short, long, default_value = ".")]
    dest: PathBuf,

    /// if set, show detailed messages about document processing
    #[arg(short, long)]
    verbose: bool,

    /// location of file used to further configure simple-src-docs
    /// (see README.md), defaults to `<DEST>/.simple-src-docs.config.toml
    #[arg(long)]
    config: Option<PathBuf>,

    /// the source directory where comments will be extracted from
    source: PathBuf,
}

fn to_regex(x: &str) -> Regex {
    return match Regex::new(x) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Invalid regex {}", e);
            std::process::exit(1);
        }
    };
}

struct SrcDocError {
    msg: String,
    code: std::process::ExitCode,
}

impl SrcDocError {
    fn new(msg: String) -> SrcDocError {
        return SrcDocError {
            msg,
            code: ExitCode::FAILURE,
        };
    }
}

fn exit_code(x: Result<(), SrcDocError>) -> ExitCode {
    match x {
        Ok(_) => {
            println!("Successfully generated documentation.");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{}", e.msg);
            return e.code;
        }
    };
}

fn main() -> ExitCode {
    return exit_code(run());
}

impl From<io::Error> for SrcDocError {
    fn from(e: io::Error) -> SrcDocError {
        return SrcDocError::new(format!("IO Error: {}", e));
    }
}

impl From<toml::de::Error> for SrcDocError {
    fn from(e: toml::de::Error) -> SrcDocError {
        return SrcDocError::new(format!("Config Error: {}", e));
    }
}

impl From<walkdir::Error> for SrcDocError {
    fn from(e: walkdir::Error) -> SrcDocError {
        return SrcDocError::new(format!("Error traversing directories: {}", e));
    }
}

fn run() -> Result<(), SrcDocError> {
    let file_field = Regex::new(r"@file\s(.*)").unwrap();
    let order_field = Regex::new(r"@order\s(.*)").unwrap();
    let args = Args::parse();
    let destination = args.dest;
    if !destination.exists() {
        return Err(SrcDocError::new(format!(
            "The destination path `{}` does not exist.",
            destination.display()
        )));
    }
    let start_comment = to_regex(&args.start_comment);
    let end_comment = to_regex(&args.end_comment);
    let comment_prefix = to_regex(&args.comment_prefix);

    let config = match args.config {
        Some(x) => SrcDocConfig::from(x)?,
        None => {
            let default_config = destination.join(".simple-src-docs.config.toml");
            if default_config.is_file() {
                SrcDocConfig::from(default_config)?
            } else {
                SrcDocConfig::new()
            }
        }
    };

    let mut docmap = HashMap::new();
    for entry in WalkDir::new(args.source) {
        let file_entry = entry?;
        let file = file_entry.path();
        if !file.is_file() { continue; }

        let io = File::open(file)?;
        let reader = io::BufReader::new(io);
        let str_lines = reader.lines().map_while(Result::ok);
        if args.verbose { println!("Reading file {}", file.to_str().unwrap()); }
        let comments  = Comments::new(
            str_lines,
            start_comment.clone(),
            end_comment.clone(),
            comment_prefix.clone(),
        );
        let docs = DocIterator::new(comments, file_field.clone(), order_field.clone());
        for doc in docs {
            config.apply(doc, &mut docmap)?;
        }
    }

    for doc in config.doc.unwrap_or_default() {
        let items = docmap.entry(doc.file).or_insert(Vec::new());
        items.push((doc.order, doc.body));
    }

    if args.verbose { println!("Writing doc files:"); }
    for (file, items) in docmap.iter_mut() {
        if args.verbose {
            println!(" - {}", file);
        }
        let path = destination.join(file);
        let dir = path.parent().unwrap();

        fs::create_dir_all(dir)?;
        let mut io = File::create(&path)?;

        items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Less));
        for (_, body) in items {
            write!(io, "{}", body)?;
        }
    }
    return Ok(());
}

// Templates ///////////////////////////////////////////////////////////////////////////////

#[derive(Deserialize)]
struct SrcDocConfig {
    header: ConfigHeader,
    #[serde(default)]
    template: Option<Vec<DocTemplate>>,
    #[serde(default)]
    doc: Option<Vec<DocResult>>,
}

impl SrcDocConfig {
    fn new() -> SrcDocConfig {
        return SrcDocConfig {
            header: ConfigHeader {
                version: Version::parse("0.1").unwrap(),
            },
            template: None,
            doc: None,
        };
    }

    fn from<T: AsRef<Path>>(path: T) -> Result<SrcDocConfig, SrcDocError> {
        let str = fs::read_to_string(&path)?;
        let mut result = toml::from_str::<SrcDocConfig>(&str)?;
        result.template.iter_mut().for_each(|x|
            x.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(Ordering::Less)));
        return Ok(result);
    }
}

fn valid_version(v: &Version) -> Result<(), ValidationError> {
    if  VersionReq::parse("0.1").unwrap().matches(v) {
        return Ok(());
    } else {
        return Err(ValidationError::new(
            "File version incompatible with semver 0.1",
        ));
    }
}

#[derive(Deserialize, Validate)]
struct ConfigHeader {
    #[validate(custom(function = "valid_version"))]
    version: Version,
}

fn zero() -> f64 {
    return 0.0;
}

fn left_zero() -> Either<f64, String> {
    return Left(0.0);
}

#[derive(Deserialize)]
struct DocTemplate {
    tags: Vec<String>,
    #[serde(default = "zero")]
    order: f64,
    output: Vec<DocTemplateElement>,
}

#[derive(Deserialize)]
struct DocTemplateElement {
    file: String,
    #[serde(with = "either::serde_untagged", default = "left_zero")]
    order: Either<f64, String>,
    body: String,
}

enum TemplateError {
    Mustache(mustache::Error),
    Parse(ParseFloatError),
}

impl From<TemplateError> for SrcDocError {
    fn from(value: TemplateError) -> Self {
        return match value {
            TemplateError::Parse(e) => SrcDocError::new(format!("Error parsing @order {}", e)),
            TemplateError::Mustache(e) => SrcDocError::new(format!("Template parsing error {}", e)),
        }
    }
}

impl From<mustache::Error> for TemplateError {
    fn from(value: mustache::Error) -> Self {
        return TemplateError::Mustache(value);
    }
}

impl From<ParseFloatError> for TemplateError {
    fn from(value: ParseFloatError) -> Self {
        return TemplateError::Parse(value);
    }
}

impl DocTemplateElement {
    fn apply(&self, doc: &DocData) -> Result<DocResult, TemplateError> {
        let mut builder = MapBuilder::new();
        for (key, val) in &doc.tags {
            builder = builder.insert_str(key, val);
        }
        builder = builder.insert_str("__body__", &doc.body);
        let data = builder.build();

        let file: String = mustache::compile_str(&self.file)?.render_data_to_string(&data)?;
        let order: f64 = match &self.order {
            Left(n) => *n,
            Right(str) => {
                let n_str: String = mustache::compile_str(&str)?.render_data_to_string(&data)?;
                match n_str.parse() {
                    Ok(x) => x,
                    Err(e) => {
                        eprintln!("Error while evaluating @order {}: {}", &str, e);
                        println!("{:?}", data);
                        0.0
                    }
                }
            }
        };
        let body: String = mustache::compile_str(&self.body)?.render_data_to_string(&data)?;
        return Ok(DocResult { file, order, body });
    }
}

impl DocTemplate {
    fn apply(&self, doc: &DocData) -> Result<Vec<DocResult>, TemplateError> {
        if self.tags.iter().all(|tag| doc.tags.contains_key(tag)) {
            return self.output.iter().map(|t| t.apply(&doc)).collect();
        } else {
            return Ok(Vec::new());
        }
    }
}

impl SrcDocConfig {
    fn apply(
        &self,
        data: DocData,
        results: &mut HashMap<String, Vec<(f64, String)>>,
    ) -> Result<(), TemplateError> {

        if let Some(x) = &self.template {
            for template in x {
                let applied = template.apply(&data)?;
                if !applied.is_empty() {
                    for r in applied {
                        let items = results.entry(r.file).or_insert(Vec::new());
                        items.push((r.order, r.body));
                    }
                    return Ok(());
                }
            }
        }
        return match DocResult::new(data) {
            None => Ok(()),
            Some(r) => {
                let items = results.entry(r.file).or_insert(Vec::new());
                items.push((r.order, r.body));
                Ok(())
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

#[derive(Debug)]
struct CommentResult {
    value: String,
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
                last: true,
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
                        last: false,
                    });
                }
            }
            return Some(CommentResult { value, last: false });
        } else {
            return self.next();
        }
    }
}

// Parsed Docs /////////////////////////////////////////////////////////////////////////////

struct DocIterator<T: Iterator<Item = String>> {
    comments: Comments<T>,
    file_field: Regex,
    order_field: Regex,
}

#[derive(Debug)]
struct DocData {
    tags: HashMap<String, String>,
    body: String,
}

impl<T: Iterator<Item = String>> DocIterator<T> {
    fn new(comments: Comments<T>, file_field: Regex, order_field: Regex) -> DocIterator<T> {
        return DocIterator {
            comments,
            file_field,
            order_field,
        };
    }
}

impl<T: Iterator<Item = String>> Iterator for DocIterator<T> {
    type Item = DocData;
    fn next(&mut self) -> Option<DocData> {
        let tagr: Regex = Regex::new(r".*@(?<tag>\S+)\s+(?<value>.*)").unwrap();
        let mut body = String::new();
        let mut tags = HashMap::new();
        let mut available_data = false;

        for comment in &mut self.comments {
            if comment.last {
                break;
            }

            if let Some(m) = tagr.captures(&comment.value) {
                if &m["tag"] == "__body__" {
                    eprintln!("The tag `__body__` is reserved.");
                    std::process::exit(1);
                }
                tags.insert(String::from(&m["tag"]), String::from(&m["value"]));
            } else {
                available_data = true;
                body.push_str(&comment.value);
                body.push('\n');
            }
        }

        if available_data {
            if !tags.contains_key("order") {
                tags.insert("order".to_string(), "0.0".to_string());
            }
            return Some(DocData { tags, body });
        } else {
            return None;
        }
    }
}

// Processed Docs //////////////////////////////////////////////////////////////////////////

#[derive(Debug, Deserialize, Clone)]
struct DocResult {
    file: String,
    #[serde(default = "zero")]
    order: f64,
    body: String,
}

impl DocResult {
    fn new(x: DocData) -> Option<DocResult> {
        let file = match x.tags.get("file") {
            None => return None,
            Some(file) => String::from(file),
        };
        let order = match x.tags.get("order") {
            None => 0.0,
            Some(o) => match o.parse() {
                Err(_) => 0.0,
                Ok(o_value) => o_value,
            },
        };
        return Some(DocResult {
            file,
            order,
            body: x.body,
        });
    }
}
