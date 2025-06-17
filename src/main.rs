use leetcode_core::GQLLeetcodeRequest;
use leetcode_core::types::language::Language;
use leetcode_core::{EditorDataRequest, QuestionRequest};
use rusqlite;
use rusqlite::Connection;
use sea_query::*;
use std::error::Error;
use std::{env, fs};

#[derive(Iden)]
enum Entries {
    Table,
    Id,
    Name,
    PremiumStatus,
    Description,
    CreatedAt,
}

#[derive(Iden)]
enum Tags {
    Table,
    Id,
    Name,
}

#[derive(Iden)]
enum EntryTags {
    Table,
    EntryId,
    TagId,
}

#[derive(Iden)]
enum ProgrammingLanguages {
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
        .table(ProgrammingLanguages::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(ProgrammingLanguages::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        .col(
            ColumnDef::new(ProgrammingLanguages::Name)
                .string_len(50)
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
                .to(ProgrammingLanguages::Table, ProgrammingLanguages::Id)
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Obtain a connection with the store
    let connection: Connection = Connection::open_with_flags(
        "./db.sqlite3",
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
    )?;

    // Build out the storage solution
    build_db(&connection)?;

    let mut query = Query::insert()
        .into_table(Entries::Table)
        .columns([Entries::Name])
        .to_owned();

    query.values(["hi".into()])?;

    connection.execute(&query.to_string(SqliteQueryBuilder), ())?;

    // Read your LeetCode cookies from env vars
    let csrf = env::var("LEETCODE_CSRF_TOKEN")?;
    let session = env::var("LEETCODE_SESSION")?;

    // Initialize the HTTP client
    leetcode_core::init(&csrf, &session).await?;

    let mut skip = 0;
    let limit = 100;

    loop {
        // Fetch a page of questions
        let page = QuestionRequest::new(limit, skip).send().await?;
        let questions = page.get_questions();
        if questions.is_empty() {
            break;
        }

        for q in questions {
            let slug = q.title_slug.clone();
            // Fetch the editor data for this problem
            let qdata = EditorDataRequest::new(slug.clone()).send().await?;
            let lang = Language::Cpp;

            // If there is a C++ snippet, write it out
            if let Some(code) = qdata.get_editor_data_by_language(&lang) {
                let filename = qdata.get_filename(&lang)?;
                fs::create_dir_all("cpp")?;
                let path = format!("cpp/{}", filename);
                fs::write(&path, code)?;
                println!("Saved {}", path);
            }
        }

        skip += limit;
    }

    Ok(())
}
