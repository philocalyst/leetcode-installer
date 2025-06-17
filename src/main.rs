use leetcode_core::GQLLeetcodeRequest;
use leetcode_core::types::language::Language;
use leetcode_core::{EditorDataRequest, QuestionRequest, init};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
