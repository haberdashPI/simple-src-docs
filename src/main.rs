use clap::Parser;
use either::{Either, Left, Right};
use lazy_static::lazy_static;
use mustache;
use mustache::MapBuilder;
use regex::Regex;
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize};
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
use walkdir::WalkDir;
use wax::{Glob, Pattern};

/// Extracts doc strings into markdown files
///
/// Walks through all files in `<SOURCE>` and searches for comments. With comments, Looks
/// for `@file [file]` on its own line and if present the contents of the comment are
/// appended to the specified file path. The file and its directories are created at the
/// given `<DEST>`. Optionally, you can provide `@order [num]` on its own line to influence
/// the ordering of the comment content. Content is sorted from the lowest to the highest
/// `order`, breaking ties by pre-sorted ordering. Additional `@` prefixed tags will be
/// excluded from the output. They don't do anything unless you define an appropriate
/// configuration template (See README.md for details). You can configure what
/// is considered a target for a given file extension in your config file.
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
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

    /// the source directories or files where comments will be extracted from
    source: Vec<PathBuf>,
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

fn read_comments(
    args: &Args,
    config: &SrcDocConfig,
    file: &Path,
    docs: &mut Vec<DocData>,
) -> Result<(), SrcDocError> {
    let io = File::open(file)?;
    let reader = io::BufReader::new(io);
    let str_lines = reader.lines().map_while(Result::ok);
    if args.verbose {
        println!("Reading file {}", file.to_str().unwrap());
    }
    let comment_config = config.find_comment_config(file);
    if let Some(c) = comment_config {
        let comments = Comments::new(str_lines, c);
        for d in DocIterator::new(comments) {
            docs.push(d);
        }
        return Ok(());
    } else {
        if args.verbose {
            println!("Skipping file without a matching extension");
        }
        return Ok(());
    }
}

fn run() -> Result<(), SrcDocError> {
    let args = Args::parse();
    let destination = &args.dest;
    if !destination.exists() {
        return Err(SrcDocError::new(format!(
            "The destination path `{}` does not exist.",
            destination.display()
        )));
    }

    let config = match &args.config {
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

    let mut all_docs: Vec<DocData> = Vec::new();
    for s in &args.source {
        for entry in WalkDir::new(s) {
            let file_entry = entry?;
            let file = file_entry.path();
            if !file.is_file() {
                continue;
            }
            read_comments(&args, &config, file, &mut all_docs)?;
        }
    }
    all_docs.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(Ordering::Less));
    let mut docmap = config.apply(&all_docs.iter().map(|x| x).collect())?;

    if args.verbose {
        println!("Writing doc files:");
    }
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

// Language Configuration //////////////////////////////////////////////////////////////////

fn str_to_glob<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Glob<'static>, D::Error> {
    let s: String = Deserialize::deserialize(deserializer)?;
    return match Glob::new(&format!("(?i){}", s)) {
        Ok(g) => Ok(g.into_owned()),
        Err(e) => Err(serde::de::Error::custom(e)),
    }
}

fn glob_to_str<S: serde::Serializer>(s: &Glob, serializer: S) -> Result<S::Ok, S::Error> {
    return serializer.serialize_str(s.to_string().as_str());
}

#[derive(Serialize, Deserialize, Clone)]
struct CommentConfig {
    #[serde(default = "zero")]
    order: f64,
    #[serde(deserialize_with = "str_to_glob", serialize_with = "glob_to_str")]
    extension: Glob<'static>,
    #[serde(with = "serde_regex")]
    start: Option<Regex>,
    #[serde(with = "serde_regex")]
    each_line: Option<Regex>,
    #[serde(with = "serde_regex")]
    stop: Option<Regex>,
}

lazy_static! {
    static ref DEFAULT_COMMENT_MAP: Vec<CommentConfig> = {
        let mut m = Vec::new();
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.{c,cpp,java,h,hpp,c++,h++,cxx,hxx,groovy,v,js,cs,ts,jsx,tsx,go,zig,kt,kts,d,swift,php,css,scala,dart,m}").unwrap(),
            start: Some(Regex::new(r"^\s*/\*\*\s*$").unwrap()),
            each_line: Some(Regex::new(r"^\s*\*\s?(.*)").unwrap()),
            stop: Some(Regex::new(r"^\s*\*/+\s*").unwrap()),
        });
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.{rb,r,sh,pl,pm,jl,awk,nim,crystal,tcl}").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*#\s?x(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 1.0,
            extension: Glob::new("(?i)*.{asm,s,clj,el,lisp,scm,ss,rkt}").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*;\s?(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 1.0,
            extension: Glob::new("(?i)*.{vb,vba}").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*'\s?(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 1.0,
            extension: Glob::new("(?i)*.{f,for,f90,f95,fortran}").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*!\s?(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.{lua,hs,elm,sql}").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*--\s?(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.{py,pyi}").unwrap(),
            start: Some(Regex::new(r#"^\s*"""\s*$"#).unwrap()),
            each_line: None,
            stop: Some(Regex::new(r#"^\s*"""\s*$"#).unwrap()),
        });
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.rs").unwrap(),
            start: None,
            each_line: Some(Regex::new(r"^\s*///\s?(.*)$").unwrap()),
            stop: None,
        });
        m.push(CommentConfig {
            order: 0.0,
            extension: Glob::new("(?i)*.jl").unwrap(),
            start: Some(Regex::new(r"^\s*#=\s*$").unwrap()),
            each_line: None,
            stop: Some(Regex::new(r"^\s*=#\s*$").unwrap()),
        });

        m
    };
}

