#![allow(non_snake_case)]

use std::{fs, io};
use std::fs::File;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

use anyhow::Context;
use camino::Utf8Path;
use chrono::NaiveDateTime;
use indoc::formatdoc;
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{debug, error};
use crate::update_bm;
use crate::adapter::dal::Dal;
use crate::environment::CONFIG;
use crate::util::helper::abspath;
use crate::model::bookmark::{Bookmark, BookmarkUpdater};
use crate::util::helper;

#[derive(Debug, PartialEq, Clone)]
pub enum DisplayField {
    Id,
    URL,
    Metadata,
    // title
    Tags,
    Desc,
    Flags,
    LastUpdateTs,
    Embedding,
    Similarity,
}

#[allow(dead_code)]
pub const MINIMUM_FIELDS: [DisplayField; 3] =
    [DisplayField::Id, DisplayField::URL, DisplayField::Metadata];
#[allow(dead_code)]
pub const DEFAULT_FIELDS: [DisplayField; 6] = [
    DisplayField::Id,
    DisplayField::URL,
    DisplayField::Metadata,
    DisplayField::Desc,
    DisplayField::Tags,
    DisplayField::Similarity,
];
#[allow(dead_code)]
pub const ALL_FIELDS: [DisplayField; 9] = [
    DisplayField::Id,
    DisplayField::URL,
    DisplayField::Metadata,
    DisplayField::Desc,
    DisplayField::Tags,
    DisplayField::Flags, // counter
    DisplayField::LastUpdateTs,
    DisplayField::Embedding,
    DisplayField::Similarity,
];

#[derive(Debug, PartialEq, Clone)]
pub struct DisplayBookmark {
    pub id: i32,
    pub URL: String,
    pub metadata: String,
    pub desc: String,
    pub tags: String,
    pub flags: i32,
    pub last_update_ts: NaiveDateTime,
    pub embedding: String,
    pub content_hash: String,
    pub similarity: Option<f32>,
}

// method for creating DisplayBookmark from Bookmark
impl From<&Bookmark> for DisplayBookmark {
    fn from(bm: &Bookmark) -> Self {
        DisplayBookmark {
            id: bm.id,
            URL: bm.URL.clone(),
            metadata: bm.metadata.clone(),
            desc: bm.desc.clone(),
            tags: bm.tags.clone(),
            flags: bm.flags,
            last_update_ts: bm.last_update_ts,
            embedding: format!("{:?}", bm.embedding),
            content_hash: format!("{:?}", bm.content_hash),
            similarity: None,
        }
    }
}

