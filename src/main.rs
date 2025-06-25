use core::panic;
use futures::StreamExt;
use futures::prelude;
use html2md;
use leetcode_core::GQLLeetcodeRequest;
use leetcode_core::errors::LcAppError;
use leetcode_core::types::editor_data::QuestionEditorData;
use leetcode_core::types::language::Language;
use leetcode_core::types::problemset_question_list::{Question, TopicTag};
use leetcode_core::{EditorDataRequest, QuestionRequest};
use rusqlite;
use rusqlite::Connection;
use sea_query::*;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::{env, fs};

#[derive(Iden)]
enum Entries {
    Table,
    Id,
    Name,
    PremiumStatus,
    Description,
    AcRate,
    Difficulty,
    CreatedAt,
}

#[derive(Iden)]
enum Tags {
    Table,
    Id,
    Name,
    Slug,
}

#[derive(Iden)]
enum EntryTags {
    Table,
    EntryId,
    TagId,
}

#[derive(Iden)]
enum Languages {
    Table,
    Id,
    Name,
}

#[derive(Iden)]
enum EntryLanguages {
    Table,
    EntryId,
    LanguageId,
}

fn create_entries_table() -> TableCreateStatement {
    Table::create()
        .table(Entries::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Entries::Id)
                .integer()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(Entries::Name).string_len(255).not_null())
        .col(ColumnDef::new(Entries::AcRate).float().not_null())
        .col(
            ColumnDef::new(Entries::Difficulty)
                .string_len(255)
                .not_null(),
        )
        .col(
            ColumnDef::new(Entries::PremiumStatus)
                .boolean()
                .default(false),
        )
        .col(ColumnDef::new(Entries::Description).text())
        .col(
            ColumnDef::new(Entries::CreatedAt)
                .timestamp()
                .default(Expr::current_timestamp()),
        )
        .to_owned()
}

