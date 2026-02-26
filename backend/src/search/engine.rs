use std::ops::Bound;
use std::path::PathBuf;

use serde::Serialize;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, INDEXED, STORED, STRING, TEXT,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::schema::Value;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

/// A message ready to be indexed into the search engine.
pub struct IndexableMessage {
    pub uid: u32,
    pub folder: String,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub body_text: String,
    pub date_epoch: i64,
    pub has_attachments: bool,
}

/// A single search result returned from a query.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub uid: u32,
    pub folder: String,
    pub score: f32,
    pub snippet: String,
}

/// Parameters for a search query, with optional filters.
#[derive(Default)]
pub struct SearchQuery {
    pub text: String,
    pub folder: Option<String>,
    pub from: Option<String>,
    #[allow(dead_code)]
    pub to: Option<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub has_attachment: Option<bool>,
    pub limit: usize,
    pub offset: usize,
}

/// Top-level search engine that manages per-user indices.
pub struct SearchEngine {
    base_dir: PathBuf,
}

/// Named field handles for convenient access within the schema.
struct SchemaFields {
    uid: Field,
    folder: Field,
    subject: Field,
    from_address: Field,
    from_name: Field,
    to_addresses: Field,
    body_text: Field,
    date_epoch: Field,
    has_attachments: Field,
}

/// A per-user Tantivy index with reader and schema.
pub struct UserIndex {
    index: Index,
    reader: IndexReader,
    #[allow(dead_code)]
    schema: Schema,
    fields: SchemaFields,
}

/// Build the shared schema used by all user indices.
fn build_schema() -> (Schema, SchemaFields) {
    let mut builder = Schema::builder();

    let uid = builder.add_u64_field("uid", INDEXED | STORED);
    let folder = builder.add_text_field("folder", STRING | STORED);

    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_stored = TextOptions::default()
        .set_stored()
        .set_indexing_options(text_indexing);

    let subject = builder.add_text_field("subject", text_stored);
    let from_address = builder.add_text_field("from_address", STRING | STORED);
    let from_name = builder.add_text_field("from_name", TEXT | STORED);
    let to_addresses = builder.add_text_field("to_addresses", TEXT);
    let body_text = builder.add_text_field("body_text", TEXT);
    let date_epoch = builder.add_i64_field("date_epoch", INDEXED | STORED);
    let has_attachments = builder.add_u64_field("has_attachments", INDEXED);

    let schema = builder.build();
    let fields = SchemaFields {
        uid,
        folder,
        subject,
        from_address,
        from_name,
        to_addresses,
        body_text,
        date_epoch,
        has_attachments,
    };

    (schema, fields)
}

impl SearchEngine {
    /// Create a new search engine with the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Open or create the Tantivy index for a specific user.
    pub fn open_user_index(&self, user_hash: &str) -> Result<UserIndex, String> {
        let index_dir = self.base_dir.join(user_hash).join("tantivy");
        std::fs::create_dir_all(&index_dir)
            .map_err(|e| format!("Failed to create index directory: {e}"))?;

        let (schema, fields) = build_schema();

        let index = if Index::open_in_dir(&index_dir).is_ok() {
            Index::open_in_dir(&index_dir)
                .map_err(|e| format!("Failed to open existing index: {e}"))?
        } else {
            Index::create_in_dir(&index_dir, schema.clone())
                .map_err(|e| format!("Failed to create index: {e}"))?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| format!("Failed to create index reader: {e}"))?;

        Ok(UserIndex {
            index,
            reader,
            schema,
            fields,
        })
    }
}

impl UserIndex {
    /// Index a single message. Uses delete-before-insert for upsert semantics.
    pub fn index_message(&self, msg: &IndexableMessage) -> Result<(), String> {
        let mut writer: IndexWriter = self
            .index
            .writer(15_000_000)
            .map_err(|e| format!("Failed to create index writer: {e}"))?;

        self.delete_existing(&mut writer, msg.uid, &msg.folder);

        let doc = self.build_document(msg);
        writer
            .add_document(doc)
            .map_err(|e| format!("Failed to add document: {e}"))?;

        writer
            .commit()
            .map_err(|e| format!("Failed to commit: {e}"))?;

        self.reader
            .reload()
            .map_err(|e| format!("Failed to reload reader: {e}"))?;

        Ok(())
    }