pub fn show_bms(bms: &Vec<DisplayBookmark>, fields: &[DisplayField]) {
    // let mut stdout = StandardStream::stdout(ColorChoice::Always);
    // Check if the output is a TTY
    let color_choice = if io::stdout().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    let mut stderr = StandardStream::stderr(color_choice);
    let first_col_width = bms.len().to_string().len();

    for (i, bm) in bms.iter().enumerate() {
        if fields.contains(&DisplayField::Metadata) {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                .unwrap();
            write!(&mut stderr, "{:first_col_width$}. {}", i + 1, bm.metadata).unwrap();
        }

        if fields.contains(&DisplayField::Similarity) {
            if let Some(similarity) = bm.similarity {
                stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))).unwrap();
                write!(&mut stderr, " [{:.3}]", similarity).unwrap();
            }
        }

        if fields.contains(&DisplayField::Id) {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::White)))
                .unwrap();
            writeln!(&mut stderr, " [{}]", bm.id).unwrap();
        }

        if fields.contains(&DisplayField::URL) {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))
                .unwrap();
            writeln!(&mut stderr, "{:first_col_width$}  {}", "", bm.URL).unwrap();
        }

        if fields.contains(&DisplayField::Desc) && !bm.desc.is_empty() {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::White)))
                .unwrap();
            writeln!(&mut stderr, "{:first_col_width$}  {}", "", bm.desc).unwrap();
        }

        if fields.contains(&DisplayField::Tags) {
            let tags = bm.tags.replace(',', " ");
            if tags.find(|c: char| !c.is_whitespace()).is_some() {
                stderr
                    .set_color(ColorSpec::new().set_fg(Some(Color::Blue)))
                    .unwrap();
                writeln!(&mut stderr, "{:first_col_width$}  {}", "", tags.trim()).unwrap();
            }
        }

        let mut flags_and_embedding_line = String::new();

        if fields.contains(&DisplayField::Flags) {
            flags_and_embedding_line.push_str(&format!("Count: {}", bm.flags));
        }

        if fields.contains(&DisplayField::Embedding) {
            let embed_status = if bm.embedding.is_empty() {
                "null"
            } else {
                "yes"
            };
            if !flags_and_embedding_line.is_empty() {
                flags_and_embedding_line.push_str(" | ");
            }
            flags_and_embedding_line.push_str(&format!("embed: {}", embed_status));
        }

        if !flags_and_embedding_line.is_empty() {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::White)))
                .unwrap();
            writeln!(
                &mut stderr,
                "{:first_col_width$}  {}",
                "", flags_and_embedding_line
            )
                .unwrap();
        }

        // if fields.contains(&DisplayField::Flags) {
        //     stderr
        //         .set_color(ColorSpec::new().set_fg(Some(Color::White)))
        //         .unwrap();
        //     writeln!(&mut stderr, "{:first_col_width$}  Count: {}", "", bm.flags).unwrap();
        // }

        if fields.contains(&DisplayField::LastUpdateTs) {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))
                .unwrap();
            writeln!(
                &mut stderr,
                "{:first_col_width$}  {}",
                "", bm.last_update_ts
            )
                .unwrap();
        }

        stderr.reset().unwrap();
        eprintln!();
    }
}

fn parse(input: &str) -> Vec<String> {
    let binding = input.trim().replace(',', "").to_lowercase();
    let tokens = binding
        .split(' ')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    debug!("{:?}", tokens);
    tokens
}

pub fn process(bms: &Vec<Bookmark>) {
    // debug!("{:?}", bms);
    let help_text = r#"
        <n1> <n2>:      opens selection in browser
        p <n1> <n2>:    print id-list of selection
        p:              print all ids
        d <n1> <n2>:    delete selection
        e:              edit selection
        t:              touch selection
        q | ENTER:      quit
        h:              help
    "#;

    loop {
        eprint!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let mut tokens = parse(&input);
        if tokens.is_empty() {
            break;
        }

        let regex = Regex::new(r"^\d+").unwrap(); // Create a new Regex object
        match tokens[0].as_str() {
            "p" => {
                if let Some(ids) = helper::ensure_int_vector(&tokens.split_off(1)) {
                    print_ids(ids, bms.clone()).unwrap_or_else(|e| {
                        error!("{}", e);
                    });
                    break;
                } else {
                    error!(
                        "Invalid input, only numbers allowed",
                    );
                }
            }
            "d" => {
                if let Some(ids) = helper::ensure_int_vector(&tokens.split_off(1)) {
                    delete_bms(ids, bms.clone()).unwrap_or_else(|e| {
                        error!("{}", e);
                    });
                    break;
                } else {
                    error!(
                        "Invalid input, only numbers allowed",
                    );
                }
            }
            "e" => {
                if let Some(ids) = helper::ensure_int_vector(&tokens.split_off(1)) {
                    edit_bms(ids, bms.clone()).unwrap_or_else(|e| {
                        error!("{}", e);
                    });
                    break;
                } else {
                    error!(
                        "Invalid input, only numbers allowed",
                    );
                }
            }
            "t" => {
                if let Some(ids) = helper::ensure_int_vector(&tokens.split_off(1)) {
                    touch_bms(ids, bms.clone()).unwrap_or_else(|e| {
                        error!("{}", e);
                    });
                    break;
                } else {
                    error!(
                        "Invalid input, only numbers allowed",
                    );
                }
            }
            "h" => println!("{}", help_text),
            "q" => break,
            // Use Regex object in a guard
            s if regex.is_match(s) => {
                if let Some(ids) = helper::ensure_int_vector(&tokens) {
                    open_bms(ids, bms.clone()).unwrap_or_else(|e| {
                        error!("{}", e);
                    });
                    break;
                } else {
                    error!(
                        "Invalid input, only numbers allowed",
                    );
                }
            }
            _ => {
                println!("Invalid Input");
                println!("{}", help_text);
            }
        }
    }
}