fn create_tags_table() -> TableCreateStatement {
    Table::create()
        .table(Tags::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Tags::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        .col(
            ColumnDef::new(Tags::Name)
                .string_len(100)
                .unique_key()
                .not_null(),
        )
        .col(
            ColumnDef::new(Tags::Slug)
                .string_len(100)
                .unique_key()
                .not_null(),
        )
        .to_owned()
}

fn create_entry_tags_table() -> TableCreateStatement {
    Table::create()
        .table(EntryTags::Table)
        .if_not_exists()
        .col(ColumnDef::new(EntryTags::EntryId).integer().not_null())
        .col(ColumnDef::new(EntryTags::TagId).integer().not_null())
        .primary_key(
            Index::create()
                .col(EntryTags::EntryId)
                .col(EntryTags::TagId),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_entry_tags_entry_id")
                .from(EntryTags::Table, EntryTags::EntryId)
                .to(Entries::Table, Entries::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_entry_tags_tag_id")
                .from(EntryTags::Table, EntryTags::TagId)
                .to(Tags::Table, Tags::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn create_languages_table() -> TableCreateStatement {
    Table::create()
        .table(Languages::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Languages::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        .col(
            ColumnDef::new(Languages::Name)
                .string_len(50)
                .unique_key()
                .not_null(),
        )
        .to_owned()
}

fn create_entry_languages_table() -> TableCreateStatement {
    Table::create()
        .table(EntryLanguages::Table)
        .if_not_exists()
        .col(ColumnDef::new(EntryLanguages::EntryId).integer().not_null())
        .col(
            ColumnDef::new(EntryLanguages::LanguageId)
                .integer()
                .not_null(),
        )
        .primary_key(
            Index::create()
                .col(EntryLanguages::EntryId)
                .col(EntryLanguages::LanguageId),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_entry_languages_entry_id")
                .from(EntryLanguages::Table, EntryLanguages::EntryId)
                .to(Entries::Table, Entries::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_entry_languages_language_id")
                .from(EntryLanguages::Table, EntryLanguages::LanguageId)
                .to(Languages::Table, Languages::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn build_db(db: &Connection) -> Result<&Connection, Box<dyn Error>> {
    // Main table for all entries
    let entries_table = create_entries_table();

    // Languages table and the meta for many-to-many
    let tags_table = create_tags_table();
    let entry_tags_table = create_entry_tags_table();

    // Languages table and the meta for many-to-many
    let languages_table = create_languages_table();
    let entry_languages_table = create_entry_languages_table();

    // Execute our sqlite statements and build the schema
    db.execute(&entries_table.to_string(SqliteQueryBuilder), ())?;
    db.execute(&entry_tags_table.to_string(SqliteQueryBuilder), ())?;
    db.execute(&tags_table.to_string(SqliteQueryBuilder), ())?;
    db.execute(&languages_table.to_string(SqliteQueryBuilder), ())?;
    db.execute(&entry_languages_table.to_string(SqliteQueryBuilder), ())?;
    Ok(db)
}

fn build_language_list(lang_data: &QuestionEditorData) -> Vec<Language> {
    let languages: Vec<Language> = lang_data
        .question
        .code_snippets
        .iter()
        .map(|item| item.lang_slug.clone())
        .collect();

    languages
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Obtain a connection with the store
    let connection: Connection = Connection::open_with_flags(
        "./db.sqlite3",
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
    )?;

    // Build out the storage solution
    build_db(&connection)?;

    let connection = Arc::new(Mutex::new(connection));

    // Read your LeetCode cookies from env vars
    let csrf = env::var("LEETCODE_CSRF_TOKEN")?;
    let session = env::var("LEETCODE_SESSION")?;

    // Initialize the HTTP client
    leetcode_core::init(&csrf, &session).await?;

    let mut skip = 0;
    let limit = 200;

    loop {
        // Fetch a page of questions
        let page = QuestionRequest::new(limit, skip).send().await?;
        let questions = page.get_questions();
        if questions.is_empty() {
            break;
        }

        let question_stream = futures::stream::iter(questions.into_iter().map(|question| {
            let connection = connection.clone(); // Clone the Arc, not the Connection
            async move {
                match process_question(question, connection).await {
                    Ok(_) => println!("Successfully processed question"),
                    Err(e) => eprintln!("Error processing question: {}", e),
                }
            }
        }));

        question_stream
            .buffer_unordered(3)
            .collect::<Vec<_>>()
            .await;

        skip += limit;
    }

    // No errors, we're good.
    Ok(())
}

async fn process_question(
    question: Question,
    connection: Arc<Mutex<Connection>>,
) -> Result<(), Box<dyn Error>> {
    // Fetch the editor data for this problem
    let slug = question.title_slug.clone();

    // Parse the ID into a u32 (why is it a string chat)
    let question_id: u32 = question.frontend_question_id.parse()?;

    // Handle premium vs free questions with optional values
    let (description, editor_data) = if question.paid_only {
        // Can't access the description of a paid question (right now)
        (String::new(), None)
    } else {
        let data = EditorDataRequest::new(slug.clone()).send().await?;
        // Get the HTML description
        let content = data.data.question.content.clone();
        // Make it beautiful markdown
        let mut description = html2md::parse_html(&content);
        description = build_markdown(description)?;
        (description, Some(data))
    };

    // Process editor data if available (non-premium questions)
    if let Some(data) = &editor_data {
        // Get the questions supported languages
        let languages = build_language_list(&data.data);

        {
            let conn = connection.lock().unwrap();
            establish_languages(&conn, &languages, question_id)?;
        }

        let lang = Language::C;
        // If there is a language snippet, write it out
        if let Some(code) = data.get_editor_data_by_language(&lang) {
            // Get the filename
            let filename = format!("{}_{}.{}", question_id, slug, lang.get_extension());

            // Create the code source directory
            fs::create_dir_all("code")?;
            let path = format!("code/{}", filename);
            fs::write(&path, code)?;
            println!("Saved {}", path);
        }
    }

    // Build and execute the database query with unified values
    let query = Query::insert()
        .into_table(Entries::Table)
        .columns([
            Entries::Id,
            Entries::Name,
            Entries::PremiumStatus,
            Entries::Description,
            Entries::AcRate,
            Entries::Difficulty,
        ])
        .on_conflict(
            OnConflict::column(Entries::Id)
                .update_column(Entries::Id)
                .to_owned(),
        )
        .values([
            question.frontend_question_id.into(),
            question.title.into(),
            question.paid_only.into(),
            description.into(),
            question.ac_rate.into(),
            question.difficulty.into(),
        ])?
        .to_owned();

    // Get topics tags, if none are found, just initalize as an empty vector. For easy interface with the DB.
    let tags = &question.topic_tags.unwrap_or(vec![]);

    {
        let conn = connection.lock().unwrap();
        establish_tags(&conn, &tags, question_id)?;
        conn.execute(&query.to_string(SqliteQueryBuilder), ())?;
    }

    Ok(())
}

fn build_markdown(description: String) -> Result<String, Box<dyn Error>> {
    let mut closing_code_block_lines: Vec<usize> = Vec::new();

    let mut count: usize = 0;
    // Add in the language tags MANUALLY :(
    let mut pair = false;
    let lines = description
        .split('\n')
        .map(|line| {
            count += 1;
            if line.starts_with("```") && !pair {
                pair = true;
                format!("{}{}", line, "python") // Python looks really good with the psuedocode
            } else if line.starts_with("```") {
                pair = false;
                closing_code_block_lines.push(count);
                line.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>();

    let lines_to_remove: std::collections::HashSet<usize> = closing_code_block_lines
        .iter()
        .filter_map(|&line_num| {
            if line_num > 1 {
                Some(line_num - 1) // Line numbers are 1-indexed, so subtract 1 to get the previous line
            } else {
                None
            }
        })
        .collect();

    let description = lines
        .into_iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_number = idx + 1; // Convert to 1-indexed
            if lines_to_remove.contains(&line_number) {
                None // Skip this line
            } else {
                Some(line)
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    Ok(description)
}

fn establish_tags(
    conn: &Connection,
    tags: &Vec<TopicTag>,
    question_id: u32,
) -> Result<(), Box<dyn Error>> {
    // Insert tags and create entry-tag relationships
    Ok(for tag in tags {
        // Insert tag if it doesn't exist
        let tag_insert = Query::insert()
            .into_table(Tags::Table)
            .columns([Tags::Name, Tags::Slug])
            .values([tag.name.clone().into(), tag.slug.clone().into()])?
            .on_conflict(OnConflict::column(Tags::Slug).do_nothing().to_owned())
            .to_owned();

        conn.execute(&tag_insert.to_string(SqliteQueryBuilder), ())?;

        // Get the tag_id by querying for it after insertion
        let tag_id_query = Query::select()
            .column(Tags::Id)
            .from(Tags::Table)
            .and_where(Expr::col(Tags::Slug).eq(&tag.slug))
            .to_owned();

        let mut stmt = conn.prepare(&tag_id_query.to_string(SqliteQueryBuilder))?;
        let tag_id: u32 = stmt.query_row([], |row| Ok(row.get::<_, u32>(0)?))?;

        // Create entry-tag relationship
        let entry_tag_insert = Query::insert()
            .into_table(EntryTags::Table)
            .columns([EntryTags::EntryId, EntryTags::TagId])
            .values([question_id.into(), tag_id.into()])?
            .on_conflict(
                OnConflict::columns([EntryTags::EntryId, EntryTags::TagId])
                    .update_columns([EntryTags::EntryId, EntryTags::TagId])
                    .to_owned(),
            )
            .to_owned();

        conn.execute(&entry_tag_insert.to_string(SqliteQueryBuilder), ())?;
    })
}

fn establish_languages(
    conn: &Connection,
    languages: &Vec<Language>,
    question_id: u32,
) -> Result<(), Box<dyn Error>> {
    // Insert languages and create entry-language relationships
    Ok(for language in languages {
        let lang_name = language.to_string();

        // Insert language if it doesn't exist
        let lang_insert = Query::insert()
            .into_table(Languages::Table)
            .columns([Languages::Name])
            .values([lang_name.clone().into()])?
            .on_conflict(OnConflict::column(Languages::Name).do_nothing().to_owned())
            .to_owned();

        conn.execute(&lang_insert.to_string(SqliteQueryBuilder), ())?;

        let lang_id: u32 = language.to_id();

        // Create entry-language relationship
        let entry_lang_insert = Query::insert()
            .into_table(EntryLanguages::Table)
            .columns([EntryLanguages::EntryId, EntryLanguages::LanguageId])
            .values([question_id.into(), lang_id.into()])?
            .on_conflict(
                OnConflict::columns([EntryLanguages::EntryId, EntryLanguages::LanguageId])
                    .update_columns([EntryLanguages::EntryId, EntryLanguages::LanguageId])
                    .to_owned(),
            )
            .to_owned();

        conn.execute(&entry_lang_insert.to_string(SqliteQueryBuilder), ())?;
    })
}