    /// Index multiple messages in a single batch. Uses delete-before-insert for each.
    pub fn index_messages_batch(&self, messages: &[IndexableMessage]) -> Result<(), String> {
        let mut writer: IndexWriter = self
            .index
            .writer(15_000_000)
            .map_err(|e| format!("Failed to create index writer: {e}"))?;

        for msg in messages {
            self.delete_existing(&mut writer, msg.uid, &msg.folder);
            let doc = self.build_document(msg);
            writer
                .add_document(doc)
                .map_err(|e| format!("Failed to add document: {e}"))?;
        }

        writer
            .commit()
            .map_err(|e| format!("Failed to commit batch: {e}"))?;

        self.reader
            .reload()
            .map_err(|e| format!("Failed to reload reader: {e}"))?;

        Ok(())
    }

    /// Delete a message from the index by uid and folder.
    #[allow(dead_code)]
    pub fn delete_message(&self, uid: u32, folder: &str) -> Result<(), String> {
        let mut writer: IndexWriter = self
            .index
            .writer(15_000_000)
            .map_err(|e| format!("Failed to create index writer: {e}"))?;

        self.delete_existing(&mut writer, uid, folder);

        writer
            .commit()
            .map_err(|e| format!("Failed to commit delete: {e}"))?;

        self.reader
            .reload()
            .map_err(|e| format!("Failed to reload reader: {e}"))?;

        Ok(())
    }