pub fn touch_bms(ids: Vec<i32>, bms: Vec<Bookmark>) -> anyhow::Result<()> {
    debug!("ids: {:?}", ids);
    do_sth_with_bms(ids, bms, do_touch).with_context(|| {
        "Error touching bookmarks".to_string()
    })?;
    Ok(())
}

pub fn edit_bms(ids: Vec<i32>, bms: Vec<Bookmark>) -> anyhow::Result<()> {
    debug!("ids: {:?}", ids);
    do_sth_with_bms(ids, bms, do_edit)
        .with_context(|| "Error opening bookmarks".to_string())?;
    Ok(())
}

pub fn open_bm(bm: &Bookmark) -> anyhow::Result<()> {
    do_touch(bm)?;
    _open_bm(&bm.URL)?;
    Ok(())
}

fn _open_bm(uri: &str) -> anyhow::Result<()> {
    if uri.starts_with("shell::") {
        let cmd = uri.replace("shell::", "");
        debug!("Shell Command {:?}", cmd);
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Error opening {}", uri))?;

        let status = child.wait().expect("Failed to wait on Vim");
        debug!("Exit status: {:?}", status);
        Ok(())
    } else {
        debug!("General OS open {:?}", uri);
        match abspath(uri) {
            Some(p) => {
                if Utf8Path::new(&p).extension() == Some("md") {
                    debug!("Opening markdown file with editor {:?}", p);
                    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                    debug!("Using editor: {:?}",editor);
                    Command::new(&editor)
                        .arg(&p)
                        .status()
                        .with_context(|| {
                            format!(
                                "Error opening {} with [{}], check your EDITOR variable.",
                                p,
                                &editor
                            )
                        })?;
                } else {
                    debug!("Opening file with default OS application {:?}", p);
                    open::that(&p).with_context(|| format!("Error OS opening {}", p))?;
                }
            }
            None => {
                debug!("Opening URI with default OS command {:?}", uri);
                open::that(uri).with_context(|| format!("Error OS opening {}", uri))?;
            }
        }
        Ok(())
    }
}

pub fn open_bms(ids: Vec<i32>, bms: Vec<Bookmark>) -> anyhow::Result<()> {
    // debug!("ids: {:?}, bms: {:?}", ids, bms);
    do_sth_with_bms(ids.clone(), bms.clone(), open_bm)
        .with_context(|| "Error opening bookmarks".to_string())?;
    Ok(())
}

pub fn delete_bms(mut ids: Vec<i32>, bms: Vec<Bookmark>) -> anyhow::Result<()> {
    // reverse sort necessary due to DB compaction (deletion of last entry first)
    ids.reverse();
    debug!("ids: {:?}, bms: {:?}", ids, bms);
    // debug!("{:?}", &ids);
    fn delete_bm(bm: &Bookmark) -> anyhow::Result<()> {
        let _ = Dal::new(CONFIG.db_url.clone()).delete_bookmark2(bm.id)?;
        eprintln!("Deleted: {}", bm.URL);
        Ok(())
    }
    do_sth_with_bms(ids, bms, delete_bm).with_context(|| {
        "Error deleting bookmarks".to_string()
    })?;
    Ok(())
}