// Templates ///////////////////////////////////////////////////////////////////////////////

fn start_stop_match(comment: &Vec<CommentConfig>) -> Result<(), ValidationError> {
    for c in comment {
        if c.start.is_none() ^ c.stop.is_none() {
            return Err(ValidationError::new(
                "start and stop must both be present, or they must both be absent.",
            ));
        }
    }
    return Ok(());
}

#[derive(Deserialize, Validate)]
struct SrcDocConfig {
    header: ConfigHeader,
    #[serde(default)]
    template: Option<ConfigTemplates>,
    #[serde(default)]
    #[validate(custom(function = "start_stop_match"))]
    comment: Option<Vec<CommentConfig>>,
}

#[derive(Deserialize)]
struct ConfigTemplates {
    #[serde(default)]
    foreach: Option<Vec<DocEachTemplate>>,
    #[serde(default)]
    all: Option<Vec<DocAllTemplate>>,
}

impl SrcDocConfig {
    fn new() -> SrcDocConfig {
        return SrcDocConfig {
            header: ConfigHeader {
                version: Version::parse("0.2.1").unwrap(),
            },
            template: None,
            comment: Some(DEFAULT_COMMENT_MAP.clone()),
        };
    }

    fn from<T: AsRef<Path>>(path: T) -> Result<SrcDocConfig, SrcDocError> {
        let str = fs::read_to_string(&path)?;
        let mut result = toml::from_str::<SrcDocConfig>(&str)?;
        let comment = if let Some(mut comment_map) = result.comment {
            for c in DEFAULT_COMMENT_MAP.iter() {
                comment_map.push(c.clone());
            }
            Some(comment_map)
        } else {
            Some(DEFAULT_COMMENT_MAP.clone())
        };
        result.comment = comment;
        return Ok(result);
    }

    fn find_comment_config(&self, file: &Path) -> Option<&CommentConfig> {
        return self.comment.as_ref()?.iter().find_map(|c| {
            if c.extension.is_match(file) || c.extension.is_match(file.file_name()?) {
                return Some(c);
            }

            return None;
        });
    }
}

fn valid_version(v: &Version) -> Result<(), ValidationError> {
    // we're on version 0.2.1: any files semver compatible with 0.2 are fine
    if VersionReq::parse("0.2").unwrap().matches(v) {
        return Ok(());
    } else {
        return Err(ValidationError::new(
            "File version incompatible with semver 0.2",
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
struct DocEachTemplate {
    tags: Vec<String>,
    file: String,
    #[serde(with = "either::serde_untagged", default = "left_zero")]
    order: Either<f64, String>,
    output: String,
}

#[derive(Deserialize)]
struct DocAllTemplate {
    file: String,
    tags: Vec<String>,
    #[serde(default = "zero")]
    order: f64,
    output: String,
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
        };
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

fn parse_order(order_str: &str) -> f64 {
    return match order_str.trim().parse() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error while evaluating @order {order_str}: {e}");
            0.0
        }
    };
}

impl DocEachTemplate {
    fn apply<'a>(
        &self,
        docs: &Vec<&'a DocData>,
        result: &mut HashMap<String, Vec<(f64, String)>>,
    ) -> Result<(), TemplateError> {
        for doc in docs {
            if !self.tags.iter().all(|tag| doc.tags.contains_key(tag)) {
                continue;
            }

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
                    parse_order(&mustache::compile_str(&str)?.render_data_to_string(&data)?)
                }
            };
            let body: String = mustache::compile_str(&self.output)?.render_data_to_string(&data)?;
            let items = result.entry(file).or_insert(Vec::new());
            items.push((order, body));
        }
        return Ok(());
    }
}

impl DocAllTemplate {
    fn apply<'a>(
        &self,
        docs: &Vec<&'a DocData>,
        result: &mut HashMap<String, Vec<(f64, String)>>,
    ) -> Result<(), TemplateError> {
        let mut builder = MapBuilder::new();
        builder = builder.insert_vec("items", |mut builder| {
            for s in docs {
                if !self.tags.iter().all(|tag| s.tags.contains_key(tag)) {
                    continue;
                }
                builder = builder.push_map(|mut map_builder| {
                    for (k, v) in &s.tags {
                        map_builder = map_builder.insert_str(k, v);
                    }
                    map_builder = map_builder.insert_str("__body__", &s.body);
                    return map_builder;
                });
            }
            return builder;
        });

        let data = builder.build();
        let body: String = mustache::compile_str(&self.output)?.render_data_to_string(&data)?;
        let items = result.entry(self.file.clone()).or_default();
        items.push((self.order, body));
        return Ok(());
    }
}