    /// Search the index with a text query and optional filters.
    /// Returns matching results and total count.
    pub fn search(&self, query: &SearchQuery) -> Result<(Vec<SearchResult>, usize), String> {
        if query.text.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let searcher = self.reader.searcher();

        // Build the text query across subject, body_text, from_name, to_addresses.
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.fields.subject,
                self.fields.body_text,
                self.fields.from_name,
                self.fields.to_addresses,
            ],
        );

        let text_query = query_parser
            .parse_query(&query.text)
            .map_err(|e| format!("Failed to parse query: {e}"))?;

        // Collect all sub-queries into a BooleanQuery with Must clauses.
        let mut clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
        clauses.push((Occur::Must, text_query));

        // Optional folder filter (STRING field -> TermQuery).
        if let Some(ref folder) = query.folder {
            let term = Term::from_field_text(self.fields.folder, folder);
            let folder_query = TermQuery::new(term, IndexRecordOption::Basic);
            clauses.push((Occur::Must, Box::new(folder_query)));
        }

        // Optional from filter (STRING field -> TermQuery on from_address).
        if let Some(ref from) = query.from {
            let term = Term::from_field_text(self.fields.from_address, from);
            let from_query = TermQuery::new(term, IndexRecordOption::Basic);
            clauses.push((Occur::Must, Box::new(from_query)));
        }

        // Optional date range filter.
        if query.date_from.is_some() || query.date_to.is_some() {
            let lower = match query.date_from {
                Some(ts) => Bound::Included(ts),
                None => Bound::Unbounded,
            };
            let upper = match query.date_to {
                Some(ts) => Bound::Included(ts),
                None => Bound::Unbounded,
            };
            let range_query =
                RangeQuery::new_i64_bounds("date_epoch".to_string(), lower, upper);
            clauses.push((Occur::Must, Box::new(range_query)));
        }

        // Optional has_attachment filter.
        if let Some(has_att) = query.has_attachment {
            let val = if has_att { 1u64 } else { 0u64 };
            let term = Term::from_field_u64(self.fields.has_attachments, val);
            let att_query = TermQuery::new(term, IndexRecordOption::Basic);
            clauses.push((Occur::Must, Box::new(att_query)));
        }

        let combined_query = BooleanQuery::new(clauses);

        // Get total count and paginated results.
        let total_limit = query.offset + query.limit;
        let (total_count, top_docs) = searcher
            .search(&combined_query, &(Count, TopDocs::with_limit(total_limit)))
            .map_err(|e| format!("Search failed: {e}"))?;

        // Build a snippet generator for the subject field.
        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &combined_query, self.fields.subject)
                .map_err(|e| format!("Failed to create snippet generator: {e}"))?;
        snippet_generator.set_max_num_chars(200);

        let mut results = Vec::new();
        for (i, (score, doc_address)) in top_docs.into_iter().enumerate() {
            if i < query.offset {
                continue;
            }

            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| format!("Failed to retrieve document: {e}"))?;

            let uid_val = retrieved_doc
                .get_first(self.fields.uid)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let folder_val = retrieved_doc
                .get_first(self.fields.folder)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
            let snippet_html = snippet.to_html();

            results.push(SearchResult {
                uid: uid_val,
                folder: folder_val,
                score,
                snippet: snippet_html,
            });
        }

        Ok((results, total_count))
    }

    /// Delete existing documents that match the uid+folder combination.
    fn delete_existing(&self, writer: &mut IndexWriter, uid: u32, folder: &str) {
        let uid_term = Term::from_field_u64(self.fields.uid, uid as u64);
        let folder_term = Term::from_field_text(self.fields.folder, folder);

        let uid_query = TermQuery::new(uid_term, IndexRecordOption::Basic);
        let folder_query = TermQuery::new(folder_term, IndexRecordOption::Basic);

        let delete_query = BooleanQuery::new(vec![
            (Occur::Must, Box::new(uid_query)),
            (Occur::Must, Box::new(folder_query)),
        ]);

        let _ = writer.delete_query(Box::new(delete_query));
    }

    /// Build a Tantivy document from an IndexableMessage.
    fn build_document(&self, msg: &IndexableMessage) -> TantivyDocument {
        doc!(
            self.fields.uid => msg.uid as u64,
            self.fields.folder => msg.folder.as_str(),
            self.fields.subject => msg.subject.as_str(),
            self.fields.from_address => msg.from_address.as_str(),
            self.fields.from_name => msg.from_name.as_str(),
            self.fields.to_addresses => msg.to_addresses.as_str(),
            self.fields.body_text => msg.body_text.as_str(),
            self.fields.date_epoch => msg.date_epoch,
            self.fields.has_attachments => if msg.has_attachments { 1u64 } else { 0u64 }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a SearchEngine + UserIndex in a temp directory.
    fn setup() -> (TempDir, UserIndex) {
        let tmp = TempDir::new().expect("Failed to create temp dir");
        let engine = SearchEngine::new(tmp.path().to_path_buf());
        let user_index = engine
            .open_user_index("testuser")
            .expect("Failed to open user index");
        (tmp, user_index)
    }

    fn make_message(uid: u32, folder: &str, subject: &str, body: &str) -> IndexableMessage {
        IndexableMessage {
            uid,
            folder: folder.to_string(),
            subject: subject.to_string(),
            from_address: "sender@example.com".to_string(),
            from_name: "Sender Name".to_string(),
            to_addresses: "recipient@example.com".to_string(),
            body_text: body.to_string(),
            date_epoch: 1700000000,
            has_attachments: false,
        }
    }

    #[test]
    fn index_and_search_by_subject() {
        let (_tmp, idx) = setup();

        let msg1 = make_message(1, "INBOX", "Meeting tomorrow morning", "Let us meet.");
        let msg2 = make_message(2, "INBOX", "Invoice for October", "Please pay the invoice.");

        idx.index_message(&msg1).unwrap();
        idx.index_message(&msg2).unwrap();

        let query = SearchQuery {
            text: "meeting".to_string(),
            limit: 10,
            ..Default::default()
        };
        let (results, total) = idx.search(&query).unwrap();

        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uid, 1);
        assert_eq!(results[0].folder, "INBOX");
    }

    #[test]
    fn index_and_search_by_body() {
        let (_tmp, idx) = setup();

        let msg1 = make_message(1, "INBOX", "Hello", "The quarterly report is attached.");
        let msg2 = make_message(2, "INBOX", "Greetings", "Please review the budget proposal.");

        idx.index_messages_batch(&[msg1, msg2]).unwrap();

        let query = SearchQuery {
            text: "quarterly".to_string(),
            limit: 10,
            ..Default::default()
        };
        let (results, total) = idx.search(&query).unwrap();

        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uid, 1);
    }

    #[test]
    fn search_with_folder_filter() {
        let (_tmp, idx) = setup();

        let msg1 = make_message(1, "INBOX", "Project update", "Status of the project.");
        let msg2 = make_message(2, "Sent", "Project update", "Status of the project.");

        idx.index_messages_batch(&[msg1, msg2]).unwrap();

        // Search with folder filter for "Sent" only.
        let query = SearchQuery {
            text: "project".to_string(),
            folder: Some("Sent".to_string()),
            limit: 10,
            ..Default::default()
        };
        let (results, total) = idx.search(&query).unwrap();

        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uid, 2);
        assert_eq!(results[0].folder, "Sent");
    }

    #[test]
    fn search_with_date_range() {
        let (_tmp, idx) = setup();

        let mut msg1 = make_message(1, "INBOX", "January news", "News from January.");
        msg1.date_epoch = 1704067200; // 2024-01-01

        let mut msg2 = make_message(2, "INBOX", "March news", "News from March.");
        msg2.date_epoch = 1709251200; // 2024-03-01

        let mut msg3 = make_message(3, "INBOX", "June news", "News from June.");
        msg3.date_epoch = 1717200000; // 2024-06-01

        idx.index_messages_batch(&[msg1, msg2, msg3]).unwrap();

        // Search for "news" within February to April.
        let query = SearchQuery {
            text: "news".to_string(),
            date_from: Some(1706745600), // 2024-02-01
            date_to: Some(1714521600),   // 2024-05-01
            limit: 10,
            ..Default::default()
        };
        let (results, total) = idx.search(&query).unwrap();

        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uid, 2);
    }

    #[test]
    fn delete_removes_from_index() {
        let (_tmp, idx) = setup();

        let msg1 = make_message(1, "INBOX", "Important email", "Very important content.");
        let msg2 = make_message(2, "INBOX", "Another email", "Some other important content.");

        idx.index_messages_batch(&[msg1, msg2]).unwrap();

        // Verify both are found.
        let query = SearchQuery {
            text: "important".to_string(),
            limit: 10,
            ..Default::default()
        };
        let (_, total) = idx.search(&query).unwrap();
        assert_eq!(total, 2);

        // Delete message 1.
        idx.delete_message(1, "INBOX").unwrap();

        let (results, total) = idx.search(&query).unwrap();
        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uid, 2);
    }

    #[test]
    fn search_empty_text_returns_empty() {
        let (_tmp, idx) = setup();

        let msg = make_message(1, "INBOX", "Test subject", "Test body text.");
        idx.index_message(&msg).unwrap();

        let query = SearchQuery {
            text: "".to_string(),
            limit: 10,
            ..Default::default()
        };
        let (results, total) = idx.search(&query).unwrap();

        assert_eq!(total, 0);
        assert!(results.is_empty());
    }

    #[test]
    fn search_pagination() {
        let (_tmp, idx) = setup();

        let messages: Vec<IndexableMessage> = (1..=5)
            .map(|i| make_message(i, "INBOX", &format!("Report number {i}"), "Detailed report content here."))
            .collect();

        idx.index_messages_batch(&messages).unwrap();

        // Page 1: limit=2, offset=0.
        let query_page1 = SearchQuery {
            text: "report".to_string(),
            limit: 2,
            offset: 0,
            ..Default::default()
        };
        let (results1, total1) = idx.search(&query_page1).unwrap();
        assert_eq!(total1, 5);
        assert_eq!(results1.len(), 2);

        // Page 2: limit=2, offset=2.
        let query_page2 = SearchQuery {
            text: "report".to_string(),
            limit: 2,
            offset: 2,
            ..Default::default()
        };
        let (results2, total2) = idx.search(&query_page2).unwrap();
        assert_eq!(total2, 5);
        assert_eq!(results2.len(), 2);

        // Results from page 1 and page 2 should be different.
        let uids1: Vec<u32> = results1.iter().map(|r| r.uid).collect();
        let uids2: Vec<u32> = results2.iter().map(|r| r.uid).collect();
        for uid in &uids2 {
            assert!(
                !uids1.contains(uid),
                "Page 2 result uid {uid} should not appear in page 1"
            );
        }
    }
}