fn do_sth_with_bms(
    ids: Vec<i32>,
    bms: Vec<Bookmark>,
    do_sth: fn(bm: &Bookmark) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // debug!("ids: {:?}, bms: {:?}", ids, bms);
    for id in ids {
        if id as usize > bms.len() {
            eprintln!("Id {} out of range", id);
            continue;
        }
        let bm = &bms[id as usize - 1];
        debug!("id: {:?}, bm: {:?}", id, bm);
        do_sth(bm).with_context(|| format!("bm {:?}", bm))?;
    }
    Ok(())
}

/// update the last_update_ts field of a bookmark
/// increases flag (counter) by 1 and prints it
pub fn do_touch(bm: &Bookmark) -> anyhow::Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    update_bm(bm.id, &vec![], &vec![], &mut dal, false)?;
    let bm = dal.get_bookmark_by_id(bm.id)?;

    show_bms(&vec![DisplayBookmark::from(&bm)], &ALL_FIELDS);
    Ok(())
}

pub fn do_edit(bm: &Bookmark) -> anyhow::Result<()> {
    // Create a file inside of `std::env::temp_dir()`.
    // let mut file = tempfile()?;
    let mut temp_file = File::create("temp.txt")?;

    let template = formatdoc! {r###"
        # Lines beginning with "#" will be stripped.
        # Add URL in next line (single line).
        {url}
        # Add TITLE in next line (single line). Leave blank to web fetch, "-" for no title.
        {title}
        # Add comma-separated TAGS in next line (single line).
        {tags}
        # Add COMMENTS in next line(s). Leave blank to web fetch, "-" for no comments.
        {comments}
        "###,
        url=bm.URL.clone(),
        title=bm.metadata.clone(),
        tags=bm.tags.clone(),
        comments=bm.desc.clone(),
    };

    temp_file.write_all(template.as_bytes()).with_context(|| {
        "Error writing to temp file".to_string()
    })?;

    // get default OS editor in variable to use in Command::new
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    debug!(
        "Using editor: {:?}",
        editor
    );
    // Open the temporary file with Vim (comment out for rstest)
    Command::new(&editor)
        .arg("temp.txt")
        .status()
        .with_context(|| {
            format!(
                "Error opening temp file with [{}], check your EDITOR variable.",
                &editor
            )
        })?;

    // Read the modified content of the file back into a string
    let modified_content = fs::read_to_string("temp.txt")
        .with_context(|| "Error reading temp file".to_string())?;

    let mut lines = modified_content.lines().filter(|l| !l.starts_with('#'));

    let url = lines.next().unwrap_or_default().to_string();
    let title = lines.next().unwrap_or_default().to_string();
    let tags = lines.next().unwrap_or_default().to_string();

    // Process multiline Description
    let desc = lines.clone().collect::<Vec<_>>().join("\n");

    let mut new_bm = Bookmark {
        id: bm.id,
        URL: url,
        metadata: title, // title
        tags,
        desc, // comments
        flags: bm.flags,
        last_update_ts: Default::default(), // will be overwritten by diesel
        embedding: None,
        content_hash: None,
    };
    debug!("lines: {:?}", lines);
    new_bm.update();

    let updated = Dal::new(CONFIG.db_url.clone())
        .update_bookmark(new_bm)
        .with_context(|| "Error updating bookmark".to_string())?;
    // Delete the temporary file
    fs::remove_file("temp.txt")?;

    let d_bms: Vec<DisplayBookmark> = updated.iter()
        .map(DisplayBookmark::from).collect();
    show_bms(&d_bms, &ALL_FIELDS);
    Ok(())
}

