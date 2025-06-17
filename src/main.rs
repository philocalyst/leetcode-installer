use leetcode_core::GQLLeetcodeRequest;
use leetcode_core::types::language::Language;
use leetcode_core::{EditorDataRequest, QuestionRequest, init};
use limbo;
use sea_query::*;
use std::error::Error;
use std::{env, fs};

// Define the table and column identifiers using enums
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

fn create_entries_table() -> TableCreateStatement {
    Table::create()
        .table(Entries::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Entries::Id)
                .integer()
                .not_null()
                .auto_increment()
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
        .col(ColumnDef::new(EntryTags::EntryId).integer())
        .col(ColumnDef::new(EntryTags::TagId).integer())
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let source = limbo::Builder::new_local("./db.sqlite3");

    let db = source.build().await?;

    let db = db.connect()?;

    let entries_table = create_entries_table();

    db.execute(&entries_table.to_string(SqliteQueryBuilder), ())
        .await?;

    // Read your LeetCode cookies from env vars
    let csrf = env::var("LEETCODE_CSRF_TOKEN")?;
    let session = env::var("LEETCODE_SESSION")?;

    // Initialize the HTTP client
    init(&csrf, &session).await?;

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