impl SrcDocConfig {
    fn apply<'a>(
        &self,
        data: &Vec<&'a DocData>,
    ) -> Result<HashMap<String, Vec<(f64, String)>>, TemplateError> {
        let mut results = HashMap::new();
        if let Some(templates) = &self.template {
            if let Some(each_templates) = &templates.foreach {
                for each_template in each_templates {
                    each_template.apply(data, &mut results)?;
                }
            }

            if let Some(all_templates) = &templates.all {
                for all_template in all_templates {
                    all_template.apply(data, &mut results)?;
                }
            }

            for doc in data {
                if let Some(file) = doc.tags.get("file") {
                    let order = doc.order;
                    let items = results.entry(file.clone()).or_default();
                    items.push((order, doc.body.clone()));
                }
            }
        }

        return Ok(results);
    }
}

// Comments ////////////////////////////////////////////////////////////////////////////////

struct Comments<'a, T: Iterator<Item = String>> {
    lines: T,
    in_comment: bool,
    config: &'a CommentConfig,
}

impl<'a, T: Iterator<Item = String>> Comments<'a, T> {
    fn new(lines: T, config: &'a CommentConfig) -> Comments<'a, T> {
        return Comments {
            lines,
            in_comment: false,
            config,
        };
    }
}

#[derive(Debug)]
struct CommentResult {
    value: String,
    last: bool,
}

impl<'a, T: Iterator<Item = String>> Iterator for Comments<'a, T> {
    type Item = CommentResult;
    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.lines.next() {
            None if self.in_comment => return Some(CommentResult {
                value: String::new(),
                last: true,
            }),
            None => return None,
            Some(x) => x,
        };

        if self.config.start.is_none() {
            // single line comment syntax
            let maybe_cap = self.config.each_line.as_ref().unwrap().captures(value.as_str());
            if let Some(capture) = maybe_cap {
                self.in_comment = true;
                if let Some(cap_match) = capture.get(1) {
                    return Some(CommentResult {
                        value: String::from(cap_match.as_str()),
                        last: false,
                    });
                }
            } else if self.in_comment {
                self.in_comment = false;
                return Some(CommentResult {
                    value: String::new(),
                    last: true,
                });
            }
        } else {
            // multiline comment syntax
            // validated invariant: if `start` is set, then `stop` is set
            let start_p = self.config.start.as_ref().unwrap();
            let end_p = self.config.stop.as_ref().unwrap();
            if !self.in_comment && start_p.is_match(&value) {
                self.in_comment = true;
                return self.next();
            } else if self.in_comment && end_p.is_match(&value) {
                let result = Some(CommentResult {
                    value: String::new(),
                    last: true,
                });
                self.in_comment = false;
                return result;
            }
            if self.in_comment {
                let each_line_r = match self.config.each_line.as_ref() {
                    Some(x) => x,
                    None => &Regex::new(r"\s*(.*)").unwrap(),
                };
                let maybe_cap = each_line_r.captures(&value);
                if let Some(capture) = maybe_cap {
                    if let Some(cap_match) = capture.get(1) {
                        return Some(CommentResult {
                            value: String::from(cap_match.as_str()),
                            last: false,
                        });
                    }
                }
                return Some(CommentResult { value, last: false });
            }
        }
        return self.next();
    }
}

// Parsed Docs /////////////////////////////////////////////////////////////////////////////

struct DocIterator<'a, T: Iterator<Item = String>> {
    comments: Comments<'a, T>,
}

#[derive(Debug)]
struct DocData {
    tags: HashMap<String, String>,
    order: f64,
    body: String,
}

impl<'a, T: Iterator<Item = String>> DocIterator<'a, T> {
    fn new(comments: Comments<'a, T>) -> DocIterator<'a, T> {
        return DocIterator { comments };
    }
}

impl<'a, T: Iterator<Item = String>> Iterator for DocIterator<'a, T> {
    type Item = DocData;
    fn next(&mut self) -> Option<DocData> {
        let tag_r: Regex = Regex::new(r".*@(?<tag>\S+)\s+(?<value>.*)").unwrap();
        let mut body = String::new();
        let mut tags = HashMap::new();
        let mut available_data = false;
        let mut order = 0.0;

        for comment in &mut self.comments {
            if comment.last {
                break;
            }

            if let Some(m) = tag_r.captures(&comment.value) {
                if &m["tag"] == "__body__" {
                    eprintln!("The tag `__body__` is reserved.");
                    std::process::exit(1);
                } else if &m["tag"] == "order" {
                    order = parse_order(&m["value"]);
                }
                tags.insert(String::from(&m["tag"]), String::from(m["value"].trim()));
            } else {
                available_data = true;
                body.push_str(&comment.value);
                body.push('\n');
            }
        }

        if available_data {
            return Some(DocData { tags, order, body });
        } else {
            return None;
        }
    }
}