fn print_ids(ids: Vec<i32>, bms: Vec<Bookmark>) -> anyhow::Result<()> {
    debug!("ids: {:?}, bms: {:?}", ids, bms);
    let selected_bms = if ids.is_empty() {
        bms // print all
    } else {
        ids.iter()
            .filter_map(|id| bms.get(*id as usize - 1))
            .cloned()
            .collect::<Vec<Bookmark>>()
    };

    let mut bm_ids: Vec<i32> = selected_bms.iter().map(|bm| bm.id).collect();
    bm_ids.sort(); // Sort the list of bm.id values numerically
    let ids_str: Vec<String> = bm_ids.iter().map(|id| id.to_string()).collect();
    println!("{}", ids_str.join(","));
    Ok(())
}

#[cfg(test)]
mod test {
    use anyhow::anyhow;
    use rstest::*;

    use crate::adapter::dal::Dal;
    use crate::adapter::json::bms_to_json;
    use crate::adapter::dal::migration::init_db;

    use super::*;

    #[fixture]
    fn bms() -> Vec<Bookmark> {
        let mut dal = Dal::new(String::from("../db/bkmr.db"));
        init_db(&mut dal.conn).expect("Error DB init");
        let bms = dal.get_bookmarks("");
        bms.unwrap()
    }

    #[rstest]
    #[ignore = "Manual Test"]
    fn test_process(bms: Vec<Bookmark>) {
        process(&bms);
    }

    #[rstest]
    #[ignore = "Manual Test"]
    fn test_show_bms(bms: Vec<Bookmark>) {
        let d_bms: Vec<DisplayBookmark> = bms.iter()
            .map(DisplayBookmark::from).collect();
        // show individual fields
        show_bms(
            &d_bms,
            &[
                DisplayField::Id,
                DisplayField::URL,
                DisplayField::Metadata,
                // DisplayField::Desc,
                // DisplayField::Tags,
                // DisplayField::LastUpdateTs,
            ],
        );
        // show_bms(&bms, &DEFAULT_FIELDS);
        // show_bms(&bms, &ALL_FIELDS);
    }

    #[rstest]
    fn test_bms_to_json(bms: Vec<Bookmark>) {
        bms_to_json(&bms);
    }

    // Config is for Makefile tests. DO NOT RUN HERE
    #[rstest]
    #[ignore = "Manual Test with Makefile"]
    #[case("https://www.google.com")]
    #[ignore = "Manual Test with Makefile"]
    #[case("./tests/resources/bkmr.pptx")]
    #[ignore = "Manual Test with Makefile"]
    #[case(r#####"shell::vim +/"## SqlAlchemy" $HOME/dev/s/private/bkmr/bkmr/tests/resources/sample_docu.md"#####
    )]
    fn test_open_bm(#[case] bm: &str) {
        _open_bm(bm).unwrap();
    }

    #[rstest]
    #[ignore = "Manual Test"]
    #[case(vec ! [1])]
    fn test_open_bms(bms: Vec<Bookmark>, #[case] ids: Vec<i32>) {
        open_bms(ids, bms).unwrap();
    }

    #[rstest]
    // #[case(vec ! [String::from("1")])]
    #[case(vec ! [])]
    fn test_print_ids(bms: Vec<Bookmark>, #[case] tokens: Vec<i32>) {
        print_ids(tokens, bms).unwrap();
    } // todo assert missing

    #[rstest]
    #[case(vec ! [1])]
    fn test_do_sth_with_bms(#[case] tokens: Vec<i32>, bms: Vec<Bookmark>) {
        let result = do_sth_with_bms(tokens, bms, |bm| {
            println!("FN: {:?}", bm);
            Ok(())
        });
        assert!(result.is_ok());
    } // todo assert missing

    #[rstest]
    fn test_do_sth_with_bms_error(bms: Vec<Bookmark>) {
        let result = do_sth_with_bms(vec![1], bms, |bm| {
            println!("FN: {:?}", bm);
            Err(anyhow!("Anyhow Error"))
        });
        assert!(result.is_err());
    }
}
